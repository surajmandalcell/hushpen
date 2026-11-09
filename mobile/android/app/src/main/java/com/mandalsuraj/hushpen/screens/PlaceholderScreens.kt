package com.mandalsuraj.hushpen.screens

import androidx.compose.foundation.layout.Column
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.mandalsuraj.hushpen.core.HostUnloadRule
import com.mandalsuraj.hushpen.core.ModelKind
import com.mandalsuraj.hushpen.core.ModelStatus
import com.mandalsuraj.hushpen.server.PlaceholderScanner

@Composable
fun PlaceholderOnboarding(model: OnboardingViewModel, models: ModelsViewModel, onMicrophone: () -> Unit, onKeyboardSettings: () -> Unit, onKeyboardPicker: () -> Unit) {
    val state by model.state.collectAsStateWithLifecycle()
    Column {
        Text("Microphone: ${if (state.microphoneGranted) "Ready" else "Not allowed"}")
        Button(onClick = onMicrophone) { Text("Allow microphone") }
        Text("Keyboard enabled: ${state.keyboardEnabled}")
        Button(onClick = onKeyboardSettings) { Text("Enable Hushpen keyboard") }
        Text("Keyboard selected: ${state.keyboardSelected}")
        Button(onClick = onKeyboardPicker) { Text("Choose Hushpen keyboard") }
        PlaceholderModels(models)
    }
}

@Composable
fun PlaceholderModels(model: ModelsViewModel) {
    val models by model.models.collectAsStateWithLifecycle()
    val selected by model.selectedModel.collectAsStateWithLifecycle()
    val download by model.download.collectAsStateWithLifecycle()
    val notice by model.notice.collectAsStateWithLifecycle()
    Column {
        Text("Speech model: $selected")
        if (models.isEmpty()) Text("No models loaded.")
        models.forEach { entry ->
            Text("${entry.name}: ${entry.status}")
            if (entry.kind == ModelKind.SPEECH) {
                when (entry.status) {
                    ModelStatus.READY -> Button(onClick = { model.select(entry.id) }) { Text("Use ${entry.name}") }
                    ModelStatus.NEEDS_VERIFICATION -> Button(onClick = { model.verify(entry.id) }) { Text("Verify ${entry.name}") }
                    else -> Button(onClick = { model.download(entry.id) }, enabled = download.modelId == null) { Text("Download ${entry.name}") }
                }
            }
        }
        download.modelId?.let {
            Text("Download $it: ${download.done} / ${download.total}${if (download.verifying) " Verifying" else ""}")
            Button(onClick = { model.cancelDownload() }) { Text("Cancel download") }
        }
        notice?.let { Text(it) }
        Button(onClick = { model.refresh() }) { Text("Refresh models") }
    }
}

@Composable
fun PlaceholderDevices(model: DevicesViewModel, onNotificationPermission: () -> Unit) {
    val server by model.server.collectAsStateWithLifecycle()
    val clients by model.pairedClients.collectAsStateWithLifecycle()
    val rules by model.allowedRules.collectAsStateWithLifecycle()
    val selected by model.unloadRule.collectAsStateWithLifecycle()
    val uri by model.pairingUri.collectAsStateWithLifecycle()
    val scanner by model.scannerOpen.collectAsStateWithLifecycle()
    val idleMinutes by model.idleMinutes.collectAsStateWithLifecycle()
    val notice by model.notice.collectAsStateWithLifecycle()
    Column {
        Text("Server mode: ${server.running}")
        Checkbox(modifier = Modifier.semantics { contentDescription = "Server mode" }, checked = server.running, onCheckedChange = {
            if (it) onNotificationPermission()
            model.setServing(it)
        })
        server.port?.let { Text("Port: $it") }
        Text("Unload rule: ${ruleName(selected)}")
        rules.forEach { rule -> Button(onClick = { model.setRule(rule) }) { Text(ruleName(rule)) } }
        if (selected is HostUnloadRule.AfterIdle) {
            OutlinedTextField(value = idleMinutes, onValueChange = model::setIdleMinutes, label = { Text("Idle minutes") })
            Button(onClick = { model.applyIdleMinutes() }) { Text("Save idle minutes") }
        }
        if (clients.isEmpty()) Text("No paired desktops.")
        clients.forEach { client ->
            Text("Desktop: $client")
            Button(onClick = { model.forget(client) }) { Text("Forget $client") }
        }
        OutlinedTextField(value = uri, onValueChange = model::setPairingUri, label = { Text("Pairing link or Tailscale pairing link") })
        Button(onClick = { model.pair() }, enabled = server.running) { Text("Pair desktop") }
        Button(onClick = model::scan, enabled = server.running) { Text("Scan desktop QR") }
        if (scanner) PlaceholderScanner(onCode = model::scanned, onClose = model::closeScanner)
        server.notice?.let { Text(it) }
        notice?.let { Text(it) }
        Button(onClick = { model.refresh() }) { Text("Refresh devices") }
    }
}

private fun ruleName(rule: HostUnloadRule): String = when (rule) {
    HostUnloadRule.KeepLoaded -> "Keep loaded"
    is HostUnloadRule.AfterIdle -> "After ${rule.minutes} idle minutes"
    HostUnloadRule.WhenMemoryLow -> "When memory is low"
    HostUnloadRule.WhileServerModeOpen -> "While server mode is open"
}

@Composable
fun PlaceholderSettings(model: SettingsViewModel, onLicenses: () -> Unit) {
    val cleanup by model.cleanupEnabled.collectAsStateWithLifecycle()
    val available by model.systemAvailable.collectAsStateWithLifecycle()
    val haptics by model.hapticsEnabled.collectAsStateWithLifecycle()
    val notice by model.notice.collectAsStateWithLifecycle()
    Column {
        Text("System cleanup available: $available")
        Text("Use system cleanup")
        Checkbox(modifier = Modifier.semantics { contentDescription = "Use system cleanup" }, checked = cleanup, onCheckedChange = { model.setCleanup(it) })
        Text("Keyboard haptics")
        Checkbox(modifier = Modifier.semantics { contentDescription = "Keyboard haptics" }, checked = haptics, onCheckedChange = { model.setHaptics(it) })
        Text("Hushpen is licensed under AGPL-3.0-or-later.")
        Button(onClick = onLicenses) { Text("Open-source licenses") }
        notice?.let { Text(it) }
    }
}
