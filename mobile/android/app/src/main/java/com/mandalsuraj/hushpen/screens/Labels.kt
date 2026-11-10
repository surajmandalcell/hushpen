package com.mandalsuraj.hushpen.screens

import com.mandalsuraj.hushpen.core.HostUnloadRule
import com.mandalsuraj.hushpen.core.Model
import kotlin.math.roundToLong

enum class Screen { Home, Models, Devices, Settings, Server }

internal fun ruleName(rule: HostUnloadRule): String = when (rule) {
    HostUnloadRule.KeepLoaded -> "Keep loaded"
    is HostUnloadRule.AfterIdle -> if (rule.minutes == 1u) "After 1 idle minute" else "After ${rule.minutes} idle minutes"
    HostUnloadRule.WhenMemoryLow -> "When memory is low"
    HostUnloadRule.WhileServerModeOpen -> "While Server mode is open"
}

internal fun ruleChoice(rule: HostUnloadRule): String = when (rule) {
    is HostUnloadRule.AfterIdle -> "After idle time"
    else -> ruleName(rule)
}

/** Rust identifies a paired desktop only by its public key, shown as lowercase hex. */
internal fun desktopName(id: String): String = "Desktop ${id.take(8)}"

internal fun sizeLabel(bytes: ULong): String {
    val value = bytes.toDouble()
    return if (value >= 1e9) String.format("%.1f GB", value / 1e9) else "${(value / 1e6).roundToLong()} MB"
}

internal fun speechPurpose(model: Model): String = when {
    model.name.startsWith("Parakeet", ignoreCase = true) -> "English and 24 European languages"
    model.name.startsWith("Whisper", ignoreCase = true) -> "Other languages and slower phones"
    else -> "Speech to text"
}
