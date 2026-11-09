package com.mandalsuraj.hushpen.server

import com.mandalsuraj.hushpen.core.MobileException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class NativeSystemLlmTest {
    @Test fun unavailableAndFailedSystemsStayUnavailable() {
        val unavailable = NativeSystemLlm(object : SystemPrompt {
            override fun available() = false
            override fun generate(text: String, instructions: String) = error("Must not generate")
        }).apply { foreground = true }
        assertFalse(unavailable.available())
        val failed = NativeSystemLlm(object : SystemPrompt {
            override fun available(): Boolean = error("AICore is missing")
            override fun generate(text: String, instructions: String) = error("Must not generate")
        }).apply { foreground = true }
        assertFalse(failed.available())
    }

    @Test fun cleanupPreservesRustInputAndInstructions() {
        val seen = mutableListOf<Pair<String, String>>()
        val system = NativeSystemLlm(object : SystemPrompt {
            override fun available() = true
            override fun generate(text: String, instructions: String): String {
                seen += text to instructions
                return "Hello there."
            }
        }).apply { foreground = true }
        assertTrue(system.available())
        assertEquals("Hello there.", system.cleanup("um hello there", "Keep names"))
        assertEquals(listOf("um hello there" to "Keep names"), seen)
        system.enabled = false
        assertFalse(system.available())
        try {
            system.cleanup("Do not send", "Do not send")
            throw AssertionError("Disabled cleanup must fail")
        } catch (_: MobileException.Failed) {
            assertEquals(1, seen.size)
        }
    }

    @Test fun nativeFailuresReachTheRustFallback() {
        val system = NativeSystemLlm(object : SystemPrompt {
            override fun available() = true
            override fun generate(text: String, instructions: String): String = error("Quota reached")
        }).apply { foreground = true }
        try {
            system.cleanup("Hello", "Keep names")
            throw AssertionError("Native failure must reach Rust")
        } catch (problem: MobileException.Failed) {
            assertEquals("Quota reached", problem.detail)
        }
    }

    @Test fun backgroundCleanupDoesNotCallTheSystemModel() {
        var called = false
        val system = NativeSystemLlm(object : SystemPrompt {
            override fun available(): Boolean {
                called = true
                return true
            }
            override fun generate(text: String, instructions: String): String {
                called = true
                return text
            }
        })
        assertFalse(system.available())
        try {
            system.cleanup("Hello", "Keep names")
            throw AssertionError("Background cleanup must fail")
        } catch (_: MobileException.Failed) {
            assertFalse(called)
        }
    }
}
