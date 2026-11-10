package com.mandalsuraj.hushpen.ime

import android.view.HapticFeedbackConstants
import android.view.View
import android.view.inputmethod.EditorInfo
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.input.pointer.AwaitPointerEventScope
import androidx.compose.ui.input.pointer.PointerId
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.semantics.text
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.mandalsuraj.hushpen.R
import com.mandalsuraj.hushpen.ui.readable
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.roundToInt

/** What the enter key shows for the focused editor. [HushpenInputMethod] decides what it does. */
@Immutable
data class EnterKey(val description: String, val icon: Int, val label: String? = null) {
    companion object {
        val Return = EnterKey("Enter", R.drawable.ms_keyboard_return)

        fun of(info: EditorInfo?): EnterKey {
            if (info == null || info.imeOptions and EditorInfo.IME_FLAG_NO_ENTER_ACTION != 0) return Return
            info.actionLabel?.toString()?.takeIf { it.isNotBlank() }?.let { return EnterKey(it, R.drawable.ms_keyboard_return, it) }
            return when (info.imeOptions and EditorInfo.IME_MASK_ACTION) {
                EditorInfo.IME_ACTION_GO -> EnterKey("Go", R.drawable.ms_arrow_forward)
                EditorInfo.IME_ACTION_SEARCH -> EnterKey("Search", R.drawable.ms_search)
                EditorInfo.IME_ACTION_SEND -> EnterKey("Send", R.drawable.ms_send)
                EditorInfo.IME_ACTION_NEXT -> EnterKey("Next", R.drawable.ms_keyboard_tab)
                EditorInfo.IME_ACTION_PREVIOUS -> EnterKey("Previous", R.drawable.ms_keyboard_tab_rtl)
                EditorInfo.IME_ACTION_DONE -> EnterKey("Done", R.drawable.ms_check)
                else -> Return
            }
        }
    }
}

class KeyboardActions(
    val type: (String) -> Unit,
    val backspace: () -> Unit,
    val enter: () -> Unit,
    val undo: () -> Unit,
    val openApp: () -> Unit,
)

/** A Gboard-style keyboard with the Hushpen voice bar in the row above the keys. */
@Composable
fun HushpenKeyboard(model: KeyboardViewModel, enterKey: EnterKey, actions: KeyboardActions) {
    val state by model.state.collectAsState()
    val hapticsOn by model.hapticsEnabled.collectAsState()
    val colors = boardColors()
    val view = LocalView.current
    val haptics = remember(view) { Haptics(view) }
    haptics.enabled = hapticsOn
    var bubble by remember { mutableStateOf<Bubble?>(null) }
    var width by remember { mutableIntStateOf(0) }
    var capsLock by remember { mutableStateOf(false) }
    var lastShift by remember { mutableLongStateOf(0L) }
    var morePage by remember { mutableStateOf(false) }
    LaunchedEffect(state.symbols) { if (!state.symbols) morePage = false }
    val board = remember(colors, haptics, actions) { Board(colors, haptics, actions.type) { bubble = it } }
    val upper = state.shift || capsLock

    Box(
        Modifier
            .fillMaxWidth()
            .background(colors.board)
            .navigationBarsPadding()
            .onSizeChanged { width = it.width },
    ) {
        Column(Modifier.fillMaxWidth()) {
            Toolbar(state, model, board, actions)
            Column(
                Modifier
                    .fillMaxWidth()
                    .height(224.dp)
                    .padding(start = 3.dp, top = 3.dp, end = 3.dp, bottom = 5.dp),
            ) {
                if (!state.symbols) {
                    KeyRow {
                        "qwertyuiop".forEachIndexed { index, letter ->
                            val digit = ((index + 1) % 10).toString()
                            CharKey(board, letter.cased(upper), hint = digit)
                        }
                    }
                    KeyRow {
                        Spacer(Modifier.weight(.5f))
                        "asdfghjkl".forEach { CharKey(board, it.cased(upper)) }
                        Spacer(Modifier.weight(.5f))
                    }
                    KeyRow {
                        val icon = when {
                            capsLock -> R.drawable.ms_shift_lock
                            state.shift -> R.drawable.ms_shift_fill
                            else -> R.drawable.ms_shift
                        }
                        FunctionKey(board, "Shift", weight = 1.5f, state = if (capsLock) "Caps lock" else if (state.shift) "On" else "Off", onTap = {
                            val now = System.currentTimeMillis()
                            when {
                                capsLock -> {
                                    capsLock = false
                                    if (state.shift) model.shift()
                                }
                                state.shift && now - lastShift < DOUBLE_TAP_MS -> {
                                    capsLock = true
                                    model.shift()
                                }
                                else -> model.shift()
                            }
                            lastShift = now
                        }) { KeyIcon(icon, colors.onFunction) }
                        "zxcvbnm".forEach { CharKey(board, it.cased(upper)) }
                        BackspaceKey(board, actions.backspace)
                    }
                } else {
                    val rows = if (morePage) MoreSymbols else Symbols
                    KeyRow { rows[0].forEach { CharKey(board, it) } }
                    KeyRow { rows[1].forEach { CharKey(board, it) } }
                    KeyRow {
                        FunctionKey(board, if (morePage) "Symbols" else "More symbols", weight = 1.5f, onTap = { morePage = !morePage }) {
                            KeyText(if (morePage) "?123" else "=\\<", colors.onFunction)
                        }
                        rows[2].forEach { CharKey(board, it) }
                        BackspaceKey(board, actions.backspace)
                    }
                }
                KeyRow {
                    FunctionKey(board, if (state.symbols) "Letters" else "Symbols", weight = 1.45f, onTap = model::symbols) {
                        KeyText(if (state.symbols) "ABC" else "?123", colors.onFunction)
                    }
                    CharKey(board, ",", function = true)
                    KeyCell(board, Press.Type(" ", bubble = false), "Space", colors.key, weight = 5.1f) {}
                    CharKey(board, ".", function = true)
                    KeyCell(board, Press.Tap(actions.enter), enterKey.description, colors.enter, weight = 1.45f, shape = CircleShape) {
                        val label = enterKey.label
                        if (label != null) KeyText(label, colors.onEnter) else KeyIcon(enterKey.icon, colors.onEnter)
                    }
                }
            }
        }
        bubble?.let { KeyBubble(it, width, colors) }
    }
}

