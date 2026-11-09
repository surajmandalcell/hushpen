package com.mandalsuraj.hushpen.server

import android.Manifest
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.view.PreviewView
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.viewmodel.compose.viewModel

@Composable
fun PlaceholderScanner(
    onCode: (String) -> Unit,
    onClose: () -> Unit,
    model: ScannerViewModel = viewModel(),
) {
    val context = LocalContext.current
    val owner = LocalLifecycleOwner.current
    val state by model.state.collectAsState()
    val request = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission(), model::permission)
    LaunchedEffect(model) { model.refreshPermission(context) }
    LaunchedEffect(state.code) { model.takeCode()?.let(onCode) }
    DisposableEffect(model) { onDispose { model.stop() } }
    Column {
        Text("Scan desktop QR code")
        state.notice?.let { Text(it) }
        if (state.permission) {
            AndroidView(factory = { PreviewView(it).also { view -> model.bind(it, owner, view) } })
        } else {
            Button(onClick = { request.launch(Manifest.permission.CAMERA) }) { Text("Allow Camera") }
        }
        Button(onClick = onClose) { Text("Close") }
    }
}
