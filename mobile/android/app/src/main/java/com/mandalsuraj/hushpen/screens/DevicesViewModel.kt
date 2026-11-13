package com.mandalsuraj.hushpen.screens

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.mandalsuraj.hushpen.core.HostUnloadRule
import com.mandalsuraj.hushpen.core.PairedDesktop
import com.mandalsuraj.hushpen.core.PairingState
import com.mandalsuraj.hushpen.server.ServerState
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

interface DevicesData {
    val pairedClients: StateFlow<List<PairedDesktop>>
    val allowedRules: StateFlow<List<HostUnloadRule>>
    val unloadRule: StateFlow<HostUnloadRule>
    val serverState: StateFlow<ServerState>
    suspend fun initialize()
    suspend fun refresh()
    suspend fun pair(payload: String): String
    suspend fun pairCode(code: String, address: String): String
    suspend fun openPairing()
    suspend fun closePairing()
    suspend fun pairingState(): PairingState
    suspend fun answerPairing(number: String, approve: Boolean)
    suspend fun forget(id: String)
    suspend fun setUnloadRule(rule: HostUnloadRule)
    fun startServing()
    fun stopServing()
}

class DevicesViewModel(private val repository: DevicesData) : ViewModel() {
    val pairedClients = repository.pairedClients
    val allowedRules = repository.allowedRules
    val unloadRule = repository.unloadRule
    private val mutableServer = MutableStateFlow(ServerState())
    val server: StateFlow<ServerState> = mutableServer
    private val mutableNotice = MutableStateFlow<String?>(null)
    val notice: StateFlow<String?> = mutableNotice
    private val mutableScanner = MutableStateFlow(false)
    val scannerOpen: StateFlow<Boolean> = mutableScanner
    private val mutableTyping = MutableStateFlow(false)
    val typing: StateFlow<Boolean> = mutableTyping
    private val mutableCodeError = MutableStateFlow<String?>(null)
    val codeError: StateFlow<String?> = mutableCodeError
    private val mutableBusy = MutableStateFlow(false)
    val busy: StateFlow<Boolean> = mutableBusy
    /** The number both screens show while a computer pairs through Nearby. */
    private val mutableComparing = MutableStateFlow<String?>(null)
    val comparing: StateFlow<String?> = mutableComparing
    /** The window is open: a computer can pick this phone now. */
    private val mutableListening = MutableStateFlow(false)
    val listening: StateFlow<Boolean> = mutableListening
    private var watchers = 0
    private var watching: Job? = null
    private val mutableIdleMinutes = MutableStateFlow("30")
    val idleMinutes: StateFlow<String> = mutableIdleMinutes

    init {
        run {
            repository.serverState.collect { mutableServer.value = it }
        }
    }

    fun refresh() = run { repository.refresh() }
    fun scan() { mutableScanner.value = true }
    fun closeScanner() { mutableScanner.value = false }
    fun scanned(payload: String) {
        mutableScanner.value = false
        run { mutableNotice.value = paired(repository.pair(payload)) }
    }
    fun typeCode() {
        mutableScanner.value = false
        mutableCodeError.value = null
        mutableTyping.value = true
    }
    fun closeTyping() { mutableTyping.value = false }
    /** An empty address looks for the computer through Bonjour; otherwise it is host:port. */
    fun pairCode(code: String, address: String) {
        viewModelScope.launch {
            mutableBusy.value = true
            mutableCodeError.value = null
            try {
                repository.initialize()
                mutableNotice.value = paired(repository.pairCode(code, address))
                mutableTyping.value = false
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (problem: Exception) {
                mutableCodeError.value = problem.message?.removePrefix("detail=") ?: "Pairing failed."
            } finally {
                mutableBusy.value = false
            }
        }
    }
    private fun paired(name: String) = if (name.isBlank()) "Computer paired." else "$name paired."

    /** Opening a screen that can pair opens the Nearby window once. Expiry, three tries, a pairing,
     *  or a failure close it, and it stays closed until the person asks again. */
    fun watch() {
        watchers += 1
        if (watching == null) watching = viewModelScope.launch { listen() }
    }

    fun unwatch() {
        watchers = maxOf(0, watchers - 1)
        if (watchers > 0) return
        watching?.cancel()
        watching = null
        mutableComparing.value = null
        mutableListening.value = false
        viewModelScope.launch { runCatching { repository.closePairing() } }
    }

    /** The explicit action: Try again, or Pair another computer. */
    fun reopenPairing() {
        mutableNotice.value = null
        viewModelScope.launch { runCatching { repository.openPairing() } }
    }

    /** Pair needs the number the person saw; Cancel always works. */
    fun answer(number: String, approve: Boolean) {
        mutableComparing.value = null
        viewModelScope.launch { runCatching { repository.answerPairing(number, approve) } }
    }

    private suspend fun listen() {
        var opened = false
        var last: PairingState = PairingState.Closed
        while (currentCoroutineContext().isActive) {
            if (mutableServer.value.running) {
                if (!opened) {
                    opened = true
                    runCatching { repository.openPairing() }
                }
                runCatching { repository.pairingState() }.getOrNull()?.let { state ->
                    mutableComparing.value = (state as? PairingState.Comparing)?.number
                    mutableListening.value = state is PairingState.Listening || state is PairingState.Comparing
                    if (state != last) {
                        when (state) {
                            is PairingState.Paired -> {
                                mutableNotice.value = "Computer paired."
                                runCatching { repository.refresh() }
                            }
                            is PairingState.Failed -> mutableNotice.value = state.message
                            is PairingState.Closed -> if (last is PairingState.Listening) {
                                mutableNotice.value = "Pairing closed. Tap Try again to pair from your computer."
                            }
                            else -> Unit
                        }
                        last = state
                    }
                }
            } else {
                opened = false
                mutableComparing.value = null
                mutableListening.value = false
                last = PairingState.Closed
            }
            delay(400)
        }
    }
    fun forget(id: String) = run { repository.forget(id) }
    fun setRule(rule: HostUnloadRule) = run { repository.setUnloadRule(rule) }
    fun setIdleMinutes(minutes: String) { mutableIdleMinutes.value = minutes }
    fun applyIdleMinutes() = run {
        val minutes = mutableIdleMinutes.value.toUIntOrNull()
            ?: throw IllegalArgumentException("Enter idle minutes as a whole number.")
        repository.setUnloadRule(HostUnloadRule.AfterIdle(minutes))
    }
    fun setServing(enabled: Boolean) = run {
        if (enabled) repository.startServing()
        else repository.stopServing()
    }

    private fun run(action: suspend () -> Unit) = viewModelScope.launch {
        mutableNotice.value = null
        try {
            repository.initialize()
            action()
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (problem: Exception) {
            mutableNotice.value = problem.message ?: "The device action failed."
        }
    }
}
