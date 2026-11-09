package com.mandalsuraj.hushpen.screens

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

interface SettingsData {
    val cleanupEnabled: StateFlow<Boolean>
    val hapticsEnabled: StateFlow<Boolean>
    val systemAvailable: StateFlow<Boolean>
    suspend fun setCleanup(enabled: Boolean)
    suspend fun setHaptics(enabled: Boolean)
}

class SettingsViewModel(private val repository: SettingsData) : ViewModel() {
    val cleanupEnabled = repository.cleanupEnabled
    val hapticsEnabled = repository.hapticsEnabled
    val systemAvailable = repository.systemAvailable
    private val mutableNotice = MutableStateFlow<String?>(null)
    val notice: StateFlow<String?> = mutableNotice

    fun setCleanup(enabled: Boolean) = run { repository.setCleanup(enabled) }
    fun setHaptics(enabled: Boolean) = run { repository.setHaptics(enabled) }

    private fun run(action: suspend () -> Unit) = viewModelScope.launch {
        mutableNotice.value = null
        try {
            action()
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (problem: Exception) {
            mutableNotice.value = problem.message ?: "The setting could not be saved."
        }
    }
}
