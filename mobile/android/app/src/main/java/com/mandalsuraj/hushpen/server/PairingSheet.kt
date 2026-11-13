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
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
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

/** The camera in a bottom sheet over Devices. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PairingSheet(onCode: (String) -> Unit, onTypeCode: () -> Unit, onClose: () -> Unit) {
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ModalBottomSheet(onDismissRequest = onClose, sheetState = sheet) {
        Column(
            Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, bottom = 16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text("Pair a desktop", style = MaterialTheme.typography.headlineSmall)
            Text(
                "Point the camera at the code in Hushpen on your computer.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(top = 8.dp, start = 12.dp, end = 12.dp),
            )
            CameraScanner(onCode = onCode, modifier = Modifier.padding(top = 20.dp).fillMaxWidth().height(330.dp))
            OutlinedButton(onClick = onTypeCode, modifier = Modifier.padding(top = 16.dp)) { Text("Type code") }
        }
    }
}

/** The fallback for a computer whose QR cannot be scanned: its 8-digit code, and where to find it. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TypeCodeSheet(busy: Boolean, error: String?, onPair: (code: String, address: String) -> Unit, onClose: () -> Unit) {
    var code by rememberSaveable { mutableStateOf("") }
    var address by rememberSaveable { mutableStateOf("") }
    ModalBottomSheet(onDismissRequest = onClose, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text("Type code", style = MaterialTheme.typography.headlineSmall)
            Text(
                "Type the code shown in Hushpen on your computer. It works once, for 2 minutes.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            OutlinedTextField(
                value = code,
                onValueChange = { value -> code = value.filter(Char::isDigit).take(8) },
                visualTransformation = CodeSpacing,
                label = { Text("Code") },
                placeholder = { Text("0000 0000") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                modifier = Modifier.fillMaxWidth().semantics { contentDescription = "Pairing code" },
            )
            OutlinedTextField(
                value = address,
                onValueChange = { address = it },
                label = { Text("Address") },
                placeholder = { Text("Automatic") },
                supportingText = { Text("Leave on Automatic unless your computer is not found. Then type host:port.") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
                modifier = Modifier.fillMaxWidth(),
            )
            error?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium) }
            Button(
                onClick = { onPair(code, address) },
                enabled = code.length == 8 && !busy,
                modifier = Modifier.fillMaxWidth(),
            ) { Text(if (busy) "Pairing" else "Pair") }
        }
    }
}

/** The code reads as 4829 1637 while the field keeps only its digits, so the cursor never jumps. */
private val CodeSpacing = VisualTransformation { text ->
    val digits = text.text
    TransformedText(
        AnnotatedString(if (digits.length > 4) digits.take(4) + " " + digits.drop(4) else digits),
        object : OffsetMapping {
            override fun originalToTransformed(offset: Int) = if (offset > 4) offset + 1 else offset
            override fun transformedToOriginal(offset: Int) = if (offset > 4) offset - 1 else offset
        },
    )
}

/** Both screens show this number while a computer pairs through Nearby. Pair on each, or Cancel. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NearbyPairingSheet(number: String, onAnswer: (number: String, approve: Boolean) -> Unit) {
    ModalBottomSheet(
        onDismissRequest = { onAnswer(number, false) },
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        Column(
            Modifier.fillMaxWidth().padding(start = 24.dp, end = 24.dp, bottom = 24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text("Pair with this computer?", style = MaterialTheme.typography.headlineSmall)
            Text(
                "It will use the models on this phone. Check that your computer shows the same number.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
            )
            Text(
                "${number.take(3)} ${number.drop(3)}",
                style = MaterialTheme.typography.displayMedium,
                modifier = Modifier.padding(vertical = 8.dp).semantics { contentDescription = "Pairing number" },
            )
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedButton(onClick = { onAnswer(number, false) }, modifier = Modifier.weight(1f)) { Text("Cancel") }
                Button(onClick = { onAnswer(number, true) }, modifier = Modifier.weight(1f)) { Text("Pair") }
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
