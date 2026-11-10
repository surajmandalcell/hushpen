package com.mandalsuraj.hushpen.ime

import android.accessibilityservice.AccessibilityServiceInfo
import android.content.ContextWrapper
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Rect
import android.os.SystemClock
import android.view.MotionEvent
import android.view.InputDevice
import android.view.ViewConfiguration
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityWindowInfo
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import androidx.test.core.app.ActivityScenario
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.lifecycle.ViewModelStore
import com.mandalsuraj.hushpen.PhoneApplication
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File

@RunWith(AndroidJUnit4::class)
class KeyboardSmokeTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val app = ApplicationProvider.getApplicationContext<PhoneApplication>()

    @Test fun microphoneWritesARealCaptureWav() {
        enableKeyboard()
        ActivityScenario.launch(ImeTestActivity::class.java).use {
            val file = File.createTempFile("microphone-", ".wav", app.cacheDir)
            val microphone = Microphone(app)
            val failure = java.util.concurrent.atomic.AtomicReference<Throwable>()
            try {
                runBlocking {
                    microphone.start(file, 16_000 * 2 * 5L) { failure.set(it) }
                    kotlinx.coroutines.delay(500)
                    microphone.stop()
                }
                assertEquals(null, failure.get())
                assertTrue("AudioRecord must write PCM samples.", file.length() > 44)
                file.inputStream().use { stream ->
                    val header = ByteArray(4)
                    assertEquals(4, stream.read(header))
                    assertEquals("RIFF", String(header, Charsets.US_ASCII))
                }
            } finally {
                runBlocking { microphone.stop() }
                microphone.close { file.delete() }
            }
        }
    }

    @Test fun keyboardTypesAndDeletesThroughTheInputConnection() {
        enableKeyboard()
        ActivityScenario.launch(ImeTestActivity::class.java).use { scenario ->
            click("h")
            click("i")
            click("Space")
            click("Shift")
            click("A")
            expectText(scenario, "hi A")
            click("Backspace")
            click("Symbols")
            click("1")
            click("Enter")
            expectText(scenario, "hi 1\n")
            click("Letters")
            click("Backspace")
            expectText(scenario, "hi 1")
            scenario.onActivity { it.editor.setText("abcdefghij"); it.editor.setSelection(10) }
            hold("Backspace", 850)
            scenario.onActivity { assertTrue("Holding Backspace must repeat.", it.editor.text.length < 9) }
        }
    }

    @Test fun keyboardHonorsEditorActionAndDeletesWholeCodePoints() {
        enableKeyboard()
        ActivityScenario.launch(ImeTestActivity::class.java).use { scenario ->
            scenario.onActivity { activity ->
                activity.editor.setSingleLine(true)
                activity.editor.setText("hello 🌍")
                activity.editor.setSelection(activity.editor.text.length)
                activity.editor.imeOptions = EditorInfo.IME_ACTION_DONE
                activity.getSystemService(InputMethodManager::class.java).restartInput(activity.editor)
            }
            click("Backspace")
            expectText(scenario, "hello ")
            click("Enter")
            waitFor("Done editor action") {
                var action: Int? = null
                scenario.onActivity { action = it.editorAction }
                action == EditorInfo.IME_ACTION_DONE
            }
            expectText(scenario, "hello ")
            scenario.onActivity { activity ->
                activity.editor.setImeActionLabel("Send", 42)
                activity.getSystemService(InputMethodManager::class.java).restartInput(activity.editor)
            }
            instrumentation.waitForIdleSync()
            click("Enter")
            waitFor("custom Send editor action") {
                var action: Int? = null
                scenario.onActivity { action = it.editorAction }
                action == 42
            }
            expectText(scenario, "hello ")
        }
    }

    @Test fun microphoneUsesTheRealRustPipelineAndUndoChecksTheEditor() {
        enableKeyboard()
        val wav = File(app.filesDir, "ime-test.wav")
        assertTrue("Missing debug WAV: ${wav.absolutePath}", wav.isFile)
        runBlocking {
            app.repository.initialize()
            assertTrue("Missing pinned Whisper Base Q5 model in ${app.filesDir}/hushpen/models/whisper", app.repository.verify("base-q5_1"))
            app.repository.selectModel("base-q5_1")
        }
        ActivityScenario.launch(ImeTestActivity::class.java).use { scenario ->
            scenario.onActivity { it.editor.setText("Previous text"); it.editor.selectAll() }
            click("Mic")
            val speechState = reveal("Dictation: listening")
            assertTrue("The listening state must be visible.", speechState.isVisibleToUser)
            click("Mic")
            var transcript = ""
            // Rust allows 120 seconds to load and 300 seconds to transcribe a cold job.
            waitFor("real speech insertion", 450_000) {
                failIfSpeechStopped(speechState)
                scenario.onActivity { transcript = it.editor.text.toString() }
                transcript.isNotBlank() && transcript != "Previous text"
            }
            assertTrue("The real model must return a transcript.", transcript.any(Char::isLetter))
            click("Undo")
            expectText(scenario, "Previous text")
            scenario.onActivity { it.editor.setText("") }
            hold("Mic", 850)
            val holdState = if (speechState.refresh()) speechState else {
                instrumentation.uiAutomation.windows.firstOrNull { it.type == AccessibilityWindowInfo.TYPE_INPUT_METHOD }
                    ?.root?.findAccessibilityNodeInfosByText("Dictation:")?.firstOrNull()
                    ?: throw AssertionError("The dictation state is unavailable.\n${failureSnapshot()}")
            }
            waitFor("hold-to-talk insertion", 450_000) {
                failIfSpeechStopped(holdState)
                var value = ""
                scenario.onActivity { value = it.editor.text.toString() }
                value.isNotBlank()
            }
            click("Undo")
            expectText(scenario, "")
            click("Mic")
            assertTrue("The second listening state must be visible.", reveal("Dictation: listening").isVisibleToUser)
            click("Mic")
            scenario.onActivity { activity -> activity.otherEditor.requestFocus() }
            waitFor("new editor") {
                var focused = false
                scenario.onActivity { focused = it.otherEditor.hasFocus() }
                focused
            }
            instrumentation.waitForIdleSync()
            runBlocking { app.repository.loadSelectedModel() }
            instrumentation.waitForIdleSync()
            scenario.onActivity {
                assertEquals("A result must not enter the old editor.", "", it.editor.text.toString())
                assertEquals("A result must not enter a new editor.", "", it.otherEditor.text.toString())
            }
        }
    }

    @Test fun microphoneRefusesMissingPermissionAndPasswordFields() {
        val denied = object : ContextWrapper(app) {
            override fun checkSelfPermission(permission: String): Int = PackageManager.PERMISSION_DENIED
        }
        lateinit var keyboard: KeyboardViewModel
        val store = ViewModelStore()
        instrumentation.runOnMainSync {
            keyboard = KeyboardViewModel(denied, app.repository)
            store.put("keyboard", keyboard)
            keyboard.openInput(canDictate = true) { error("Permission refusal must not insert text.") }
            keyboard.toggle()
        }
        waitFor("microphone permission refusal") { keyboard.state.value.phase == "failed" }
        assertFalse("The permission failure must be visible.", keyboard.state.value.notice.isBlank())
        instrumentation.runOnMainSync {
            keyboard.openInput(canDictate = false) { error("A password field must not receive dictation.") }
            keyboard.toggle()
        }
        assertEquals("idle", keyboard.state.value.phase)
        assertEquals("Dictation is off in password fields.", keyboard.state.value.notice)
        instrumentation.runOnMainSync { keyboard.closeInput(); store.clear() }
    }

    private fun enableKeyboard() {
        val component = "${app.packageName}/.ime.HushpenInputMethod"
        shell("ime enable $component")
        shell("ime set $component")
        shell("pm grant ${app.packageName} android.permission.RECORD_AUDIO")
        val info = instrumentation.uiAutomation.serviceInfo
        info.flags = info.flags or AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS
        instrumentation.uiAutomation.serviceInfo = info
    }

    private fun shell(command: String) {
        instrumentation.uiAutomation.executeShellCommand(command).use { descriptor ->
            android.os.ParcelFileDescriptor.AutoCloseInputStream(descriptor).use { it.readBytes() }
        }
    }

    private fun click(label: String) {
        val node = reveal(label)
        assertTrue("The $label action must be clickable.", node.performAction(AccessibilityNodeInfo.ACTION_CLICK))
        instrumentation.waitForIdleSync()
        instrumentation.uiAutomation.windows.forEach { it.root }
    }

    private fun hold(label: String, milliseconds: Long) {
        val node = reveal(label)
        val bounds = Rect()
        node.getBoundsInScreen(bounds)
        val started = SystemClock.uptimeMillis()
        fun send(action: Int) {
            val event = MotionEvent.obtain(started, SystemClock.uptimeMillis(), action,
                bounds.exactCenterX(), bounds.exactCenterY(), 0)
            event.source = InputDevice.SOURCE_TOUCHSCREEN
            try {
                assertTrue("The emulator must accept $label touch.", instrumentation.uiAutomation.injectInputEvent(event, true))
            } finally { event.recycle() }
        }
        send(MotionEvent.ACTION_DOWN)
        SystemClock.sleep(ViewConfiguration.getLongPressTimeout() + milliseconds)
        send(MotionEvent.ACTION_UP)
        instrumentation.waitForIdleSync()
    }

    private fun reveal(label: String): AccessibilityNodeInfo {
        var result: AccessibilityNodeInfo? = null
        var attempts = 0
        waitFor("visible keyboard action $label") {
            val node = find(label)
            if (node != null && node.isVisibleToUser && node.isEnabled) {
                result = node
                true
            } else {
                if (node?.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.id) != true) {
                    val direction = if ((attempts++ / 8) % 2 == 0) AccessibilityNodeInfo.ACTION_SCROLL_FORWARD else AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD
                    fun scroll(parent: AccessibilityNodeInfo): Boolean {
                        if (parent.isScrollable && parent.performAction(direction)) return true
                        for (index in 0 until parent.childCount) {
                            if (parent.getChild(index)?.let(::scroll) == true) return true
                        }
                        return false
                    }
                    instrumentation.uiAutomation.windows.filter { it.type == AccessibilityWindowInfo.TYPE_INPUT_METHOD }
                        .forEach { window -> window.root?.let(::scroll) }
                }
                false
            }
        }
        return checkNotNull(result)
    }

    private fun find(label: String): AccessibilityNodeInfo? {
        var remaining = 500
        fun search(node: AccessibilityNodeInfo, depth: Int): AccessibilityNodeInfo? {
            if (remaining-- <= 0 || depth > 40 || !node.refresh()) return null
            if (node.text?.toString().equals(label, ignoreCase = true) || node.contentDescription?.toString().equals(label, ignoreCase = true)) return node
            for (index in 0 until node.childCount) {
                node.getChild(index)?.let { child -> search(child, depth + 1)?.let { return it } }
                if (remaining <= 0) break
            }
            return null
        }
        return instrumentation.uiAutomation.windows.firstNotNullOfOrNull { window -> window.root?.let { search(it, 0) } }
            ?: instrumentation.uiAutomation.rootInActiveWindow?.let { search(it, 0) }
    }

    private fun expectText(scenario: ActivityScenario<ImeTestActivity>, expected: String) {
        waitFor("editor text $expected") {
            var actual = ""
            scenario.onActivity { actual = it.editor.text.toString() }
            actual == expected
        }
    }

    private fun failIfSpeechStopped(node: AccessibilityNodeInfo) {
        if (!node.refresh()) return
        val state = node.text?.toString()
        if (state == "Dictation: failed" || state == "Dictation: cancelled") {
            throw AssertionError("Speech stopped: $state\n${failureSnapshot()}")
        }
    }

    private fun waitFor(description: String, timeoutMs: Long = 15_000, condition: () -> Boolean) {
        val deadline = SystemClock.elapsedRealtime() + timeoutMs
        while (SystemClock.elapsedRealtime() < deadline) {
            if (condition()) return
            SystemClock.sleep(100)
        }
        throw AssertionError("Timed out waiting for $description.\n${failureSnapshot()}")
    }

    private fun failureSnapshot(): String {
        val labels = linkedSetOf<String>()
        var remaining = 500
        fun collect(node: AccessibilityNodeInfo, depth: Int) {
            if (remaining-- <= 0 || depth > 40 || labels.size >= 100 || !node.refresh()) return
            if (node.isVisibleToUser) {
                listOfNotNull(node.text, node.contentDescription).forEach { text ->
                    if (labels.size < 100) labels.add(text.toString().take(160))
                }
            }
            for (index in 0 until minOf(node.childCount, 100)) {
                node.getChild(index)?.let { collect(it, depth + 1) }
                if (remaining <= 0 || labels.size >= 100) break
            }
        }
        runCatching {
            instrumentation.uiAutomation.windows.take(10).forEach { window ->
                window.root?.let { collect(it, 0) }
            }
        }.onFailure { labels.add("Accessibility snapshot failed: ${it.javaClass.simpleName}") }
        runCatching {
            val file = File(app.filesDir, "ime-failure.png")
            check(!file.exists() || file.delete()) { "Previous screenshot could not be removed." }
            val bitmap = checkNotNull(instrumentation.uiAutomation.takeScreenshot()) { "Screenshot is unavailable." }
            try {
                file.outputStream().use { output ->
                    check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) { "Screenshot could not be saved." }
                }
            } finally { bitmap.recycle() }
        }.onFailure { labels.add("Screenshot failed: ${it.javaClass.simpleName}") }
        return labels.joinToString("\n").take(4_000)
    }
}
