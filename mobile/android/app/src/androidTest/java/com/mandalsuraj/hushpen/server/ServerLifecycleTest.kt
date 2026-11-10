package com.mandalsuraj.hushpen.server

import android.os.SystemClock
import androidx.lifecycle.Lifecycle
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.mandalsuraj.hushpen.MainActivity
import com.mandalsuraj.hushpen.PhoneApplication
import com.mandalsuraj.hushpen.core.HostUnloadRule
import java.net.InetSocketAddress
import java.net.Socket
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ServerLifecycleTest {
    @Test fun backgroundServingAndOpenScreenRuleFollowTheSameRustHost() {
        val application = InstrumentationRegistry.getInstrumentation().targetContext.applicationContext as PhoneApplication
        val repository = application.repository
        runBlocking { repository.initialize() }
        val server = repository.serverController
        ActivityScenario.launch(MainActivity::class.java).use { activity ->
            try {
                server.foreground(true)
                server.start(HostUnloadRule.KeepLoaded)
                await { server.state.value.running }
                val port = requireNotNull(server.state.value.port)
                Socket().use { it.connect(InetSocketAddress("127.0.0.1", port), 2_000) }
                activity.moveToState(Lifecycle.State.CREATED)
                SystemClock.sleep(250)
                assertTrue(server.state.value.running)
                server.memoryLow()
                assertTrue(server.state.value.running)
                server.updateRule(HostUnloadRule.WhileServerModeOpen)
                await { !server.state.value.running }
                activity.moveToState(Lifecycle.State.RESUMED)
                server.foreground(true)
                server.start(HostUnloadRule.WhileServerModeOpen)
                await { server.state.value.running }
                activity.moveToState(Lifecycle.State.CREATED)
                await { !server.state.value.running }
                assertFalse(server.state.value.running)
            } finally {
                server.stop()
                await { !server.state.value.running }
            }
        }
    }

    private fun await(ready: () -> Boolean) {
        val deadline = SystemClock.elapsedRealtime() + 20_000
        while (!ready() && SystemClock.elapsedRealtime() < deadline) SystemClock.sleep(50)
        assertTrue("Server mode did not reach the expected state: ${ready()}", ready())
    }
}