private const val DOUBLE_TAP_MS = 350L
private const val REPEAT_MS = 60L
private val KeyShape = RoundedCornerShape(10.dp)
private val Symbols = listOf(
    listOf("1", "2", "3", "4", "5", "6", "7", "8", "9", "0"),
    listOf("@", "#", "$", "_", "&", "-", "+", "(", ")", "/"),
    listOf("*", "\"", "'", ":", ";", "!", "?"),
)
private val MoreSymbols = listOf(
    listOf("~", "`", "|", "•", "√", "π", "÷", "×", "¶", "∆"),
    listOf("£", "¢", "€", "¥", "^", "°", "=", "{", "}", "\\"),
    listOf("%", "©", "®", "™", "✓", "[", "]"),
)
private val FlowPhases = setOf("listening", "transcribing", "cleaning", "inserting", "cancelled")
private val FlowFill = Color(0xFF1F1F1F)
private val FlowEdge = Color(0xFF353535)
private val FlowButton = Color.White.copy(alpha = .14f)
private val FlowLabel = Color.White.copy(alpha = .85f)

private fun Char.cased(upper: Boolean) = if (upper) uppercase() else toString()

@Immutable
private class BoardColors(
    val board: Color,
    val key: Color,
    val label: Color,
    val hint: Color,
    val function: Color,
    val onFunction: Color,
    val enter: Color,
    val onEnter: Color,
    val accent: Color,
    val error: Color,
    val icon: Color,
    val bubble: Color,
    val pressed: Color,
)

/** Gboard's roles: a tinted board, bright letter keys, tonal function keys, and an accent enter key. */
@Composable
private fun boardColors(): BoardColors {
    val scheme = MaterialTheme.colorScheme
    val dark = isSystemInDarkTheme()
    return remember(scheme, dark) {
        if (dark) {
            BoardColors(
                board = scheme.surfaceContainerLow,
                key = scheme.surfaceContainerHighest,
                label = scheme.onSurface,
                hint = scheme.onSurfaceVariant,
                function = lerp(scheme.surfaceContainer, scheme.secondaryContainer, .6f),
                onFunction = scheme.onSecondaryContainer,
                enter = scheme.primary,
                onEnter = scheme.onPrimary,
                accent = scheme.primary,
                error = scheme.error,
                icon = scheme.onSurfaceVariant,
                bubble = lerp(scheme.surfaceContainerHighest, Color.White, .08f),
                pressed = Color.White.copy(alpha = .12f),
            )
        } else {
            BoardColors(
                board = scheme.surfaceContainerHigh,
                key = scheme.surfaceContainerLowest,
                label = scheme.onSurface,
                hint = scheme.onSurfaceVariant,
                function = scheme.secondaryContainer,
                onFunction = scheme.onSecondaryContainer,
                enter = lerp(scheme.primaryContainer, scheme.primary, .12f),
                onEnter = scheme.onPrimaryContainer,
                accent = scheme.primary,
                error = scheme.error,
                icon = scheme.onSurfaceVariant,
                bubble = scheme.surfaceContainerLowest,
                pressed = Color.Black.copy(alpha = .08f),
            )
        }
    }
}

