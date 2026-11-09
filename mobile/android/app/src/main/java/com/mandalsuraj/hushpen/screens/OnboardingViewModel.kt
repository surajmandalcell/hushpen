package com.mandalsuraj.hushpen.screens

import androidx.lifecycle.ViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

data class OnboardingState(val microphoneGranted: Boolean = false, val keyboardEnabled: Boolean = false, val keyboardSelected: Boolean = false)

class OnboardingViewModel : ViewModel() {
    private val mutableState = MutableStateFlow(OnboardingState())
    val state: StateFlow<OnboardingState> = mutableState

    fun update(microphoneGranted: Boolean, keyboardEnabled: Boolean, keyboardSelected: Boolean) {
        mutableState.value = OnboardingState(microphoneGranted, keyboardEnabled, keyboardSelected)
    }
}
