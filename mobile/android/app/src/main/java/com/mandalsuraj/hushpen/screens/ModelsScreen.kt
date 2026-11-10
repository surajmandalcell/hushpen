package com.mandalsuraj.hushpen.screens

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.mandalsuraj.hushpen.DownloadState
import com.mandalsuraj.hushpen.R
import com.mandalsuraj.hushpen.core.Model
import com.mandalsuraj.hushpen.core.ModelKind
import com.mandalsuraj.hushpen.core.ModelStatus
import com.mandalsuraj.hushpen.ui.GroupFooter
import com.mandalsuraj.hushpen.ui.GroupItem
import com.mandalsuraj.hushpen.ui.ListGroup
import com.mandalsuraj.hushpen.ui.ListScreen
import com.mandalsuraj.hushpen.ui.Notices
import com.mandalsuraj.hushpen.ui.ReadyMark
import com.mandalsuraj.hushpen.ui.SectionHeader

@Composable
fun ModelsScreen(model: ModelsViewModel, settings: SettingsViewModel, onBack: () -> Unit) {
    val catalog by model.models.collectAsStateWithLifecycle()
    val selected by model.selectedModel.collectAsStateWithLifecycle()
    val download by model.download.collectAsStateWithLifecycle()
    val notice by model.notice.collectAsStateWithLifecycle()
    val cleanup by settings.cleanupEnabled.collectAsStateWithLifecycle()
    val available by settings.systemAvailable.collectAsStateWithLifecycle()
    val speech = catalog.filter { it.kind == ModelKind.SPEECH }
    val system = catalog.filter { it.kind == ModelKind.SYSTEM_CLEANUP }

    ListScreen(
        title = "Models",
        onBack = onBack,
        actions = {
            IconButton(onClick = { model.refresh() }) {
                Icon(painterResource(R.drawable.ms_refresh), contentDescription = "Refresh")
            }
        },
    ) {
        Notices(listOfNotNull(notice))
        SectionHeader("Speech to Text")
        ListGroup {
            speech.forEach { entry ->
                row { shape -> SpeechModelRow(shape, entry, selected, download, model) }
            }
        }
        GroupFooter("Tap a downloaded model to use it for the keyboard and paired desktops.")
        if (system.isNotEmpty()) {
            SectionHeader("Text cleanup")
            ListGroup {
                system.forEach { entry ->
                    row { shape ->
                        GroupItem(
                            shape = shape,
                            headline = entry.name,
                            supporting = when {
                                !cleanup -> "Off. Turn it on in Settings."
                                available -> "Ready. Works while Hushpen is open."
                                else -> "Not available on this phone. Rule cleanup still works."
                            },
                            supportingLines = 2,
                            icon = R.drawable.ms_auto_awesome,
                        )
                    }
                }
            }
            GroupFooter("Built into this phone. Nothing to download.")
        }
    }
}

@Composable
private fun SpeechModelRow(shape: Shape, entry: Model, selected: String, download: DownloadState, model: ModelsViewModel) {
    val active = download.modelId == entry.id
    val ready = entry.status == ModelStatus.READY
    val inUse = ready && entry.id == selected
    val fraction = if (download.total > 0u) (download.done.toDouble() / download.total.toDouble()).toFloat() else 0f
    val status = when {
        active && download.verifying -> "Checking the file"
        active -> "Downloading ${sizeLabel(download.done)} of ${sizeLabel(download.total)}"
        else -> listOfNotNull(
            sizeLabel(entry.bytes),
            "Recommended".takeIf { entry.recommended },
            when (entry.status) {
                ModelStatus.READY -> if (inUse) "In use" else "Downloaded"
                ModelStatus.NEEDS_VERIFICATION -> "Not checked"
                ModelStatus.DAMAGED -> "Damaged"
                else -> null
            },
        ).joinToString(" · ")
    }
    GroupItem(
        shape = shape,
        headline = entry.name,
        supporting = "${speechPurpose(entry)}\n$status",
        supportingLines = 2,
        supportingColor = if (entry.status == ModelStatus.DAMAGED && !active) MaterialTheme.colorScheme.error else Color.Unspecified,
        icon = R.drawable.ms_graphic_eq,
        modifier = if (ready && !inUse) Modifier.clickable { model.select(entry.id) } else Modifier,
        below = if (active) {
            {
                val bar = Modifier.fillMaxWidth().padding(top = 8.dp)
                if (download.verifying || download.total == 0uL) LinearProgressIndicator(bar) else LinearProgressIndicator(progress = { fraction }, modifier = bar)
            }
        } else null,
        trailing = {
            when {
                active -> IconButton(onClick = { model.cancelDownload() }) {
                    Icon(painterResource(R.drawable.ms_close), contentDescription = "Cancel download")
                }
                inUse -> ReadyMark("In use")
                ready -> TextButton(onClick = { model.select(entry.id) }) { Text("Use") }
                entry.status == ModelStatus.NEEDS_VERIFICATION -> TextButton(onClick = { model.verify(entry.id) }) { Text("Verify") }
                else -> IconButton(onClick = { model.download(entry.id) }, enabled = download.modelId == null) {
                    Icon(painterResource(R.drawable.ms_download), contentDescription = "Download ${entry.name}")
                }
            }
        },
    )
}
