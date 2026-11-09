package com.mandalsuraj.hushpen.server

import android.os.PowerManager
import android.util.Log
import com.mandalsuraj.hushpen.core.JobActivity

class JobWakeLock(private val lock: PowerManager.WakeLock) : JobActivity, AutoCloseable {
    init { lock.setReferenceCounted(false) }

    @Synchronized
    override fun changed(active: Boolean) {
        try {
            if (active && !lock.isHeld) lock.acquire(10 * 60 * 1000L)
            if (!active && lock.isHeld) lock.release()
        } catch (problem: RuntimeException) {
            Log.w("HushpenHost", "Could not change the model job wake lock", problem)
        }
    }

    override fun close() = changed(false)
}
