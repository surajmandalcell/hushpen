package com.mandalsuraj.hushpen.screens

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.mandalsuraj.hushpen.R
import com.mandalsuraj.hushpen.core.HostUnloadRule
import com.mandalsuraj.hushpen.core.PairedDesktop
import com.mandalsuraj.hushpen.server.NearbyPairingSheet
import com.mandalsuraj.hushpen.server.PairingSheet
import com.mandalsuraj.hushpen.server.TypeCodeSheet
import com.mandalsuraj.hushpen.ui.GroupItem
import com.mandalsuraj.hushpen.ui.ListGroup
import com.mandalsuraj.hushpen.ui.ListScreen
import com.mandalsuraj.hushpen.ui.Notices
import com.mandalsuraj.hushpen.ui.SectionHeader
import com.mandalsuraj.hushpen.ui.ToggleItem

@Composable
fun DevicesScreen(model: DevicesViewModel, onServing: (Boolean) -> Unit, onBack: () -> Unit) {
    val server by model.server.collectAsStateWithLifecycle()
    val clients by model.pairedClients.collectAsStateWithLifecycle()
    val rules by model.allowedRules.collectAsStateWithLifecycle()
    val rule by model.unloadRule.collectAsStateWithLifecycle()
    val scanner by model.scannerOpen.collectAsStateWithLifecycle()
    val listening by model.listening.collectAsStateWithLifecycle()
    val idleMinutes by model.idleMinutes.collectAsStateWithLifecycle()
    val notice by model.notice.collectAsStateWithLifecycle()
    var choosingRule by rememberSaveable { mutableStateOf(false) }
    var forgetting by remember { mutableStateOf<PairedDesktop?>(null) }

    ListScreen(
        title = "Devices",
        onBack = onBack,
        actions = {
            IconButton(onClick = { model.refresh() }) {
                Icon(painterResource(R.drawable.ms_refresh), contentDescription = "Refresh")
            }
        },
        floatingActionButton = {
            if (server.running) {
                ExtendedFloatingActionButton(
                    onClick = model::scan,
                    icon = { Icon(painterResource(R.drawable.ms_qr_code_scanner), contentDescription = null) },
                    text = { Text("Pair desktop") },
                )
            }
        },
    ) {
        Notices(listOfNotNull(notice, server.notice))
        ListGroup {
            row { shape ->
                ToggleItem(
                    shape = shape,
                    headline = "Server mode",
                    supporting = if (server.running) "Serving on port ${server.port}" else "Off",
                    icon = R.drawable.ms_sensors,
                    checked = server.running,
                    onCheckedChange = onServing,
                )
            }
            row { shape ->
                GroupItem(
                    shape = shape,
                    headline = "Unload models",
                    supporting = ruleName(rule),
                    icon = R.drawable.ms_timer,
                    modifier = Modifier.clickable(enabled = rules.isNotEmpty()) {
                        (rule as? HostUnloadRule.AfterIdle)?.let { model.setIdleMinutes(it.minutes.toString()) }
                        choosingRule = true
                    },
                )
            }
        }
        if (server.running && !listening) {
            ListGroup {
                row { shape ->
                    GroupItem(
                        shape = shape,
                        headline = if (clients.isEmpty()) "Try again" else "Pair another computer",
                        supporting = "Let a computer find this phone",
                        icon = R.drawable.ms_sensors,
                        modifier = Modifier.clickable { model.reopenPairing() },
                    )
                }
            }
        }
        SectionHeader("Paired desktops")
        ListGroup {
            if (clients.isEmpty()) {
                row { shape ->
                    GroupItem(
                        shape = shape,
                        headline = "No paired desktops.",
                        supporting = if (server.running) {
                            "On your computer, open Add a Device and pick this phone."
                        } else {
                            "Turn on Server mode, then pair from your computer."
                        },
                        supportingLines = 2,
                        icon = R.drawable.ms_computer,
                    )
                }
            }
            clients.forEach { desktop ->
                row { shape ->
                    GroupItem(
                        shape = shape,
                        headline = desktop.name,
                        supporting = "Paired",
                        icon = R.drawable.ms_computer,
                        trailing = {
                            IconButton(onClick = { forgetting = desktop }) {
                                Icon(painterResource(R.drawable.ms_delete), contentDescription = "Forget ${desktop.name}")
                            }
                        },
                    )
                }
            }
        }
    }

    val typing by model.typing.collectAsStateWithLifecycle()
    val codeError by model.codeError.collectAsStateWithLifecycle()
    val busy by model.busy.collectAsStateWithLifecycle()
    if (scanner) {
        PairingSheet(
            onCode = model::scanned,
            onTypeCode = model::typeCode,
            onClose = model::closeScanner,
        )
    }
    if (typing) {
        TypeCodeSheet(busy = busy, error = codeError, onPair = model::pairCode, onClose = model::closeTyping)
    }
    NearbyPairing(model)
    if (choosingRule) {
        UnloadRuleDialog(
            rules = rules,
            selected = rule,
            minutes = idleMinutes,
            onMinutes = model::setIdleMinutes,
            onSave = { choice ->
                if (choice is HostUnloadRule.AfterIdle) model.applyIdleMinutes() else model.setRule(choice)
                choosingRule = false
            },
            onDismiss = { choosingRule = false },
        )
    }
    forgetting?.let { desktop ->
        AlertDialog(
            onDismissRequest = { forgetting = null },
            title = { Text("Forget ${desktop.name}?") },
            text = { Text("This desktop can no longer use this phone's models. You can pair it again later.") },
            confirmButton = {
                TextButton(onClick = {
                    model.forget(desktop.id)
                    forgetting = null
                }) { Text("Forget") }
            },
            dismissButton = { TextButton(onClick = { forgetting = null }) { Text("Cancel") } },
        )
    }
}

/** Opens the Nearby window while this screen is in front, and shows the number to compare. */
@Composable
fun NearbyPairing(model: DevicesViewModel) {
    val comparing by model.comparing.collectAsStateWithLifecycle()
    DisposableEffect(model) {
        model.watch()
        onDispose { model.unwatch() }
    }
    comparing?.let { NearbyPairingSheet(number = it, onAnswer = model::answer) }
}

@Composable
private fun UnloadRuleDialog(
    rules: List<HostUnloadRule>,
    selected: HostUnloadRule,
    minutes: String,
    onMinutes: (String) -> Unit,
    onSave: (HostUnloadRule) -> Unit,
    onDismiss: () -> Unit,
) {
    var choice by remember { mutableStateOf(rules.firstOrNull { it::class == selected::class } ?: selected) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Unload models") },
        text = {
            Column(Modifier.selectableGroup()) {
                Text(
                    "Free this phone's memory when desktops stop using its models.",
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.padding(bottom = 8.dp),
                )
                rules.forEach { rule ->
                    val on = rule::class == choice::class
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .heightIn(min = 48.dp)
                            .selectable(selected = on, role = Role.RadioButton) { choice = rule },
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(selected = on, onClick = null)
                        Spacer(Modifier.width(16.dp))
                        Text(ruleChoice(rule), style = MaterialTheme.typography.bodyLarge)
                    }
                    if (on && rule is HostUnloadRule.AfterIdle) {
                        OutlinedTextField(
                            value = minutes,
                            onValueChange = onMinutes,
                            label = { Text("Idle minutes") },
                            singleLine = true,
                            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                            modifier = Modifier.fillMaxWidth().padding(start = 40.dp, bottom = 8.dp),
                        )
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = { onSave(choice) }) { Text("Save") } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}
