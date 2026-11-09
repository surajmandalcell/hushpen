package com.mandalsuraj.hushpen.server

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import androidx.camera.core.CameraSelector
import androidx.camera.core.ExperimentalGetImage
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.core.content.ContextCompat
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ViewModel
import com.google.mlkit.vision.barcode.BarcodeScanner
import com.google.mlkit.vision.barcode.BarcodeScannerOptions
import com.google.mlkit.vision.barcode.BarcodeScanning
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.common.InputImage
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger

data class ScannerState(
    val permission: Boolean = false,
    val code: String? = null,
    val notice: String? = null,
)

class ScannerViewModel : ViewModel() {
    private val mutableState = MutableStateFlow(ScannerState())
    val state = mutableState.asStateFlow()
    private val epoch = AtomicInteger()
    private val scanning = AtomicBoolean()
    private val delivered = AtomicBoolean()
    private val executor = Executors.newSingleThreadExecutor()
    private var scanner: BarcodeScanner? = null
    private var provider: ProcessCameraProvider? = null
    private var analysis: ImageAnalysis? = null
    private var preview: Preview? = null

    fun refreshPermission(context: Context) = permission(
        ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED
    )

    fun permission(granted: Boolean) {
        mutableState.update {
            it.copy(permission = granted, notice = if (granted) null else "Allow Camera to scan the desktop QR code")
        }
    }

    @androidx.annotation.OptIn(markerClass = [ExperimentalGetImage::class])
    fun bind(context: Context, owner: LifecycleOwner, view: PreviewView) {
        stop()
        delivered.set(false)
        val current = epoch.incrementAndGet()
        val main = ContextCompat.getMainExecutor(context)
        val future = ProcessCameraProvider.getInstance(context)
        future.addListener({
            if (epoch.get() != current) return@addListener
            try {
                val camera = future.get()
                val decoder = BarcodeScanning.getClient(
                    BarcodeScannerOptions.Builder().setBarcodeFormats(Barcode.FORMAT_QR_CODE).build()
                )
                scanner = decoder
                provider = camera
                val cameraPreview = Preview.Builder().build().also {
                    it.setSurfaceProvider(view.surfaceProvider)
                }
                val cameraAnalysis = ImageAnalysis.Builder()
                    .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                    .build()
                preview = cameraPreview
                analysis = cameraAnalysis
                cameraAnalysis.setAnalyzer(executor) { image ->
                    val media = image.image
                    if (media == null || epoch.get() != current || !scanning.compareAndSet(false, true)) {
                        image.close()
                    } else {
                        try {
                            decoder.process(InputImage.fromMediaImage(media, image.imageInfo.rotationDegrees))
                                .addOnSuccessListener { codes ->
                                    if (epoch.get() == current) {
                                        codes.firstNotNullOfOrNull { it.rawValue }?.let { code ->
                                            if (delivered.compareAndSet(false, true)) {
                                                mutableState.update { it.copy(code = code, notice = null) }
                                                analysis?.clearAnalyzer()
                                            }
                                        }
                                    }
                                }
                                .addOnFailureListener { problem ->
                                    if (epoch.get() == current) {
                                        mutableState.update { it.copy(notice = problem.message ?: "QR scanning failed") }
                                    }
                                }
                                .addOnCompleteListener {
                                    scanning.set(false)
                                    image.close()
                                }
                        } catch (problem: Exception) {
                            scanning.set(false)
                            image.close()
                            if (epoch.get() == current) {
                                mutableState.update { it.copy(notice = problem.message ?: "QR scanning failed") }
                            }
                        }
                    }
                }
                camera.bindToLifecycle(owner, CameraSelector.DEFAULT_BACK_CAMERA, cameraPreview, cameraAnalysis)
            } catch (problem: Exception) {
                stop()
                mutableState.update { it.copy(notice = problem.message ?: "Camera could not start") }
            }
        }, main)
    }

    fun stop() {
        epoch.incrementAndGet()
        mutableState.update { it.copy(code = null) }
        analysis?.clearAnalyzer()
        analysis?.let { provider?.unbind(it) }
        preview?.let { provider?.unbind(it) }
        analysis = null
        preview = null
        provider = null
        scanner?.close()
        scanner = null
    }

    fun takeCode(): String? {
        val code = mutableState.value.code
        mutableState.update { it.copy(code = null) }
        return code
    }

    override fun onCleared() {
        stop()
        executor.shutdown()
    }
}
