package com.mandalsuraj.hushpen

import android.content.Context
import android.app.ActivityManager
import android.os.Build
import com.mandalsuraj.hushpen.core.DownloadProgress
import com.mandalsuraj.hushpen.core.Engine
import com.mandalsuraj.hushpen.core.Host
import com.mandalsuraj.hushpen.core.HostUnloadRule
import com.mandalsuraj.hushpen.core.Model
import com.mandalsuraj.hushpen.core.ModelKind
import com.mandalsuraj.hushpen.core.ModelStatus
import com.mandalsuraj.hushpen.core.ModelRepository
import com.mandalsuraj.hushpen.core.LocalCleanup
import com.mandalsuraj.hushpen.core.SystemCleanup
import com.mandalsuraj.hushpen.core.cleanText
import com.mandalsuraj.hushpen.server.NativeSystemLlm
import com.mandalsuraj.hushpen.server.ServerController
import com.mandalsuraj.hushpen.server.ServerState
import com.mandalsuraj.hushpen.screens.DevicesData
import com.mandalsuraj.hushpen.screens.ModelsData
import com.mandalsuraj.hushpen.screens.SettingsData
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

data class DownloadState(val modelId: String? = null, val done: ULong = 0u, val total: ULong = 0u, val verifying: Boolean = false)

class PhoneRepository(private val context: Context) : ModelsData, SettingsData, DevicesData {
    private val preferences = context.getSharedPreferences("phone", Context.MODE_PRIVATE)
    private val initialization = Mutex()
    private val speech = Mutex()
    private val cleanupSelection = Mutex()
    @Volatile private var ready = false
    @Volatile private var activityVisible = false
    private lateinit var catalog: ModelRepository
    private lateinit var nativeSystem: NativeSystemLlm
    private lateinit var system: SystemCleanup
    private lateinit var local: LocalCleanup
    lateinit var engine: Engine
        private set
    lateinit var host: Host
        private set
    lateinit var serverController: ServerController
        private set
    override val serverState: StateFlow<ServerState> get() = serverController.state
    private val mutableModels = MutableStateFlow<List<Model>>(emptyList())
    override val models: StateFlow<List<Model>> = mutableModels
    private val mutableSelected = MutableStateFlow(preferences.getString("speechModel", "").orEmpty())
    override val selectedModel: StateFlow<String> = mutableSelected
    private val mutableSelectedCleanup = MutableStateFlow(preferences.getString("cleanupModel", "s1-mini").orEmpty())
    override val selectedCleanupModel: StateFlow<String> = mutableSelectedCleanup
    private val mutableCleanup = MutableStateFlow(preferences.getBoolean("cleanup", false))
    override val cleanupEnabled: StateFlow<Boolean> = mutableCleanup
    private val mutableHaptics = MutableStateFlow(preferences.getBoolean("haptics", true))
    override val hapticsEnabled: StateFlow<Boolean> = mutableHaptics
    private val mutableDownload = MutableStateFlow(DownloadState())
    override val download: StateFlow<DownloadState> = mutableDownload
    private val mutableSystemAvailable = MutableStateFlow(false)
    override val systemAvailable: StateFlow<Boolean> = mutableSystemAvailable
    private val mutableClients = MutableStateFlow<List<String>>(emptyList())
    override val pairedClients: StateFlow<List<String>> = mutableClients
    private val mutableRules = MutableStateFlow<List<HostUnloadRule>>(emptyList())
    override val allowedRules: StateFlow<List<HostUnloadRule>> = mutableRules
    private val mutableRule = MutableStateFlow<HostUnloadRule>(readRule())
    override val unloadRule: StateFlow<HostUnloadRule> = mutableRule

