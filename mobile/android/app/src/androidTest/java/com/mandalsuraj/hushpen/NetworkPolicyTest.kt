package com.mandalsuraj.hushpen

import android.security.NetworkSecurityPolicy
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.mandalsuraj.hushpen.core.ModelRepository
import java.io.File
import java.security.KeyStore
import java.io.IOException
import java.net.URL
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import javax.net.ssl.TrustManagerFactory
import javax.net.ssl.X509TrustManager
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.assertThrows
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class NetworkPolicyTest {
    @Test fun rustStillDownloadsAndVerifiesItsPinnedModel() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val directory = File(context.cacheDir, "network-model-check")
        directory.deleteRecursively()
        val repository = ModelRepository(directory.absolutePath)
        val timer = Executors.newSingleThreadScheduledExecutor()
        timer.schedule({ repository.cancelDownload() }, 300, TimeUnit.SECONDS)
        try {
            assertTrue("Rust must finish its pinned HTTPS model download.", repository.download("base-q5_1", null))
            assertTrue(File(repository.modelPath("base-q5_1")).isFile)
        } finally {
            timer.shutdownNow()
            repository.close()
            directory.deleteRecursively()
        }
    }

    @Test fun sdkHttpIsRejectedBeforeOpeningAConnection() {
        for (scheme in listOf("http", "https")) {
            assertThrows(IOException::class.java) {
                URL("$scheme://firebaselogging.googleapis.com/").openConnection()
            }
        }
    }

    @Test fun platformSdkTrafficHasNoCleartextOrTrustedTlsRoute() {
        assertFalse(NetworkSecurityPolicy.getInstance().isCleartextTrafficPermitted)
        val factory = TrustManagerFactory.getInstance(TrustManagerFactory.getDefaultAlgorithm())
        factory.init(null as KeyStore?)
        val trust = factory.trustManagers.filterIsInstance<X509TrustManager>()
        assertTrue("The Android TLS policy must have a trust manager.", trust.isNotEmpty())
        assertTrue("Platform SDKs must not trust a remote TLS server.", trust.all { it.acceptedIssuers.isEmpty() })
    }
}
