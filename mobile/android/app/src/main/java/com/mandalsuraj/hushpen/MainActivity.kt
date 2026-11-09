package com.mandalsuraj.hushpen

import android.Manifest
import android.content.ComponentName
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Bundle
import android.provider.Settings
import android.view.inputmethod.InputMethodManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import com.mandalsuraj.hushpen.screens.DevicesViewModel
import com.mandalsuraj.hushpen.screens.ModelsViewModel
import com.mandalsuraj.hushpen.screens.OnboardingViewModel
import com.mandalsuraj.hushpen.screens.PlaceholderDevices
import com.mandalsuraj.hushpen.screens.PlaceholderModels
import com.mandalsuraj.hushpen.screens.PlaceholderOnboarding
import com.mandalsuraj.hushpen.screens.PlaceholderSettings
import com.mandalsuraj.hushpen.screens.SettingsViewModel
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    private val repository get() = (application as PhoneApplication).repository
    private lateinit var onboarding: OnboardingViewModel

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            val onboardingModel: OnboardingViewModel = viewModel()
            onboarding = onboardingModel
            LaunchedEffect(onboardingModel) { refreshPermissions() }
            val models: ModelsViewModel = viewModel(factory = viewModelFactory { initializer { ModelsViewModel(repository) } })
            val devices: DevicesViewModel = viewModel(factory = viewModelFactory { initializer { DevicesViewModel(repository) } })
            val settings: SettingsViewModel = viewModel(factory = viewModelFactory { initializer { SettingsViewModel(repository) } })
            var screen by rememberSaveable { mutableStateOf("Onboarding") }
            val microphone = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { refreshPermissions() }
            val notifications = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { }
            Column(Modifier.verticalScroll(rememberScrollState())) {
                Text("Hushpen")
                listOf("Onboarding", "Models", "Devices", "Settings").forEach { destination ->
                    Button(onClick = { screen = destination }) { Text(destination) }
                }
                when (screen) {
                    "Onboarding" -> PlaceholderOnboarding(
                        onboardingModel,
                        models,
                        onMicrophone = { microphone.launch(Manifest.permission.RECORD_AUDIO) },
                        onKeyboardSettings = { startActivity(Intent(Settings.ACTION_INPUT_METHOD_SETTINGS)) },
                        onKeyboardPicker = { getSystemService(InputMethodManager::class.java).showInputMethodPicker() },
                    )
                    "Models" -> PlaceholderModels(models)
                    "Devices" -> PlaceholderDevices(devices) {
                        if (android.os.Build.VERSION.SDK_INT >= 33) notifications.launch(Manifest.permission.POST_NOTIFICATIONS)
                    }
                    "Settings" -> PlaceholderSettings(settings) { startActivity(Intent(this@MainActivity, LicensesActivity::class.java)) }
                }
            }
        }
    }

    override fun onResume() {
        super.onResume()
        repository.setActivityVisible(true)
        refreshPermissions()
        lifecycleScope.launch {
            try {
                repository.initialize()
                repository.refresh()
            } catch (problem: Exception) {
                android.util.Log.e("Hushpen", "Could not initialize phone models", problem)
            }
        }
    }

    override fun onStop() {
        repository.setActivityVisible(false)
        super.onStop()
    }

    private fun refreshPermissions() {
        if (!::onboarding.isInitialized) return
        val inputMethods = getSystemService(InputMethodManager::class.java)
        val ownId = ComponentName(this, "com.mandalsuraj.hushpen.ime.HushpenInputMethod")
        val enabled = inputMethods.enabledInputMethodList.any { ComponentName(it.packageName, it.serviceName) == ownId }
        val selected = Settings.Secure.getString(contentResolver, Settings.Secure.DEFAULT_INPUT_METHOD)
            ?.let(ComponentName::unflattenFromString) == ownId
        onboarding.update(checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED, enabled, selected)
    }
}
