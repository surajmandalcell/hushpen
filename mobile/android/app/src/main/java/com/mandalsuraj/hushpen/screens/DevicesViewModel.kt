package com.mandalsuraj.hushpen.screens

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.mandalsuraj.hushpen.core.HostUnloadRule
import com.mandalsuraj.hushpen.server.ServerState
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

interface DevicesData {
    val pairedClients: StateFlow<List<String>>
    val allowedRules: StateFlow<List<HostUnloadRule>>
    val unloadRule: StateFlow<HostUnloadRule>
    val serverState: StateFlow<ServerState>
    suspend fun initialize()
    suspend fun refresh()
    suspend fun pair(uri: String)
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
    private val mutablePairingUri = MutableStateFlow("")
    val pairingUri: StateFlow<String> = mutablePairingUri
    private val mutableScanner = MutableStateFlow(false)
    val scannerOpen: StateFlow<Boolean> = mutableScanner
    private val mutableIdleMinutes = MutableStateFlow("30")
    val idleMinutes: StateFlow<String> = mutableIdleMinutes

    init {
        run {
            repository.serverState.collect { mutableServer.value = it }
        }
    }

    fun refresh() = run { repository.refresh() }
    fun setPairingUri(uri: String) { mutablePairingUri.value = uri }
    fun scan() { mutableScanner.value = true }
    fun closeScanner() { mutableScanner.value = false }
    fun scanned(uri: String) {
        mutablePairingUri.value = uri
        mutableScanner.value = false
        pair()
    }
    fun pair() = run { repository.pair(mutablePairingUri.value) }
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
