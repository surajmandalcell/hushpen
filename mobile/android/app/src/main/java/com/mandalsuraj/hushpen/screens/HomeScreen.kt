package com.mandalsuraj.hushpen.screens

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.mandalsuraj.hushpen.DownloadState
import com.mandalsuraj.hushpen.R
import com.mandalsuraj.hushpen.core.Model
import com.mandalsuraj.hushpen.core.ModelKind
import com.mandalsuraj.hushpen.core.ModelStatus
import com.mandalsuraj.hushpen.ui.GroupItem
import com.mandalsuraj.hushpen.ui.HushpenSwitch
import com.mandalsuraj.hushpen.ui.ListGroup
import com.mandalsuraj.hushpen.ui.ListScreen
import com.mandalsuraj.hushpen.ui.Notices
import com.mandalsuraj.hushpen.ui.ReadyMark

@Composable
fun HomeScreen(
    onboarding: OnboardingViewModel,
    models: ModelsViewModel,
    devices: DevicesViewModel,
    onMicrophone: () -> Unit,
    onKeyboardSettings: () -> Unit,
    onKeyboardPicker: () -> Unit,
    onServing: (Boolean) -> Unit,
    onOpen: (Screen) -> Unit,
) {
    val setup by onboarding.state.collectAsStateWithLifecycle()
    val catalog by models.models.collectAsStateWithLifecycle()
    val selected by models.selectedModel.collectAsStateWithLifecycle()
    val download by models.download.collectAsStateWithLifecycle()
    val modelNotice by models.notice.collectAsStateWithLifecycle()
    val server by devices.server.collectAsStateWithLifecycle()
    val clients by devices.pairedClients.collectAsStateWithLifecycle()
    val deviceNotice by devices.notice.collectAsStateWithLifecycle()
    val speech = catalog.filter { it.kind == ModelKind.SPEECH }
    val current = speech.firstOrNull { it.id == selected } ?: speech.firstOrNull { it.recommended }

    ListScreen(title = "Hushpen") {
        Notices(listOfNotNull(modelNotice, deviceNotice, server.notice))
        ListGroup {
            row { shape ->
                val ready = setup.keyboardEnabled && setup.keyboardSelected
                val action = if (setup.keyboardEnabled) onKeyboardPicker else onKeyboardSettings
                GroupItem(
                    shape = shape,
                    headline = "Keyboard",
                    supporting = when {
                        ready -> "On and selected"
                        setup.keyboardEnabled -> "On, not selected"
                        else -> "Off"
                    },
                    icon = R.drawable.ms_keyboard,
                    modifier = Modifier.clickable(onClick = action),
                    trailing = {
                        if (ready) ReadyMark() else FilledTonalButton(onClick = action) {
                            Text(if (setup.keyboardEnabled) "Choose" else "Turn on")
                        }
                    },
                )
            }
            row { shape ->
                GroupItem(
                    shape = shape,
                    headline = "Microphone",
                    supporting = if (setup.microphoneGranted) "Allowed" else "Not allowed",
                    icon = R.drawable.ms_mic,
                    modifier = if (setup.microphoneGranted) Modifier else Modifier.clickable(onClick = onMicrophone),
                    trailing = {
                        if (setup.microphoneGranted) ReadyMark() else FilledTonalButton(onClick = onMicrophone) { Text("Allow") }
                    },
                )
            }
            row { shape -> SpeechModelSetup(shape, current, download, models, onOpen) }
        }
        Spacer(Modifier.height(16.dp))
        ListGroup {
            row { shape ->
                GroupItem(
                    shape = shape,
                    headline = "Server mode",
                    supporting = if (server.running) "Serving on port ${server.port}" else "Share models with your computer",
                    icon = R.drawable.ms_sensors,
                    modifier = Modifier.clickable { if (server.running) onOpen(Screen.Server) else onServing(true) },
                    trailing = { HushpenSwitch(server.running, onCheckedChange = onServing) },
                )
            }
        }
        Spacer(Modifier.height(16.dp))
        ListGroup {
            row { shape ->
                GroupItem(
                    shape = shape,
                    headline = "Models",
                    supporting = if (catalog.isEmpty()) {
                        "Speech and cleanup models"
                    } else {
                        val count = if (speech.size == 1) "1 speech model" else "${speech.size} speech models"
                        listOfNotNull(count, "text cleanup".takeIf { catalog.any { it.kind == ModelKind.SYSTEM_CLEANUP || it.kind == ModelKind.CLEANUP } }).joinToString()
                    },
                    icon = R.drawable.ms_deployed_code,
                    modifier = Modifier.clickable { onOpen(Screen.Models) },
                )
            }
            row { shape ->
                GroupItem(
                    shape = shape,
                    headline = "Devices",
                    supporting = if (clients.isEmpty()) "No paired desktops" else clients.joinToString(transform = ::desktopName),
                    icon = R.drawable.ms_computer,
                    modifier = Modifier.clickable { onOpen(Screen.Devices) },
                )
            }
            row { shape ->
                GroupItem(
                    shape = shape,
                    headline = "Settings",
                    supporting = "Cleanup, haptics, licenses",
                    icon = R.drawable.ms_settings,
                    modifier = Modifier.clickable { onOpen(Screen.Settings) },
                )
            }
        }
    }
}

@Composable
private fun SpeechModelSetup(shape: Shape, current: Model?, download: DownloadState, models: ModelsViewModel, onOpen: (Screen) -> Unit) {
    val active = current != null && download.modelId == current.id
    val fraction = if (download.total > 0u) (download.done.toDouble() / download.total.toDouble()).toFloat() else 0f
    GroupItem(
        shape = shape,
        headline = "Speech model",
        supporting = when {
            current == null -> "Loading models"
            active && download.verifying -> "${current.name} · Checking"
            active -> "${current.name} · Downloading ${(fraction * 100).toInt()}%"
            current.status == ModelStatus.READY -> "${current.name} · Ready"
            current.status == ModelStatus.NEEDS_VERIFICATION -> "${current.name} · Not checked"
            current.status == ModelStatus.DAMAGED -> "${current.name} · Damaged"
            else -> "${current.name} · Not downloaded"
        },
        icon = R.drawable.ms_graphic_eq,
        modifier = Modifier.clickable { onOpen(Screen.Models) },
        trailing = {
            when {
                current == null -> Unit
                active -> CircularProgressIndicator(progress = { fraction }, modifier = Modifier.size(28.dp))
                current.status == ModelStatus.READY -> ReadyMark()
                current.status == ModelStatus.NEEDS_VERIFICATION -> FilledTonalButton(onClick = { models.verify(current.id) }) { Text("Verify") }
                else -> FilledTonalButton(onClick = { models.download(current.id) }, enabled = download.modelId == null) { Text("Download") }
            }
        },
    )
}