    override suspend fun initialize() = withContext(Dispatchers.IO) {
        initialization.withLock {
            if (ready) return@withLock
            val data = File(context.filesDir, "hushpen").absolutePath
            val memory = ActivityManager.MemoryInfo().also {
                context.getSystemService(ActivityManager::class.java).getMemoryInfo(it)
            }.totalMem.toULong()
            catalog = ModelRepository.withMemory(data, memory)
            engine = Engine(data)
            local = LocalCleanup(data, memory)
            local.setMemoryBudget(memory)
            val cleanupModels = catalog.models().filter { it.kind == ModelKind.CLEANUP }
            val cleanupModel = cleanupModels.firstOrNull { it.id == mutableSelectedCleanup.value }
                ?: cleanupModels.firstOrNull { it.recommended }
                ?: cleanupModels.firstOrNull()
            cleanupModel?.let { local.setModel(it.id) }
            mutableSelectedCleanup.value = cleanupModel?.id.orEmpty()
            nativeSystem = NativeSystemLlm().also {
                it.enabled = mutableCleanup.value
                it.foreground = activityVisible
            }
            system = SystemCleanup(nativeSystem)
            system.setLocal(if (mutableCleanup.value) local else null)
            host = Host(engine, Build.MODEL.take(32).ifBlank { "Android phone" }, system)
            serverController = ServerController(context.applicationContext, host)
            mutableModels.value = catalog.models()
            if (mutableSelected.value.isEmpty()) {
                mutableSelected.value = mutableModels.value.firstOrNull { it.kind == ModelKind.SPEECH && it.recommended }?.id.orEmpty()
            }
            mutableRules.value = host.allowedUnloadRules()
            mutableSystemAvailable.value = system.available()
            mutableClients.value = host.pairedClients()
            ready = true
            nativeSystem.foreground = activityVisible
            serverController.foreground(activityVisible)
        }
    }

    override suspend fun refresh() = withContext(Dispatchers.IO) {
        initialize()
        mutableModels.value = catalog.models()
        mutableSystemAvailable.value = system.available()
        mutableClients.value = host.pairedClients()
    }

    override suspend fun download(id: String) = withContext(Dispatchers.IO) {
        initialize()
        check(mutableDownload.compareAndSet(DownloadState(), DownloadState(modelId = id))) {
            "A model download is already running."
        }
        try {
            val downloaded = catalog.download(id, object : DownloadProgress {
                override fun bytes(done: ULong, total: ULong) {
                    mutableDownload.value = DownloadState(id, done, total)
                }
                override fun verifying() {
                    mutableDownload.value = mutableDownload.value.copy(verifying = true)
                }
            })
            if (downloaded) {
                mutableModels.value = catalog.models()
                if (catalog.models().any { it.id == id && it.kind == ModelKind.CLEANUP }) {
                    if (catalog.models().none { it.id == mutableSelectedCleanup.value && it.status == ModelStatus.READY }) selectModel(id)
                } else {
                    val selected = catalog.selectionAfterDownload(mutableSelected.value, id)
                    if (selected != mutableSelected.value) selectModel(selected)
                }
            }
            refresh()
        } finally {
            mutableDownload.value = DownloadState()
        }
    }

    override fun cancelDownload() { if (ready) catalog.cancelDownload() }

    fun cancelSpeech() { if (ready) engine.cancel() }

    internal fun shutdown() {
        if (ready) {
            host.stop()
            local.shutdown()
            engine.shutdown()
        }
    }

    suspend fun releaseSpeech() = withContext(Dispatchers.IO) {
        if (ready) speech.withLock { host.releaseLocalModel() }
    }

    fun setActivityVisible(visible: Boolean) {
        activityVisible = visible
        if (ready) {
            nativeSystem.foreground = activityVisible
            serverController.foreground(visible)
        }
    }


    override suspend fun verify(id: String): Boolean = withContext(Dispatchers.IO) {
        initialize()
        catalog.verify(id).also { refresh() }
    }

