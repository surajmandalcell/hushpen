package com.mandalsuraj.hushpen.screens

import androidx.compose.foundation.clickable
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.mandalsuraj.hushpen.R
import com.mandalsuraj.hushpen.ui.GroupFooter
import com.mandalsuraj.hushpen.ui.GroupItem
import com.mandalsuraj.hushpen.ui.ListGroup
import com.mandalsuraj.hushpen.ui.ListScreen
import com.mandalsuraj.hushpen.ui.Notices
import com.mandalsuraj.hushpen.ui.SectionHeader
import com.mandalsuraj.hushpen.ui.ToggleItem

@Composable
fun SettingsScreen(model: SettingsViewModel, onLicenses: () -> Unit, onBack: () -> Unit) {
    val cleanup by model.cleanupEnabled.collectAsStateWithLifecycle()
    val available by model.systemAvailable.collectAsStateWithLifecycle()
    val haptics by model.hapticsEnabled.collectAsStateWithLifecycle()
    val notice by model.notice.collectAsStateWithLifecycle()

    ListScreen(title = "Settings", onBack = onBack) {
        Notices(listOfNotNull(notice))
        SectionHeader("Cleanup")
        ListGroup {
            row { shape ->
                ToggleItem(
                    shape = shape,
                    headline = "Use system cleanup",
                    supporting = when {
                        !available -> "No system model on this phone. Rule cleanup still works."
                        cleanup -> "On. Uses the model built into this phone."
                        else -> "Uses the model built into this phone"
                    },
                    supportingLines = 2,
                    icon = R.drawable.ms_auto_awesome,
                    checked = cleanup,
                    onCheckedChange = { model.setCleanup(it) },
                )
            }
        }
        GroupFooter("The system model works only while Hushpen is open. At other times, rules clean the text.")
        SectionHeader("Keyboard")
        ListGroup {
            row { shape ->
                ToggleItem(
                    shape = shape,
                    headline = "Haptics",
                    supporting = "Vibrate when you tap a key",
                    icon = R.drawable.ms_vibration,
                    checked = haptics,
                    onCheckedChange = { model.setHaptics(it) },
                )
            }
        }
        SectionHeader("About")
        ListGroup {
            row { shape ->
                GroupItem(
                    shape = shape,
                    headline = "Open-source licenses",
                    supporting = "Hushpen is licensed under AGPL-3.0-or-later.",
                    supportingLines = 2,
                    icon = R.drawable.ms_license,
                    modifier = Modifier.clickable(onClick = onLicenses),
                )
            }
        }
    }
}
