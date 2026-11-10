package com.mandalsuraj.hushpen.server

import android.os.PowerManager
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class JobWakeLockTest {
    @Test fun lockFollowsOnlyModelJobsAndReleasesOnClose() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val lock = context.getSystemService(PowerManager::class.java)
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "Hushpen:WakeTest")
        val activity = JobWakeLock(lock)
        try {
            assertFalse(lock.isHeld)
            activity.changed(true)
            assertTrue(lock.isHeld)
            activity.changed(true)
            activity.changed(false)
            assertFalse(lock.isHeld)
            activity.changed(true)
            activity.close()
            assertFalse(lock.isHeld)
        } finally {
            activity.close()
        }
    }
}
