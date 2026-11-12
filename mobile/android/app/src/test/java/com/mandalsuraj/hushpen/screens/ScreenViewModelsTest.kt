package com.mandalsuraj.hushpen.screens

import com.mandalsuraj.hushpen.DownloadState
import com.mandalsuraj.hushpen.core.Model
import com.mandalsuraj.hushpen.core.HostUnloadRule
import com.mandalsuraj.hushpen.server.ServerState
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class ScreenViewModelsTest {
    private val dispatcher = StandardTestDispatcher()

    @Before fun before() { Dispatchers.setMain(dispatcher) }
    @After fun after() { Dispatchers.resetMain() }

    @Test fun onboardingFactsCanChangeAfterReturningFromSettings() {
        val model = OnboardingViewModel()
        assertEquals(OnboardingState(), model.state.value)
        model.update(true, true, false)
        assertEquals(OnboardingState(true, true, false), model.state.value)
        model.update(false, false, false)
        assertEquals(OnboardingState(), model.state.value)
    }

    @Test fun modelIntentsReachTheRepositoryAndCancellationIsImmediate() = runTest(dispatcher) {
        val data = FakeModels()
        val model = ModelsViewModel(data)
        advanceUntilIdle()
        model.download("base-q5_1")
        advanceUntilIdle()
        model.select("base-q5_1")
        advanceUntilIdle()
        model.cancelDownload()
        assertEquals(listOf("refresh", "download:base-q5_1", "select:base-q5_1", "cancel"), data.actions)
        assertEquals("base-q5_1", model.selectedModel.value)
    }

    @Test fun failedVerificationLeavesAnActionableNotice() = runTest(dispatcher) {
        val data = FakeModels()
        val model = ModelsViewModel(data)
        advanceUntilIdle()
        model.verify("base-q5_1")
        advanceUntilIdle()
        assertEquals("The model did not pass verification.", model.notice.value)
    }

    @Test fun modelFailureKeepsTheSelectionAndCanRecover() = runTest(dispatcher) {
        val data = FakeModels()
        val model = ModelsViewModel(data)
        advanceUntilIdle()
        data.fail = true
        model.select("missing")
        advanceUntilIdle()
        assertEquals("base-q5_1", model.selectedModel.value)
        assertEquals("Model unavailable", model.notice.value)
        data.fail = false
        model.refresh()
        advanceUntilIdle()
        assertEquals(null, model.notice.value)
    }

    @Test fun settingsIntentsUseRepositoryState() = runTest(dispatcher) {
        val data = FakeSettings()
        val model = SettingsViewModel(data)
        model.setCleanup(true)
        model.setHaptics(false)
        advanceUntilIdle()
        assertTrue(model.cleanupEnabled.value)
        assertFalse(model.hapticsEnabled.value)
    }

    @Test fun settingsSaveFailureKeepsTheOldValue() = runTest(dispatcher) {
        val data = FakeSettings().also { it.fail = true }
        val model = SettingsViewModel(data)
        model.setCleanup(true)
        advanceUntilIdle()
        assertFalse(model.cleanupEnabled.value)
        assertEquals("Storage full", model.notice.value)
    }

    @Test fun scannerForwardsTheWholeOfferToRustWithoutParsingIt() = runTest(dispatcher) {
        val data = FakeDevices()
        val model = DevicesViewModel(data)
        model.scan()
        assertTrue(model.scannerOpen.value)
        val offer = "hushpen-pair://v1?host=100.64.0.1&port=7342&otp=123456"
        model.scanned(offer)
        advanceUntilIdle()
        assertFalse(model.scannerOpen.value)
        assertEquals(offer, model.pairingUri.value)
        assertEquals(listOf("pair:$offer"), data.actions)
    }

    @Test fun devicesFollowServerStateAndForwardServingAndForgetIntents() = runTest(dispatcher) {
        val data = FakeDevices()
        val model = DevicesViewModel(data)
        advanceUntilIdle()
        data.serverState.value = ServerState(true, 7342)
        advanceUntilIdle()
        assertTrue(model.server.value.running)
        model.setServing(false)
        model.forget("desktop")
        advanceUntilIdle()
        assertEquals(listOf("stop", "forget:desktop"), data.actions)
    }

    @Test fun invalidIdleInputDoesNotReachTheHost() = runTest(dispatcher) {
        val data = FakeDevices()
        val model = DevicesViewModel(data)
        model.setIdleMinutes("-1")
        model.applyIdleMinutes()
        advanceUntilIdle()
        assertEquals("Enter idle minutes as a whole number.", model.notice.value)
        assertTrue(data.actions.isEmpty())
    }

    private class FakeModels : ModelsData {
        override val models = MutableStateFlow<List<Model>>(emptyList())
        override val selectedModel = MutableStateFlow("base-q5_1")
        override val selectedCleanupModel = MutableStateFlow("s1-mini")
        override val download = MutableStateFlow(DownloadState())
        val actions = mutableListOf<String>()
        var fail = false
        override suspend fun initialize() = Unit
        override suspend fun refresh() { actions += "refresh" }
        override suspend fun download(id: String) { actions += "download:$id" }
        override suspend fun verify(id: String) = false
        override suspend fun selectModel(id: String) {
            check(!fail) { "Model unavailable" }
            actions += "select:$id"
            selectedModel.value = id
        }
        override fun cancelDownload() { actions += "cancel" }
    }

    private class FakeSettings : SettingsData {
        override val cleanupEnabled = MutableStateFlow(false)
        override val hapticsEnabled = MutableStateFlow(true)
        override val systemAvailable = MutableStateFlow(false)
        var fail = false
        override suspend fun setCleanup(enabled: Boolean) {
            check(!fail) { "Storage full" }
            cleanupEnabled.value = enabled
        }
        override suspend fun setHaptics(enabled: Boolean) { hapticsEnabled.value = enabled }
    }

    private class FakeDevices : DevicesData {
        override val pairedClients = MutableStateFlow(listOf("desktop"))
        override val allowedRules = MutableStateFlow<List<HostUnloadRule>>(emptyList())
        override val unloadRule = MutableStateFlow<HostUnloadRule>(HostUnloadRule.KeepLoaded)
        override val serverState = MutableStateFlow(ServerState())
        val actions = mutableListOf<String>()
        override suspend fun initialize() = Unit
        override suspend fun refresh() = Unit
        override suspend fun pair(uri: String) { actions += "pair:$uri" }
        override suspend fun forget(id: String) { actions += "forget:$id" }
        override suspend fun setUnloadRule(rule: HostUnloadRule) { unloadRule.value = rule }
        override fun startServing() { actions += "start" }
        override fun stopServing() { actions += "stop" }
    }
}