private class Haptics(private val view: View) {
    var enabled = true

    fun tap() {
        if (enabled) view.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
    }

    fun hold() {
        if (enabled) view.performHapticFeedback(HapticFeedbackConstants.LONG_PRESS)
    }
}

private class Bubble(val label: String, val bounds: Rect)

private class Board(val colors: BoardColors, val haptics: Haptics, val type: (String) -> Unit, val showBubble: (Bubble?) -> Unit)

private sealed interface Press {
    class Type(val text: String, val alternate: String? = null, val bubble: Boolean = true) : Press
    class Tap(val action: () -> Unit) : Press
    class Repeat(val action: () -> Unit) : Press
}

@Composable
private fun ColumnScope.KeyRow(content: @Composable RowScope.() -> Unit) {
    Row(Modifier.weight(1f).fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, content = content)
}

@Composable
private fun RowScope.CharKey(board: Board, text: String, hint: String? = null, function: Boolean = false) {
    val colors = board.colors
    KeyCell(board, Press.Type(text, alternate = hint), text, if (function) colors.function else colors.key) {
        KeyText(text, if (function) colors.onFunction else colors.label, size = 23.sp, weight = FontWeight.Normal)
        if (hint != null) {
            Text(
                hint,
                color = colors.hint,
                fontSize = 10.sp,
                lineHeight = 10.sp,
                modifier = Modifier.align(Alignment.TopEnd).padding(top = 3.dp, end = 5.dp),
            )
        }
    }
}

@Composable
private fun RowScope.FunctionKey(
    board: Board,
    description: String,
    weight: Float,
    onTap: () -> Unit,
    state: String? = null,
    content: @Composable BoxScope.() -> Unit,
) {
    KeyCell(board, Press.Tap(onTap), description, board.colors.function, weight = weight, state = state, content = content)
}

@Composable
private fun RowScope.BackspaceKey(board: Board, backspace: () -> Unit) {
    KeyCell(board, Press.Repeat(backspace), "Backspace", board.colors.function, weight = 1.5f) {
        KeyIcon(R.drawable.ms_backspace, board.colors.onFunction)
    }
}

@Composable
private fun KeyText(text: String, color: Color, size: TextUnit = 15.sp, weight: FontWeight = FontWeight.Medium) {
    Text(text, color = color, fontSize = size, lineHeight = size, fontWeight = weight, maxLines = 1, overflow = TextOverflow.Clip)
}

@Composable
private fun KeyIcon(icon: Int, tint: Color) {
    Icon(painterResource(icon), contentDescription = null, tint = tint, modifier = Modifier.size(24.dp))
}

/** One key: its touch area fills the cell, so the gaps between keys still type. */
@Composable
private fun RowScope.KeyCell(
    board: Board,
    press: Press,
    description: String,
    fill: Color,
    weight: Float = 1f,
    shape: Shape = KeyShape,
    state: String? = null,
    content: @Composable BoxScope.() -> Unit,
) {
    var pressed by remember { mutableStateOf(false) }
    val bounds = remember { arrayOf(Rect.Zero) }
    val current by rememberUpdatedState(press)
    Box(
        Modifier
            .weight(weight)
            .fillMaxHeight()
            .clearAndSetSemantics {
                contentDescription = description
                role = Role.Button
                if (state != null) stateDescription = state
                onClick {
                    board.activate(current)
                    true
                }
            }
            .pointerInput(board) {
                awaitEachGesture {
                    val down = awaitFirstDown()
                    down.consume()
                    pressed = true
                    board.haptics.tap()
                    try {
                        track(board, current, down.id, bounds[0])
                    } finally {
                        pressed = false
                        board.showBubble(null)
                    }
                }
            }
            .padding(horizontal = 3.dp),
        contentAlignment = Alignment.Center,
    ) {
        Box(
            Modifier
                .fillMaxWidth()
                .height(46.dp)
                .onGloballyPositioned { bounds[0] = it.boundsInRoot() }
                .clip(shape)
                .background(if (pressed) board.colors.pressed.compositeOver(fill) else fill),
            contentAlignment = Alignment.Center,
            content = content,
        )
    }
}

