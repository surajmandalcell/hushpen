package com.mandalsuraj.hushpen.server

import android.content.Context
import android.content.Intent
import android.net.wifi.WifiManager
import android.os.PowerManager
import com.mandalsuraj.hushpen.core.Host
import com.mandalsuraj.hushpen.core.HostUnloadRule
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

interface ServerApplication {
    val serverController: ServerController
}

data class ServerState(val running: Boolean = false, val port: Int? = null, val notice: String? = null)

class ServerController(context: Context, private val host: Host) : AutoCloseable {
    private val context = context.applicationContext
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val mutex = Mutex()
    private val mutableState = MutableStateFlow(ServerState())
    val state: StateFlow<ServerState> = mutableState.asStateFlow()
    @Volatile private var requestedRule: HostUnloadRule = HostUnloadRule.KeepLoaded
    @Volatile private var activityOpen = true
    private var service: ModelHostService? = null
    private var runningRule: HostUnloadRule? = null
    private var wake: JobWakeLock? = null
    private var multicast: WifiManager.MulticastLock? = null

    fun start(rule: HostUnloadRule) {
        requestedRule = rule
        try {
            context.startForegroundService(Intent(context, ModelHostService::class.java))
        } catch (problem: Exception) {
            mutableState.value = ServerState(notice = problem.message ?: "Server mode could not start")
        }
    }

    fun stop() {
        context.stopService(Intent(context, ModelHostService::class.java))
        scope.launch { mutex.withLock { stopServing() } }
    }

    fun updateRule(rule: HostUnloadRule) {
        requestedRule = rule
        scope.launch {
            mutex.withLock {
                if (service == null || runningRule == rule) return@withLock
                try {
                    host.stop()
                    val port = host.start("0.0.0.0:7342", rule).toInt()
                    runningRule = rule
                    mutableState.value = ServerState(running = true, port = port)
                } catch (problem: Exception) {
                    service?.stopSelf()
                    stopServing()
                    mutableState.value = ServerState(notice = problem.message ?: "Unload rule could not be changed")
                }
            }
        }
    }

    internal fun attach(next: ModelHostService) {
        scope.launch {
            mutex.withLock {
                if (next.destroyed) return@withLock
                val rule = requestedRule
                try {
                    if (service === next && runningRule == rule) return@withLock
                    if (service != null) stopServing()
                    val power = context.getSystemService(PowerManager::class.java)
                    wake = JobWakeLock(power.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "Hushpen:ModelJob"))
                    host.setJobActivity(wake)
                    val wifi = context.getSystemService(WifiManager::class.java)
                    multicast = wifi?.createMulticastLock("Hushpen:Discovery")?.apply {
                        setReferenceCounted(false)
                        acquire()
                    }
                    host.setForeground(activityOpen)
                    val port = host.start("0.0.0.0:7342", rule).toInt()
                    service = next
                    runningRule = rule
                    mutableState.value = ServerState(running = true, port = port)
                } catch (problem: Exception) {
                    stopServing()
                    mutableState.value = ServerState(notice = problem.message ?: "Server mode could not start")
                    next.stopSelf()
                }
            }
        }
    }

    internal fun detach(previous: ModelHostService) {
        scope.launch {
            mutex.withLock {
                if (service === previous) stopServing()
            }
        }
    }

    fun foreground(open: Boolean) {
        activityOpen = open
        scope.launch {
            mutex.withLock {
                host.setForeground(open)
                if (!open && requestedRule == HostUnloadRule.WhileServerModeOpen) {
                    service?.stopSelf()
                    stopServing()
                }
            }
        }
    }

    fun memoryLow() {
        scope.launch { mutex.withLock { host.memoryLow() } }
    }

    private fun stopServing() {
        host.stop()
        host.setJobActivity(null)
        wake?.close()
        wake = null
        multicast?.let { if (it.isHeld) it.release() }
        multicast = null
        service = null
        runningRule = null
        mutableState.value = ServerState()
    }

    override fun close() = stop()
}
