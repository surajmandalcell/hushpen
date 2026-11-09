package com.mandalsuraj.hushpen.ime

internal data class InsertReceipt(
    val text: String,
    val end: Int,
    val before: String,
    val replaced: String = "",
) {
    fun matches(selectionStart: Int, selectionEnd: Int, surrounding: String): Boolean =
        selectionStart == end && selectionEnd == end && surrounding == before + text
}