private fun Board.activate(press: Press) = when (press) {
    is Press.Type -> type(press.text)
    is Press.Tap -> press.action()
    is Press.Repeat -> press.action()
}

private suspend fun AwaitPointerEventScope.track(board: Board, press: Press, id: PointerId, bounds: Rect) = with(board) {
    val longPress = viewConfiguration.longPressTimeoutMillis
    when (press) {
        is Press.Type -> {
            if (press.bubble) showBubble(Bubble(press.text, bounds))
            val released = withTimeoutOrNull(longPress) { awaitRelease(id) } != null
            val text = if (!released && press.alternate != null) {
                haptics.hold()
                showBubble(Bubble(press.alternate, bounds))
                awaitRelease(id)
                press.alternate
            } else {
                if (!released) awaitRelease(id)
                press.text
            }
            showBubble(null)
            type(text)
        }
        is Press.Tap -> {
            awaitRelease(id)
            press.action()
        }
        is Press.Repeat -> {
            press.action()
            if (withTimeoutOrNull(longPress) { awaitRelease(id) } == null) {
                do press.action() while (withTimeoutOrNull(REPEAT_MS) { awaitRelease(id) } == null)
            }
        }
    }
}

/** Waits until this pointer lifts, wherever it moved, so a sloppy tap still counts. */
private suspend fun AwaitPointerEventScope.awaitRelease(id: PointerId) {
    while (true) {
        val change = awaitPointerEvent().changes.firstOrNull { it.id == id } ?: return
        change.consume()
        if (!change.pressed) return
    }
}

@Composable
private fun KeyBubble(bubble: Bubble, width: Int, colors: BoardColors) {
    val density = LocalDensity.current
    val height = 52.dp
    val (x, y, bubbleWidth) = with(density) {
        val w = bubble.bounds.width + 12.dp.toPx()
        val left = (bubble.bounds.center.x - w / 2).coerceIn(0f, (width - w).coerceAtLeast(0f))
        val top = (bubble.bounds.top - height.toPx() - 2.dp.toPx()).coerceAtLeast(0f)
        Triple(left, top, w.toDp())
    }
    Surface(
        color = colors.bubble,
        shape = RoundedCornerShape(12.dp),
        shadowElevation = 3.dp,
        modifier = Modifier
            .offset { IntOffset(x.roundToInt(), y.roundToInt()) }
            .size(bubbleWidth, height)
            .clearAndSetSemantics {},
    ) {
        Box(contentAlignment = Alignment.Center) {
            Text(bubble.label, color = colors.label, fontSize = 30.sp, lineHeight = 30.sp)
        }
    }
}

@Composable
private fun Toolbar(state: KeyboardState, model: KeyboardViewModel, board: Board, actions: KeyboardActions) {
    val flow = state.phase in FlowPhases
    Row(
        Modifier
            .fillMaxWidth()
            .height(48.dp)
            .padding(horizontal = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        if (flow) Spacer(Modifier.size(44.dp, 40.dp)) else ToolbarButton(board, R.drawable.ms_settings, "Hushpen settings", onClick = actions.openApp)
        VoiceSlot(Modifier.weight(1f), state, model, board)
        if (flow) {
            Spacer(Modifier.size(44.dp, 40.dp))
        } else {
            ToolbarButton(board, R.drawable.ms_undo, "Undo", enabled = state.canUndo, tonal = state.canUndo, onClick = actions.undo)
        }
    }
}

@Composable
private fun ToolbarButton(board: Board, icon: Int, description: String, onClick: () -> Unit, enabled: Boolean = true, tonal: Boolean = false) {
    val colors = board.colors
    Box(
        Modifier
            .size(44.dp, 40.dp)
            .actionSemantics(description, enabled, onClick)
            .clip(CircleShape)
            .background(if (tonal) colors.function else Color.Transparent)
            .clickable(enabled = enabled, role = Role.Button) {
                board.haptics.tap()
                onClick()
            },
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            painterResource(icon),
            contentDescription = null,
            tint = if (tonal) colors.onFunction else colors.icon,
            modifier = Modifier.size(22.dp).alpha(if (enabled) 1f else .36f),
        )
    }
}

