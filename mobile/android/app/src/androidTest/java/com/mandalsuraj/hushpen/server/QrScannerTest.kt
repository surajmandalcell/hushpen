package com.mandalsuraj.hushpen.server

import android.Manifest
import android.content.Context
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraManager
import android.net.Uri
import android.os.Handler
import android.os.Looper
import androidx.activity.ComponentActivity
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import com.google.android.gms.tasks.Tasks
import com.google.mlkit.vision.barcode.BarcodeScannerOptions
import com.google.mlkit.vision.barcode.BarcodeScanning
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.common.InputImage
import java.io.File
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicReference
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.RuleChain
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class QrScannerTest {
    private val compose = createAndroidComposeRule<ComponentActivity>()
    @get:Rule val rules: RuleChain = RuleChain.outerRule(GrantPermissionRule.grant(Manifest.permission.CAMERA)).around(compose)

    @Test fun desktopQrImageDecodesWithoutExposingThePairingCode() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val image = File(context.filesDir, "qr-test.png")
        assertTrue("Stage the desktop QR image at files/qr-test.png before this test", image.isFile)
        val scanner = BarcodeScanning.getClient(
            BarcodeScannerOptions.Builder().setBarcodeFormats(Barcode.FORMAT_QR_CODE).build()
        )
        try {
            val codes = Tasks.await(
                scanner.process(InputImage.fromFilePath(context, Uri.fromFile(image))),
                20,
                TimeUnit.SECONDS,
            )
            assertTrue("The desktop image must contain one pairing QR code", codes.count {
                it.rawValue?.startsWith("hushpen-pair://v1?") == true
            } == 1)
        } finally {
            scanner.close()
        }
    }

    @Test fun placeholderScannerOpensAndReleasesTheCamera() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val cameras = context.getSystemService(Context.CAMERA_SERVICE) as CameraManager
        val back = cameras.cameraIdList.firstOrNull {
            cameras.getCameraCharacteristics(it).get(CameraCharacteristics.LENS_FACING) == CameraCharacteristics.LENS_FACING_BACK
        } ?: throw AssertionError("The emulator must provide a back camera")
        val available = AtomicBoolean(false)
        val callback = object : CameraManager.AvailabilityCallback() {
            override fun onCameraAvailable(cameraId: String) {
                if (cameraId == back) available.set(true)
            }
            override fun onCameraUnavailable(cameraId: String) {
                if (cameraId == back) available.set(false)
            }
        }
        val model = AtomicReference<ScannerViewModel>()
        val shown = mutableStateOf(true)
        cameras.registerAvailabilityCallback(callback, Handler(Looper.getMainLooper()))
        try {
            compose.waitUntil(10_000) { available.get() }
            compose.setContent {
                val scanner: ScannerViewModel = viewModel()
                model.set(scanner)
                if (shown.value) PlaceholderScanner(onCode = {}, onClose = { shown.value = false }, model = scanner)
            }
            compose.onNodeWithText("Scan desktop QR code").assertExists()
            compose.waitUntil(20_000) {
                !available.get() || model.get()?.state?.value?.notice != null
            }
            assertNull("CameraX must bind without a scanner error", model.get().state.value.notice)
            assertFalse("CameraX must open the back camera", available.get())
            compose.runOnIdle { shown.value = false }
            compose.waitUntil(10_000) { available.get() }
            assertNull("CameraX teardown must not create a scanner error", model.get().state.value.notice)
        } finally {
            compose.runOnIdle { shown.value = false }
            cameras.unregisterAvailabilityCallback(callback)
        }
    }
}
