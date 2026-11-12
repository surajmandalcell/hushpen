package com.mandalsuraj.hushpen

import android.app.ActivityManager
import android.os.Debug
import android.os.SystemClock
import android.util.Log
import androidx.test.platform.app.InstrumentationRegistry
import com.mandalsuraj.hushpen.core.Engine
import com.mandalsuraj.hushpen.core.LocalCleanup
import com.mandalsuraj.hushpen.core.ModelRepository
import java.io.File
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicLong
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test

class PhoneModelsTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val data = File(context.filesDir, "hushpen")
    private val memory: ActivityManager.MemoryInfo
        get() = ActivityManager.MemoryInfo().also {
            context.getSystemService(ActivityManager::class.java).getMemoryInfo(it)
        }

    @Test(timeout = 240_000) fun s1MiniCleansWithTheDesktopPrompt() = cleanup("s1-mini")

    @Test(timeout = 240_000) fun qwenCleansWithTheDesktopPrompt() = cleanup("qwen3.5-2b")

    @Test fun catalogUsesThePhonesPhysicalMemoryWithoutModelFixtures() {
        val capacity = memory.totalMem.toULong()
        val models = ModelRepository.withMemory(data.absolutePath, capacity).models().map { it.id }
        assertTrue(models.contains("small-q5_1"))
        assertEquals(capacity >= 6_000_000_000uL, models.contains("large-v3-turbo-q5_0"))
        assertEquals(capacity >= 4_000_000_000uL, models.contains("s1-mini"))
        assertEquals(capacity >= 8_000_000_000uL, models.contains("qwen3.5-2b"))
    }

    @Test(timeout = 60_000) fun selectionsAndDisabledCleanupSurviveRepositoryRestart() = runBlocking {
        requireModelFixtures(6_000_000_000)
        val preferences = context.getSharedPreferences("phone", android.content.Context.MODE_PRIVATE)
        val savedSpeech = preferences.getString("speechModel", null)
        val savedCleanup = preferences.getString("cleanupModel", null)
        val savedEnabled = preferences.getBoolean("cleanup", false)
        val first = PhoneRepository(context)
        val repositories = mutableListOf<PhoneRepository>()
        try {
            first.initialize()
            repositories += first
            assertTrue(first.verify("large-v3-turbo-q5_0"))
            assertTrue(first.verify("s1-mini"))
            first.selectModel("large-v3-turbo-q5_0")
            first.selectModel("s1-mini")
            assertEquals("large-v3-turbo-q5_0", first.selectedModel.value)
            assertEquals("s1-mini", first.selectedCleanupModel.value)
            first.setCleanup(false)
            val expected = first.cleanup("um this is a test")
            assertEquals("This is a test.", expected)

            val restarted = PhoneRepository(context)
            repositories += restarted
            restarted.initialize()
            assertEquals("large-v3-turbo-q5_0", restarted.selectedModel.value)
            assertEquals("s1-mini", restarted.selectedCleanupModel.value)
            assertFalse(restarted.cleanupEnabled.value)
            assertEquals(expected, restarted.cleanup("um this is a test"))
        } finally {
            repositories.forEach { it.shutdown() }
            assertTrue(preferences.edit().apply {
                if (savedSpeech == null) remove("speechModel") else putString("speechModel", savedSpeech)
                if (savedCleanup == null) remove("cleanupModel") else putString("cleanupModel", savedCleanup)
                putBoolean("cleanup", savedEnabled)
            }.commit())
        }
    }

    @Test(timeout = 240_000) fun turboLoadsThePinnedSpeechModel() {
        requireModelFixtures(6_000_000_000)
        val catalog = ModelRepository.withMemory(data.absolutePath, memory.totalMem.toULong())
        assertTrue("Stage the pinned Turbo Q5 fixture before running this test.", catalog.verify("large-v3-turbo-q5_0"))
        val engine = Engine(data.absolutePath)
        try {
            measure("large-v3-turbo-q5_0-load") { engine.load("large-v3-turbo-q5_0") }
        } finally {
            engine.shutdown()
        }
    }

    private fun cleanup(id: String) {
        requireModelFixtures(if (id == "qwen3.5-2b") 8_000_000_000 else 4_000_000_000)
        val capacity = InstrumentationRegistry.getArguments().getString("catalogMemoryBytes")
            ?.toULong() ?: memory.totalMem.toULong()
        val catalog = ModelRepository.withMemory(data.absolutePath, capacity)
        assertTrue("Stage the pinned $id fixture before running this test.", catalog.verify(id))
        val cleanup = LocalCleanup(data.absolutePath, capacity)
        try {
            cleanup.setModel(id)
            cleanup.setMemoryBudget(memory.availMem.toULong())
            assertTrue("The emulator has too little free memory for $id.", cleanup.available())
            measure(id) {
                val result = cleanup.clean("this is a test", "general")
                assertEquals("The real cleanup model must run.", "used", result.outcome)
                assertEquals("This is a test.", result.text)
            }
        } finally {
            cleanup.shutdown()
        }
    }

    private fun requireModelFixtures(minimumMemory: Long) {
        assumeTrue("Enable staged model tests with -e phoneModelFixtures true.",
            InstrumentationRegistry.getArguments().getString("phoneModelFixtures") == "true")
        assumeTrue("This phone has too little physical RAM for this model.", memory.totalMem >= minimumMemory)
    }

    private fun measure(name: String, action: () -> Unit) {
        val baseline = statusBytes("VmRSS")
        val peak = AtomicLong(baseline)
        val samples = Executors.newSingleThreadScheduledExecutor()
        samples.scheduleAtFixedRate({
            peak.accumulateAndGet(statusBytes("VmRSS")) { current, sample -> maxOf(current, sample) }
        }, 0, 25, TimeUnit.MILLISECONDS)
        val start = SystemClock.elapsedRealtime()
        try {
            action()
        } finally {
            val elapsed = SystemClock.elapsedRealtime() - start
            samples.shutdownNow()
            assertTrue("Memory sampling must stop.", samples.awaitTermination(5, TimeUnit.SECONDS))
            val pss = Debug.MemoryInfo().also { Debug.getMemoryInfo(it) }.totalPss.toLong() * 1024
            val gate = InstrumentationRegistry.getArguments().getString("catalogMemoryBytes") ?: memory.totalMem.toString()
            val report = "model=$name elapsed_ms=$elapsed baseline_rss_bytes=$baseline peak_rss_bytes=${peak.get()} process_hwm_bytes=${statusBytes("VmHWM")} final_pss_bytes=$pss physical_memory_bytes=${memory.totalMem} catalog_memory_bytes=$gate"
            Log.i("PhoneModelsTest", report)
            InstrumentationRegistry.getInstrumentation().sendStatus(0, android.os.Bundle().apply {
                putString("stream", "PHONE_MODEL_MEASUREMENT $report\n")
            })
        }
    }

    private fun statusBytes(field: String): Long = File("/proc/self/status").useLines { lines ->
        val value = checkNotNull(lines.firstOrNull { it.startsWith("$field:") }) {
            "Missing $field memory counter."
        }
        value.substringAfter(':').trim().substringBefore(' ').toLong() * 1024
    }
}