/**
 * The voice bar, and the flow bar that replaces it while a dictation runs. Its gesture lives on
 * this slot, which stays put while the content swaps, so a hold keeps going under the finger.
 */
@Composable
private fun VoiceSlot(modifier: Modifier, state: KeyboardState, model: KeyboardViewModel, board: Board) {
    val canStart by rememberUpdatedState(state.phase !in FlowPhases || state.phase == "cancelled")
    var pressed by remember { mutableStateOf(false) }
    Box(
        modifier
            .fillMaxHeight()
            .semantics {
                contentDescription = "Mic"
                text = AnnotatedString("Dictation: ${state.phase}")
                stateDescription = phaseName(state.phase)
                liveRegion = LiveRegionMode.Polite
                role = Role.Button
                onClick(label = if (state.phase == "listening") "Finish" else "Talk") {
                    model.toggle()
                    true
                }
            }
            .pointerInput(model) {
                awaitEachGesture {
                    val down = awaitFirstDown()
                    if (!canStart) return@awaitEachGesture
                    down.consume()
                    pressed = true
                    board.haptics.tap()
                    try {
                        if (withTimeoutOrNull(viewConfiguration.longPressTimeoutMillis) { awaitRelease(down.id) } != null) {
                            model.toggle()
                        } else {
                            board.haptics.hold()
                            model.holdDown()
                            try {
                                awaitRelease(down.id)
                            } finally {
                                model.holdUp()
                            }
                        }
                    } finally {
                        pressed = false
                    }
                }
            },
        contentAlignment = Alignment.Center,
    ) {
        when (state.phase) {
            "listening" -> FlowPill {
                PillButton("Cancel", R.drawable.ms_close, FlowButton, FlowLabel) {
                    board.haptics.tap()
                    model.cancel()
                }
                Spacer(Modifier.width(7.dp))
                Wave(ripple = false)
                Spacer(Modifier.width(7.dp))
                PillButton("Finish", R.drawable.ms_check, Color.White, FlowFill) {
                    board.haptics.tap()
                    model.toggle()
                }
            }
            "transcribing", "cleaning", "inserting" -> FlowPill(Modifier.padding(horizontal = 26.dp)) { Wave(ripple = true) }
            "cancelled" -> FlowPill {
                Box(Modifier.size(40.dp), contentAlignment = Alignment.Center) {
                    Box(Modifier.size(30.dp).background(FlowButton, CircleShape), contentAlignment = Alignment.Center) {
                        Icon(painterResource(R.drawable.ms_close), contentDescription = null, tint = Color.White.copy(alpha = .5f), modifier = Modifier.size(16.dp))
                    }
                }
                Spacer(Modifier.width(4.dp))
                Text("Cancelled", color = FlowLabel, fontSize = 14.sp, fontWeight = FontWeight.Medium)
                Spacer(Modifier.width(8.dp))
                PillButton("Recover", R.drawable.ms_replay, FlowButton, FlowLabel) {
                    board.haptics.tap()
                    model.recover()
                }
            }
            else -> VoiceBar(state, board.colors, pressed)
        }
    }
}

private fun phaseName(phase: String) = when (phase) {
    "listening" -> "Listening"
    "transcribing" -> "Transcribing"
    "cleaning" -> "Cleaning up"
    "inserting" -> "Inserting"
    "done" -> "Inserted"
    "cancelled" -> "Cancelled"
    "failed" -> "Failed"
    else -> "Ready"
}

/** Rust sends error codes; the bar shows a short sentence. A countdown is stale once listening ends. */
private fun barNotice(raw: String): String? = when {
    raw.isBlank() || raw.endsWith("seconds left.") -> null
    raw == "ENGINE_NO_SPEECH" -> "No speech heard"
    raw == "ENGINE_NO_MODEL" -> "Download a speech model in Hushpen"
    raw.startsWith("MIC_") || raw.startsWith("CAPTURE_") -> "The microphone stopped"
    raw.startsWith("INSERT_") -> "The text could not be inserted"
    raw.matches(Regex("[A-Z_]+")) -> "Dictation failed"
    else -> readable(raw).removeSuffix(".")
}

