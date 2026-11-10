package com.mandalsuraj.hushpen.screens

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.BatteryManager
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.LocalActivity
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import com.mandalsuraj.hushpen.R
import com.mandalsuraj.hushpen.core.HostUnloadRule
import com.mandalsuraj.hushpen.server.ServerState
import com.mandalsuraj.hushpen.ui.readable

private val Card = Color(0xFF16181C)
private val Label = Color.White.copy(alpha = .48f)
private val Value = Color.White.copy(alpha = .8f)
private val Serving = Color(0xFF30D158)
private val Charge = Color(0xFFFFB340)
private val Problem = Color(0xFFFF9C8F)

/** The dim, always-black screen a phone shows on a desk while it serves desktops. */
@Composable
fun ServerScreen(
    server: ServerState,
    starting: Boolean,
    rule: HostUnloadRule,
    speechModel: String,
    cleanup: String,
    desktops: List<String>,
    onStop: () -> Unit,
    onClose: () -> Unit,
) {
    val activity = LocalActivity.current as? ComponentActivity
    DisposableEffect(activity) {
        val dark = SystemBarStyle.dark(android.graphics.Color.TRANSPARENT)
        activity?.enableEdgeToEdge(statusBarStyle = dark, navigationBarStyle = dark)
        onDispose { activity?.enableEdgeToEdge() }
    }
    val plugged = rememberPluggedIn()
    val (status, statusColor) = when {
        server.running -> "Serving" to Serving
        starting -> "Starting" to Label
        server.notice != null -> "Stopped" to Problem
        else -> "Off" to Label
    }
    Box(Modifier.fillMaxSize().background(Color.Black).windowInsetsPadding(WindowInsets.safeDrawing)) {
        Column(Modifier.fillMaxSize().padding(horizontal = 18.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Spacer(Modifier.weight(0.6f))
            Surface(color = Card, shape = RoundedCornerShape(28.dp), modifier = Modifier.fillMaxWidth()) {
                Column {
                    Row(
                        Modifier.fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 20.dp, bottom = 14.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text("Server mode", color = Color.White.copy(alpha = .82f), fontSize = 22.sp, modifier = Modifier.weight(1f).semantics { heading() })
                        Box(Modifier.size(8.dp).background(statusColor, CircleShape))
                        Text(status, color = statusColor, fontSize = 15.sp, fontWeight = FontWeight.Medium, modifier = Modifier.padding(start = 7.dp))
                    }
                    StatRow("Port", server.port?.toString() ?: if (starting) "Starting" else "None")
                    StatRow("Speech model", speechModel)
                    StatRow("Cleanup", cleanup)
                    StatRow("Desktops", desktops.ifEmpty { listOf("None yet") }.joinToString())
                    StatRow("Unload", ruleName(rule))
                    server.notice?.let { NoteRow(R.drawable.ms_error, readable(it), Problem) }
                    if (!plugged) NoteRow(R.drawable.ms_bolt_fill, "Plug in to keep serving", Charge)
                }
            }
            Spacer(Modifier.weight(1f))
            Text(
                if (rule == HostUnloadRule.WhileServerModeOpen) "Server mode stops when you leave Hushpen." else "Keeps serving with the screen off.",
                color = Color.White.copy(alpha = .4f),
                fontSize = 14.sp,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(horizontal = 12.dp),
            )
            Spacer(Modifier.height(28.dp))
            val serving = server.running || starting
            Button(
                onClick = if (serving) onStop else onClose,
                colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF2B2D33), contentColor = Color.White.copy(alpha = .85f)),
                shape = RoundedCornerShape(28.dp),
                contentPadding = PaddingValues(horizontal = 34.dp),
                modifier = Modifier.height(56.dp),
            ) {
                Text(if (serving) "Stop Server mode" else "Close", fontSize = 16.sp, fontWeight = FontWeight.Medium)
            }
            Spacer(Modifier.height(36.dp))
        }
    }
}

@Composable
private fun StatRow(label: String, value: String) {
    Row(
        Modifier.fillMaxWidth().height(50.dp).hairline().padding(horizontal = 20.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(label, color = Label, fontSize = 15.sp)
        Spacer(Modifier.weight(1f))
        Text(value, color = Value, fontSize = 15.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

@Composable
private fun NoteRow(icon: Int, text: String, color: Color) {
    Row(
        Modifier.fillMaxWidth().height(50.dp).hairline().padding(horizontal = 20.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Icon(painterResource(icon), contentDescription = null, tint = color, modifier = Modifier.size(16.dp))
        Text(text, color = color, fontSize = 15.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

private fun Modifier.hairline() = drawBehind {
    drawLine(Color.White.copy(alpha = .07f), Offset(0f, 0f), Offset(size.width, 0f), strokeWidth = 1.dp.toPx())
}

@Composable
private fun rememberPluggedIn(): Boolean {
    val context = LocalContext.current
    var plugged by remember { mutableStateOf(true) }
    DisposableEffect(context) {
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) {
                plugged = intent.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) != 0
            }
        }
        ContextCompat.registerReceiver(context, receiver, IntentFilter(Intent.ACTION_BATTERY_CHANGED), ContextCompat.RECEIVER_NOT_EXPORTED)
            ?.let { receiver.onReceive(context, it) }
        onDispose { context.unregisterReceiver(receiver) }
    }
    return plugged
}