    override suspend fun selectModel(id: String) = withContext(Dispatchers.IO) {
        initialize()
        catalog.modelPath(id)
        if (catalog.models().any { it.id == id && it.kind == ModelKind.CLEANUP }) {
            cleanupSelection.withLock {
                val previous = mutableSelectedCleanup.value
                if (!preferences.edit().putString("cleanupModel", id).commit()) {
                    preferences.edit().putString("cleanupModel", previous).commit()
                    error("Could not save the cleanup model.")
                }
                try {
                    local.setModel(id)
                } catch (problem: Exception) {
                    preferences.edit().putString("cleanupModel", previous).commit()
                    throw problem
                }
                mutableSelectedCleanup.value = id
            }
            return@withContext
        }
        speech.withLock {
            engine.unload()
            if (!preferences.edit().putString("speechModel", id).commit()) {
                preferences.edit().putString("speechModel", mutableSelected.value).commit()
                error("Could not save the speech model.")
            }
            mutableSelected.value = id
        }
    }

    suspend fun loadSelectedModel() = withContext(Dispatchers.IO) {
        initialize()
        speech.withLock { engine.load(mutableSelected.value) }
    }

    suspend fun transcribe(wav: File): String = withContext(Dispatchers.IO) {
        initialize()
        speech.withLock {
            currentCoroutineContext().ensureActive()
            engine.load(mutableSelected.value)
            currentCoroutineContext().ensureActive()
            engine.transcribe(wav.absolutePath, null).text
        }
    }

    suspend fun cleanup(raw: String): String = withContext(Dispatchers.IO) {
        initialize()
        if (mutableCleanup.value) releaseSpeech()
        cleanText(raw, if (mutableCleanup.value) system else null).text
    }

    override suspend fun setCleanup(enabled: Boolean) = withContext(Dispatchers.IO) {
        initialize()
        cleanupSelection.withLock {
            if (!preferences.edit().putBoolean("cleanup", enabled).commit()) {
                preferences.edit().putBoolean("cleanup", mutableCleanup.value).commit()
                error("Could not save cleanup.")
            }
            nativeSystem.enabled = enabled
            system.setLocal(if (enabled) local else null)
            mutableCleanup.value = enabled
            mutableSystemAvailable.value = system.available()
        }
    }

    override suspend fun setHaptics(enabled: Boolean) = withContext(Dispatchers.IO) {
        check(preferences.edit().putBoolean("haptics", enabled).commit()) { "Could not save haptics." }
        mutableHaptics.value = enabled
    }

    override suspend fun setUnloadRule(rule: HostUnloadRule) = withContext(Dispatchers.IO) {
        initialize()
        host.validateUnloadRule(rule)
        val value = when (rule) {
            HostUnloadRule.KeepLoaded -> "keep"
            is HostUnloadRule.AfterIdle -> "idle"
            HostUnloadRule.WhenMemoryLow -> "memory"
            HostUnloadRule.WhileServerModeOpen -> "foreground"
        }
        check(preferences.edit().putString("unloadRule", value).putInt("idleMinutes", (rule as? HostUnloadRule.AfterIdle)?.minutes?.toInt() ?: 30).commit()) { "Could not save the unload rule." }
        mutableRule.value = rule
        if (serverController.state.value.running) {
            serverController.updateRule(rule)
        }
    }

    override suspend fun pair(payload: String) = withContext(Dispatchers.IO) {
        initialize()
        host.pair(payload)
        mutableClients.value = host.pairedClients()
    }

    override suspend fun forget(id: String) = withContext(Dispatchers.IO) {
        initialize()
        host.forget(id)
        serverController.stop()
        mutableClients.value = host.pairedClients()
    }

    override fun startServing() = serverController.start(mutableRule.value)
    override fun stopServing() = serverController.stop()

    private fun readRule(): HostUnloadRule = when (preferences.getString("unloadRule", "keep")) {
        "idle" -> HostUnloadRule.AfterIdle(preferences.getInt("idleMinutes", 30).toUInt())
        "memory" -> HostUnloadRule.WhenMemoryLow
        "foreground" -> HostUnloadRule.WhileServerModeOpen
        else -> HostUnloadRule.KeepLoaded
    }
}