@Composable
private fun VoiceBar(state: KeyboardState, colors: BoardColors, pressed: Boolean) {
    val notice = barNotice(state.notice)
    val (icon, tint, label) = when {
        state.phase == "done" -> Triple(R.drawable.ms_check, colors.accent, "Inserted")
        notice != null -> Triple(R.drawable.ms_error, colors.error, notice)
        state.phase == "failed" -> Triple(R.drawable.ms_error, colors.error, "Dictation failed")
        !state.canDictate -> Triple(R.drawable.ms_mic_fill, colors.onFunction.copy(alpha = .5f), "Dictation is off in password fields")
        else -> Triple(R.drawable.ms_mic_fill, colors.accent, "Tap or hold to talk")
    }
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 4.dp)
            .height(40.dp)
            .clip(CircleShape)
            .background(if (pressed) colors.pressed.compositeOver(colors.function) else colors.function)
            .padding(horizontal = 16.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(painterResource(icon), contentDescription = null, tint = tint, modifier = Modifier.size(20.dp))
        Text(
            label,
            color = colors.onFunction,
            fontSize = 14.sp,
            fontWeight = FontWeight.Medium,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f, fill = false),
        )
    }
}

/** The desktop flow bar pill. It stays dark in both appearances, as on the desktop. */
@Composable
private fun FlowPill(modifier: Modifier = Modifier, content: @Composable RowScope.() -> Unit) {
    CompositionLocalProvider(LocalContentColor provides Color.White) {
        Row(
            Modifier
                .height(40.dp)
                .clip(CircleShape)
                .background(FlowFill)
                .border(.5.dp, FlowEdge, CircleShape)
                .then(modifier),
            verticalAlignment = Alignment.CenterVertically,
            content = content,
        )
    }
}

/**
 * Sets the button's accessibility click itself. In the IME window an accessibility click on a
 * plain clickable never reached its lambda, while explicit semantics work, as on the keys.
 */
private fun Modifier.actionSemantics(description: String, enabled: Boolean, onClick: () -> Unit) = clearAndSetSemantics {
    contentDescription = description
    role = Role.Button
    if (enabled) {
        onClick {
            onClick()
            true
        }
    } else {
        disabled()
    }
}

@Composable
private fun PillButton(description: String, icon: Int, fill: Color, tint: Color, onClick: () -> Unit) {
    Box(
        Modifier
            .size(40.dp)
            .actionSemantics(description, true, onClick)
            .clip(CircleShape)
            .clickable(role = Role.Button, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Box(Modifier.size(30.dp).background(fill, CircleShape), contentAlignment = Alignment.Center) {
            Icon(painterResource(icon), contentDescription = null, tint = tint, modifier = Modifier.size(16.dp))
        }
    }
}

private const val WAVE_BARS = 18
private val WaveHeights = floatArrayOf(
    .42f, .7f, .95f, .6f, .82f, 1f, .66f, .5f, .88f, .58f, .36f, .76f,
    .92f, .54f, .7f, .44f, .84f, .6f, .94f, .48f, .72f, .38f, .8f, .64f,
)

/** Listening bars swing on their own beat; the processing ripple runs short bars up and down. */
@Composable
private fun Wave(ripple: Boolean) {
    val period = if (ripple) 1300 else 1800
    val clock = rememberInfiniteTransition(label = "wave")
    val time by clock.animateFloat(0f, period.toFloat(), infiniteRepeatable(tween(period, easing = LinearEasing)), label = "time")
    Canvas(Modifier.size(width = 122.dp, height = 20.dp)) {
        val bar = 3.dp.toPx()
        val gap = 4.dp.toPx()
        val count = WAVE_BARS
        for (index in 0 until count) {
            val phase = ((time + (index * 137) % 900) % period) / period
            val height = if (ripple) {
                size.height * .28f * (1f + 1.1f * (.5f - .5f * cos(2 * PI * phase).toFloat()))
            } else {
                val edge = minOf(index, count - 1 - index)
                val full = when (edge) {
                    0 -> .7f
                    1 -> .85f
                    2 -> .95f
                    else -> WaveHeights[(index * 5) % WaveHeights.size]
                }
                val swing = if (phase < .5f) phase * 2 else 2 - phase * 2
                size.height * full * (.5f + .5f * swing * swing * (3 - 2 * swing))
            }
            drawRoundRect(
                color = Color.White.copy(alpha = .9f),
                topLeft = Offset(index * (bar + gap), (size.height - height) / 2),
                size = Size(bar, height),
                cornerRadius = CornerRadius(bar / 2),
            )
        }
    }
}
