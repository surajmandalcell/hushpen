package com.mandalsuraj.hushpen

import android.Manifest
import android.content.ComponentName
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.view.inputmethod.InputMethodManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import com.mandalsuraj.hushpen.core.ModelKind
import com.mandalsuraj.hushpen.screens.DevicesScreen
import com.mandalsuraj.hushpen.screens.DevicesViewModel
import com.mandalsuraj.hushpen.screens.HomeScreen
import com.mandalsuraj.hushpen.screens.ModelsScreen
import com.mandalsuraj.hushpen.screens.ModelsViewModel
import com.mandalsuraj.hushpen.screens.OnboardingViewModel
import com.mandalsuraj.hushpen.screens.Screen
import com.mandalsuraj.hushpen.screens.ServerScreen
import com.mandalsuraj.hushpen.screens.SettingsScreen
import com.mandalsuraj.hushpen.screens.SettingsViewModel
import com.mandalsuraj.hushpen.screens.desktopName
import com.mandalsuraj.hushpen.ui.HushpenTheme
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    private val repository get() = (application as PhoneApplication).repository
    private lateinit var onboarding: OnboardingViewModel

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        setContent {
            HushpenTheme {
                val onboardingModel: OnboardingViewModel = viewModel()
                onboarding = onboardingModel
                LaunchedEffect(onboardingModel) { refreshPermissions() }
                val models: ModelsViewModel = viewModel(factory = viewModelFactory { initializer { ModelsViewModel(repository) } })
                val devices: DevicesViewModel = viewModel(factory = viewModelFactory { initializer { DevicesViewModel(repository) } })
                val settings: SettingsViewModel = viewModel(factory = viewModelFactory { initializer { SettingsViewModel(repository) } })
                var screen by rememberSaveable { mutableStateOf(Screen.Home) }
                var requested by rememberSaveable { mutableStateOf(false) }
                val server by devices.server.collectAsStateWithLifecycle()
                LaunchedEffect(server.running, server.notice) { if (server.running || server.notice != null) requested = false }
                val microphone = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { refreshPermissions() }
                val notifications = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { }
                val serve: (Boolean) -> Unit = { on ->
                    if (on && Build.VERSION.SDK_INT >= 33 &&
                        checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
                    ) {
                        notifications.launch(Manifest.permission.POST_NOTIFICATIONS)
                    }
                    requested = on
                    devices.setServing(on)
                }
                val home = { screen = Screen.Home }
                BackHandler(enabled = screen != Screen.Home, onBack = home)
                when (screen) {
                    Screen.Home -> HomeScreen(
                        onboardingModel,
                        models,
                        devices,
                        onMicrophone = { microphone.launch(Manifest.permission.RECORD_AUDIO) },
                        onKeyboardSettings = { startActivity(Intent(Settings.ACTION_INPUT_METHOD_SETTINGS)) },
                        onKeyboardPicker = { getSystemService(InputMethodManager::class.java).showInputMethodPicker() },
                        onServing = { on ->
                            serve(on)
                            if (on) screen = Screen.Server
                        },
                        onOpen = { screen = it },
                    )
                    Screen.Models -> ModelsScreen(models, settings, onBack = home)
                    Screen.Devices -> DevicesScreen(devices, onServing = serve, onBack = home)
                    Screen.Settings -> SettingsScreen(
                        settings,
                        onLicenses = { startActivity(Intent(this, LicensesActivity::class.java)) },
                        onBack = home,
                    )
                    Screen.Server -> {
                        val catalog by models.models.collectAsStateWithLifecycle()
                        val selected by models.selectedModel.collectAsStateWithLifecycle()
                        val selectedCleanup by models.selectedCleanupModel.collectAsStateWithLifecycle()
                        val rule by devices.unloadRule.collectAsStateWithLifecycle()
                        val clients by devices.pairedClients.collectAsStateWithLifecycle()
                        val cleanup by settings.cleanupEnabled.collectAsStateWithLifecycle()
                        val system by settings.systemAvailable.collectAsStateWithLifecycle()
                        val speech = catalog.filter { it.kind == ModelKind.SPEECH }
                        ServerScreen(
                            server = server,
                            starting = requested && !server.running && server.notice == null,
                            rule = rule,
                            speechModel = (speech.firstOrNull { it.id == selected } ?: speech.firstOrNull { it.recommended })?.name ?: "None",
                            cleanup = when {
                                !cleanup -> "Rules"
                                system -> "System model when open"
                                else -> catalog.firstOrNull { it.id == selectedCleanup && it.status == com.mandalsuraj.hushpen.core.ModelStatus.READY }?.name ?: "Rules"
                            },
                            desktops = clients.map(::desktopName),
                            onStop = {
                                serve(false)
                                home()
                            },
                            onClose = home,
                        )
                    }
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
