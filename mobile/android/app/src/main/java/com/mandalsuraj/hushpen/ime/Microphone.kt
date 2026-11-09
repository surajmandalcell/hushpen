package com.mandalsuraj.hushpen.ime

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import java.io.File
import java.io.RandomAccessFile
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.concurrent.atomic.AtomicBoolean

internal class Microphone(private val context: Context) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private var recorder: AudioRecord? = null
    private var reader: Job? = null
    private val running = AtomicBoolean(false)
    private var failure: Throwable? = null
    private val operation = Mutex()

    suspend fun start(file: File, maxBytes: Long, onFailure: (Throwable) -> Unit) = withContext(Dispatchers.IO) { operation.withLock {
        require(maxBytes in 1..Int.MAX_VALUE.toLong() - 44) { "Invalid recording limit." }
        check(recorder == null) { "Microphone is already recording." }
        check(context.checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) {
            "Allow the microphone in Hushpen first."
        }
        val bytes = AudioRecord.getMinBufferSize(SAMPLE_RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT)
        check(bytes > 0) { "The microphone does not support 16 kHz audio." }
        val record = AudioRecord.Builder()
            .setAudioSource(MediaRecorder.AudioSource.VOICE_RECOGNITION)
            .setAudioFormat(AudioFormat.Builder().setSampleRate(SAMPLE_RATE)
                .setChannelMask(AudioFormat.CHANNEL_IN_MONO).setEncoding(AudioFormat.ENCODING_PCM_16BIT).build())
            .setBufferSizeInBytes(bytes * 2)
            .build()
        try {
            check(record.state == AudioRecord.STATE_INITIALIZED) { "The microphone did not open." }
            record.startRecording()
            check(record.recordingState == AudioRecord.RECORDSTATE_RECORDING) { "The microphone did not start." }
        } catch (error: Throwable) {
            record.release()
            throw error
        }
        recorder = record
        failure = null
        running.set(true)
        reader = scope.launch(Dispatchers.IO) {
            try {
                RandomAccessFile(file, "rw").use { output ->
                    output.setLength(0)
                    output.write(wavHeader(0))
                    val buffer = ByteArray(bytes)
                    while (running.get()) {
                        val count = record.read(buffer, 0, buffer.size, AudioRecord.READ_BLOCKING)
                        if (count < 0 && running.get()) error("Microphone read failed ($count).")
                        if (count > 0) {
                            check(output.length() + count <= maxBytes + 44) { "The recording reached its maximum length." }
                            output.write(buffer, 0, count)
                        }
                    }
                    val length = output.length() - 44
                    check(length <= Int.MAX_VALUE) { "The recording is too large." }
                    output.seek(0)
                    output.write(wavHeader(length.toInt()))
                }
            } catch (error: Throwable) {
                failure = error
                running.set(false)
                onFailure(error)
            }
        }
    } }

    suspend fun stop() = operation.withLock { withContext(Dispatchers.IO) {
        val record = recorder ?: return@withContext
        running.set(false)
        try {
            record.stop()
            withTimeout(2_000) { reader?.join() }
        } finally {
            record.release()
            reader?.cancel()
            recorder = null
            reader = null
        }
        failure?.let { throw it }
    } }

    fun close(onClosed: () -> Unit) {
        scope.launch {
            try {
                stop()
            } catch (error: Exception) {
                Log.w("Hushpen", "Microphone shutdown failed.", error)
            } finally {
                onClosed()
                scope.cancel()
            }
        }
    }

    companion object {
        const val SAMPLE_RATE = 16_000

        internal fun wavHeader(bytes: Int): ByteArray = ByteBuffer.allocate(44).order(ByteOrder.LITTLE_ENDIAN)
            .put("RIFF".toByteArray(Charsets.US_ASCII)).putInt(bytes + 36)
            .put("WAVEfmt ".toByteArray(Charsets.US_ASCII)).putInt(16)
            .putShort(1).putShort(1).putInt(SAMPLE_RATE).putInt(SAMPLE_RATE * 2)
            .putShort(2).putShort(16).put("data".toByteArray(Charsets.US_ASCII)).putInt(bytes).array()
    }
}
