package com.mandalsuraj.hushpen.server

import android.Manifest
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.view.PreviewView
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.mandalsuraj.hushpen.R
import kotlinx.coroutines.launch

/** The camera in a bottom sheet over Devices, with a pairing link as the fallback. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PairingSheet(uri: String, onUri: (String) -> Unit, onCode: (String) -> Unit, onPair: () -> Unit, onClose: () -> Unit) {
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    var manual by rememberSaveable { mutableStateOf(false) }
    val clipboard = LocalClipboard.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    ModalBottomSheet(onDismissRequest = onClose, sheetState = sheet) {
        Column(
            Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, bottom = 16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text("Pair a desktop", style = MaterialTheme.typography.headlineSmall)
            Text(
                if (manual) "Paste the pairing link from Hushpen on your computer." else "Point the camera at the code in Hushpen on your computer.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(top = 8.dp, start = 12.dp, end = 12.dp),
            )
            if (!manual) {
                CameraScanner(onCode = onCode, modifier = Modifier.padding(top = 20.dp).fillMaxWidth().height(330.dp))
                TextButton(onClick = { manual = true }, modifier = Modifier.padding(top = 12.dp)) {
                    Icon(painterResource(R.drawable.ms_link), contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(8.dp))
                    Text("Enter link instead")
                }
            } else {
                OutlinedTextField(
                    value = uri,
                    onValueChange = onUri,
                    label = { Text("Pairing link") },
                    placeholder = { Text("hushpen-pair://v1?...") },
                    singleLine = true,
                    trailingIcon = {
                        IconButton(onClick = {
                            scope.launch {
                                clipboard.getClipEntry()?.clipData?.takeIf { it.itemCount > 0 }
                                    ?.getItemAt(0)?.coerceToText(context)?.toString()?.trim()?.let(onUri)
                            }
                        }) {
                            Icon(painterResource(R.drawable.ms_content_paste), contentDescription = "Paste")
                        }
                    },
                    supportingText = { Text("Copy it on your computer: Devices, Add device, Phone.") },
                    modifier = Modifier.fillMaxWidth().padding(top = 20.dp),
                )
                Row(
                    Modifier.fillMaxWidth().padding(top = 12.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
                ) {
                    TextButton(onClick = { manual = false }) {
                        Icon(painterResource(R.drawable.ms_qr_code_scanner), contentDescription = null, modifier = Modifier.size(18.dp))
                        Spacer(Modifier.width(8.dp))
                        Text("Scan instead")
                    }
                    Button(onClick = onPair, enabled = uri.isNotBlank()) { Text("Pair") }
                }
            }
        }
    }
}

/** The camera preview that reads the desktop QR code. Pairing payloads stay opaque here. */
@Composable
fun CameraScanner(onCode: (String) -> Unit, modifier: Modifier = Modifier, model: ScannerViewModel = viewModel()) {
    val context = LocalContext.current
    val owner = LocalLifecycleOwner.current
    val state by model.state.collectAsStateWithLifecycle()
    val request = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission(), model::permission)
    LaunchedEffect(model) { model.refreshPermission(context) }
    LaunchedEffect(state.code) { model.takeCode()?.let(onCode) }
    DisposableEffect(model) { onDispose { model.stop() } }
    Column(modifier, horizontalAlignment = Alignment.CenterHorizontally) {
        Box(
            Modifier
                .fillMaxWidth()
                .weight(1f)
                .clip(RoundedCornerShape(24.dp))
                .background(Color.Black)
                .semantics { contentDescription = "Camera preview" },
            contentAlignment = Alignment.Center,
        ) {
            if (state.permission) {
                AndroidView(
                    factory = {
                        PreviewView(it).apply {
                            // A texture view follows the rounded clip; a surface view would not.
                            implementationMode = PreviewView.ImplementationMode.COMPATIBLE
                            scaleType = PreviewView.ScaleType.FILL_CENTER
                        }.also { view -> model.bind(it, owner, view) }
                    },
                    modifier = Modifier.fillMaxSize(),
                )
                Viewfinder(Modifier.size(190.dp))
            } else {
                Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(16.dp),
                    modifier = Modifier.padding(24.dp),
                ) {
                    Icon(painterResource(R.drawable.ms_photo_camera), contentDescription = null, tint = Color.White.copy(alpha = .85f), modifier = Modifier.size(32.dp))
                    Text(
                        "Allow Camera to scan the desktop QR code.",
                        color = Color.White.copy(alpha = .85f),
                        style = MaterialTheme.typography.bodyMedium,
                        textAlign = TextAlign.Center,
                    )
                    Button(onClick = { request.launch(Manifest.permission.CAMERA) }) { Text("Allow camera") }
                }
            }
        }
        state.notice?.takeIf { state.permission }?.let {
            Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(top = 8.dp))
        }
    }
}

@Composable
private fun Viewfinder(modifier: Modifier) {
    Canvas(modifier) {
        val stroke = 4.dp.toPx()
        val arm = 44.dp.toPx()
        val radius = 24.dp.toPx()
        val inset = stroke / 2
        val corner = Path().apply {
            moveTo(inset, arm)
            lineTo(inset, radius)
            arcTo(Rect(inset, inset, inset + radius * 2 - inset, inset + radius * 2 - inset), 180f, 90f, false)
            lineTo(arm, inset)
        }
        val style = Stroke(width = stroke, cap = StrokeCap.Round)
        for (x in listOf(1f, -1f)) for (y in listOf(1f, -1f)) {
            scale(x, y, pivot = Offset(size.width / 2, size.height / 2)) { drawPath(corner, Color.White, style = style) }
        }
    }
}
