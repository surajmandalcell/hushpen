package com.mandalsuraj.hushpen.ime

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class InsertReceiptTest {
    @Test fun undoRequiresTheSameCursorAndUnchangedText() {
        val receipt = InsertReceipt("Hello 🌍", 12, "Old ")
        assertTrue(receipt.matches(12, 12, "Old Hello 🌍"))
        assertFalse(receipt.matches(11, 11, "Old Hello 🌍"))
        assertFalse(receipt.matches(11, 12, "Old Hello 🌍"))
        assertFalse(receipt.matches(12, 12, "New Hello 🌍"))
        assertFalse(receipt.matches(12, 12, "Old Hello !"))
    }
}
