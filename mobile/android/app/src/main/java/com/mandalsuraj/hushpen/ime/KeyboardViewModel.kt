package com.mandalsuraj.hushpen.ime

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.os.SystemClock
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.mandalsuraj.hushpen.BuildConfig
import com.mandalsuraj.hushpen.PhoneRepository
import com.mandalsuraj.hushpen.core.Dictation
import com.mandalsuraj.hushpen.core.DictationEffect
import com.mandalsuraj.hushpen.core.DictationEvent
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

data class KeyboardState(
    val phase: String = "idle",
    val notice: String = "",
    val shift: Boolean = false,
    val symbols: Boolean = false,
    val canUndo: Boolean = false,
    val canDictate: Boolean = true,
)

class KeyboardViewModel(
    private val context: Context,
    private val repository: PhoneRepository,
) : ViewModel() {
    private val mutableState = MutableStateFlow(KeyboardState())
    val state = mutableState.asStateFlow()
    val hapticsEnabled = repository.hapticsEnabled
    private var machine = Dictation(MAX_MINUTES)
    private val events = Channel<Pair<Dictation, DictationEvent>>(Channel.UNLIMITED)
    private val microphone = Microphone(context)
    private val audio = mutableMapOf<ULong, File>()
    private val jobs = mutableMapOf<ULong, Job>()
    private var insert: ((String) -> Boolean)? = null
    val inputOpen: Boolean get() = insert != null
    private var activeCapture: ULong? = null
    private var stopping: Job? = null

    init {
        viewModelScope.launch {
            for ((owner, event) in events) {
                if (owner !== machine) continue
                try {
                    for (effect in owner.handle(event, now())) {
                        if (owner !== machine) break
                        execute(owner, effect)
                    }
                    if (owner === machine) mutableState.value = mutableState.value.copy(phase = owner.snapshot(now()).state)
                } catch (error: CancellationException) {
                    throw error
                } catch (error: Exception) {
                    if (owner !== machine) continue
                    mutableState.value = mutableState.value.copy(notice = error.message ?: "Dictation failed.")
                    val snapshot = owner.snapshot(now())
                    events.trySend(owner to if (snapshot.state == "transcribing") {
                        DictationEvent.TranscribeFailed(snapshot.session)
                    } else DictationEvent.CaptureFailed)
                }
            }
        }
        viewModelScope.launch {
            while (isActive) {
                delay(100)
                send(DictationEvent.Tick)
            }
        }
    }

    fun openInput(canDictate: Boolean, deliver: (String) -> Boolean) {
        closeInput()
        insert = deliver
        mutableState.value = KeyboardState(canDictate = canDictate)
    }

    fun closeInput() {
        insert = null
        jobs.values.forEach { it.cancel() }
        jobs.clear()
        repository.cancelSpeech()
        val previous = machine
        machine = Dictation(MAX_MINUTES)
        previous.close()
        val files = audio.values.toList()
        audio.clear()
        activeCapture = null
        val previousStop = stopping
        stopping = viewModelScope.launch {
            previousStop?.join()
            runCatching { microphone.stop() }
            withContext(Dispatchers.IO) { files.forEach(File::delete) }
        }
    }

    fun toggle() = dictate(DictationEvent.Toggle)
    fun holdDown() = dictate(DictationEvent.HoldDown)
    fun holdUp() = send(DictationEvent.HoldUp)
    fun cancel() = send(DictationEvent.Cancel)
    fun recover() = send(DictationEvent.Recover)

    fun shift() {
        mutableState.value = mutableState.value.copy(shift = !state.value.shift)
    }

    fun symbols() {
        mutableState.value = mutableState.value.copy(symbols = !state.value.symbols)
    }

    fun typed() {
        mutableState.value = mutableState.value.copy(shift = false, canUndo = false)
    }

    fun setUndo(available: Boolean) {
        mutableState.value = mutableState.value.copy(canUndo = available)
    }

    fun notice(text: String) {
        mutableState.value = mutableState.value.copy(notice = text)
    }

    private fun dictate(event: DictationEvent) {
        if (!state.value.canDictate) {
            notice("Dictation is off in password fields.")
            return
        }
        send(event)
    }

    private fun send(event: DictationEvent) {
        events.trySend(machine to event)
    }

    private suspend fun execute(owner: Dictation, effect: DictationEffect) {
        when (effect) {
            is DictationEffect.StartCapture -> {
                stopping?.join()
                if (owner !== machine) return
                check(context.checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) {
                    "Allow the microphone in Hushpen first."
                }
                mutableState.value = mutableState.value.copy(notice = "", canUndo = false)
                val folder = File(context.cacheDir, "dictation").apply { mkdirs() }
                val file = File.createTempFile("session-", ".wav", folder)
                audio[effect.session] = file
                val test = File(context.filesDir, "ime-test.wav")
                if (BuildConfig.DEBUG && test.isFile) {
                    withContext(Dispatchers.IO) { test.copyTo(file, overwrite = true) }
                } else {
                    microphone.start(file, MAX_MINUTES.toLong() * 60 * Microphone.SAMPLE_RATE * 2) { error ->
                        viewModelScope.launch {
                            if (owner === machine) notice(error.message ?: "Microphone capture failed.")
                        }
                        events.trySend(owner to DictationEvent.CaptureFailed)
                    }
                    activeCapture = effect.session
                }
            }
            is DictationEffect.StopCapture -> {
                try {
                    stopCapture(effect.session)
                } catch (error: Exception) {
                    dropAudio(effect.session)
                    throw error
                }
                if (!effect.keep) dropAudio(effect.session)
            }
            is DictationEffect.HoldCapture -> stopCapture(effect.session)
            is DictationEffect.DropAudio -> dropAudio(effect.session)
            is DictationEffect.Transcribe -> {
                val file = audio[effect.session] ?: error("The recording is missing.")
                jobs[effect.session] = viewModelScope.launch {
                    try {
                        val text = repository.transcribe(file)
                        events.send(owner to DictationEvent.Transcribed(effect.session, text))
                    } catch (error: CancellationException) {
                        throw error
                    } catch (error: Exception) {
                        if (owner === machine) notice(error.message ?: "Speech recognition failed.")
                        events.send(owner to DictationEvent.TranscribeFailed(effect.session))
                    }
                }
            }
            is DictationEffect.CancelTranscribe -> {
                repository.cancelSpeech()
                jobs.remove(effect.session)?.cancel()
            }
            is DictationEffect.Clean -> {
                jobs[effect.session] = viewModelScope.launch {
                    try {
                        val text = repository.cleanup(effect.raw)
                        events.send(owner to DictationEvent.Cleaned(effect.session, text))
                    } catch (error: CancellationException) {
                        throw error
                    } catch (error: Exception) {
                        if (owner === machine) notice(error.message ?: "Text cleanup failed.")
                        events.send(owner to DictationEvent.Cleaned(effect.session, effect.raw))
                    }
                }
            }
            is DictationEffect.StopLlm -> jobs.remove(effect.session)?.cancel()
            is DictationEffect.Insert -> {
                val inserted = !effect.copy && (insert?.invoke(effect.text) == true)
                events.send(owner to if (inserted) DictationEvent.Inserted(effect.session) else DictationEvent.InsertFailed(effect.session))
            }
            is DictationEffect.SaveRow -> {
                jobs.remove(effect.session)
                dropAudio(effect.session)
            }
            is DictationEffect.Notify -> notice(effect.code)
            is DictationEffect.MaxDurationWarning -> notice("${effect.secondsLeft} seconds left.")
            is DictationEffect.Cue, is DictationEffect.UpdatePasteLast, DictationEffect.PasteLast -> Unit
            is DictationEffect.Edit -> error("Keyboard editing is unavailable.")
        }
    }

    private suspend fun stopCapture(session: ULong) {
        if (activeCapture == session) {
            activeCapture = null
            microphone.stop()
        }
    }

    private suspend fun dropAudio(session: ULong) {
        stopCapture(session)
        audio.remove(session)?.let { file -> withContext(Dispatchers.IO) { file.delete() } }
    }

    private fun now() = SystemClock.elapsedRealtime().toULong()

    override fun onCleared() {
        repository.cancelSpeech()
        val files = audio.values.toList()
        microphone.close { files.forEach(File::delete) }
        machine.close()
        events.close()
    }

    companion object {
        private const val MAX_MINUTES: ULong = 6u
    }
}
