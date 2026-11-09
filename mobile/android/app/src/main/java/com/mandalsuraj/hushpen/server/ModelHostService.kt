package com.mandalsuraj.hushpen.server

import android.app.Notification
import android.app.ActivityManager
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.ComponentCallbacks2
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import com.mandalsuraj.hushpen.PhoneApplication
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive

class ModelHostService : Service() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    @Volatile internal var destroyed = false
        private set
    @Volatile private var controller: ServerController? = null

    override fun onCreate() {
        super.onCreate()
        val notifications = getSystemService(NotificationManager::class.java)
        notifications.createNotificationChannel(
            NotificationChannel(CHANNEL, "Server mode", NotificationManager.IMPORTANCE_LOW).apply {
                description = "Models available to paired desktops"
                setSound(null, null)
                enableVibration(false)
            }
        )
        val open = packageManager.getLaunchIntentForPackage(packageName)
        val notification = Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_btn_speak_now)
            .setContentTitle("Hushpen server mode")
            .setContentText("Models are available to paired desktops")
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setCategory(Notification.CATEGORY_SERVICE)
            .apply {
                if (open != null) setContentIntent(
                    PendingIntent.getActivity(this@ModelHostService, 0, open, PendingIntent.FLAG_IMMUTABLE)
                )
            }
            .build()
        if (Build.VERSION.SDK_INT >= 34) {
            startForeground(NOTIFICATION, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(NOTIFICATION, notification)
        }
        scope.launch {
            val activity = getSystemService(ActivityManager::class.java)
            val memory = ActivityManager.MemoryInfo()
            while (isActive) {
                activity.getMemoryInfo(memory)
                if (memory.lowMemory) controller?.memoryLow()
                delay(5_000)
            }
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        scope.launch {
            try {
                val repository = (application as PhoneApplication).repository
                repository.initialize()
                if (!destroyed) {
                    controller = repository.serverController
                    controller?.attach(this@ModelHostService)
                }
            } catch (problem: Exception) {
                android.util.Log.e("HushpenHost", "Server mode failed", problem)
                stopSelf(startId)
            }
        }
        return START_NOT_STICKY
    }

    override fun onTrimMemory(level: Int) {
        super.onTrimMemory(level)
        if (level == ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW ||
            level == ComponentCallbacks2.TRIM_MEMORY_RUNNING_CRITICAL ||
            level >= ComponentCallbacks2.TRIM_MEMORY_BACKGROUND
        ) controller?.memoryLow()
    }

    override fun onLowMemory() {
        super.onLowMemory()
        controller?.memoryLow()
    }

    override fun onDestroy() {
        destroyed = true
        controller?.detach(this)
        scope.cancel()
        stopForeground(STOP_FOREGROUND_REMOVE)
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    companion object {
        private const val CHANNEL = "hushpen-server"
        private const val NOTIFICATION = 7342
    }
}
