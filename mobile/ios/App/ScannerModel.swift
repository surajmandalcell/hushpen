import AVFoundation
import Observation
import SwiftUI
import UIKit

@Observable @MainActor
final class ScannerModel {
    private let capture = QrCapture()
    private(set) var permission = "Not requested"
    private(set) var scanning = false
    private(set) var starting = false
    private(set) var value = ""
    var notice = ""
    private var generation = UUID()
    private var stopping: Task<Void, Never>?

    var session: AVCaptureSession { capture.session }

    func start() async {
        guard !starting && !scanning else { return }
        starting = true
        defer { starting = false }
        let token = UUID()
        generation = token
        value = ""
        notice = ""
        await stopping?.value
        guard generation == token else { return }
        let status = AVCaptureDevice.authorizationStatus(for: .video)
        let allowed: Bool
        if status == .notDetermined { allowed = await AVCaptureDevice.requestAccess(for: .video) }
        else { allowed = status == .authorized }
        guard generation == token else { return }
        guard allowed else {
            permission = status == .restricted ? "Restricted" : "Denied"
            notice = "Allow the camera in Settings, or paste the desktop pairing code."
            return
        }
        permission = "Allowed"
        capture.found = { [weak self] value in
            Task { @MainActor in
                guard let self, self.generation == token, self.scanning else { return }
                self.value = value
                self.stop()
            }
        }
        do {
            try await capture.start()
            guard generation == token else { await capture.stop(); return }
            scanning = true
        } catch {
            if generation == token { notice = phoneError(error) }
            await capture.stop()
        }
    }

    func stop() {
        generation = UUID()
        scanning = false
        let previous = stopping
        stopping = Task { [capture] in
            await previous?.value
            await capture.stop()
        }
    }

    func openSettings() {
        guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
        UIApplication.shared.open(url)
    }
}

private final class QrCapture: NSObject, AVCaptureMetadataOutputObjectsDelegate, @unchecked Sendable {
    let session = AVCaptureSession()
    private let queue = DispatchQueue(label: "com.mandalsuraj.hushpen.camera")
    private let lock = NSLock()
    private var callback: (@Sendable (String) -> Void)?
    var found: (@Sendable (String) -> Void)? {
        get { lock.withLock { callback } }
        set { lock.withLock { callback = newValue } }
    }

    func start() async throws {
        try await withCheckedThrowingContinuation { continuation in
            queue.async { [self] in
                do {
                    if session.inputs.isEmpty {
                        guard let device = AVCaptureDevice.default(for: .video) else {
                            throw MobileError.Failed(detail: "No camera is available. Paste the desktop pairing code.")
                        }
                        let input = try AVCaptureDeviceInput(device: device)
                        let output = AVCaptureMetadataOutput()
                        guard session.canAddInput(input), session.canAddOutput(output) else {
                            throw MobileError.Failed(detail: "The camera cannot scan QR codes.")
                        }
                        session.beginConfiguration()
                        session.addInput(input)
                        session.addOutput(output)
                        output.setMetadataObjectsDelegate(self, queue: queue)
                        output.metadataObjectTypes = [.qr]
                        session.commitConfiguration()
                    }
                    session.startRunning()
                    guard session.isRunning else { throw MobileError.Failed(detail: "The camera could not start.") }
                    continuation.resume()
                } catch { continuation.resume(throwing: error) }
            }
        }
    }

    func stop() async {
        await withCheckedContinuation { continuation in
            queue.async { [self] in
                session.stopRunning()
                continuation.resume()
            }
        }
    }

    func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput metadataObjects: [AVMetadataObject], from connection: AVCaptureConnection) {
        guard let code = metadataObjects.compactMap({ $0 as? AVMetadataMachineReadableCodeObject }).first?.stringValue else { return }
        found?(code)
    }
}

struct CameraPreview: UIViewRepresentable {
    let session: AVCaptureSession

    func makeUIView(context: Context) -> CameraView {
        let view = CameraView()
        view.preview.session = session
        view.preview.videoGravity = .resizeAspectFill
        return view
    }

    func updateUIView(_ uiView: CameraView, context: Context) {}
}

final class CameraView: UIView {
    override class var layerClass: AnyClass { AVCaptureVideoPreviewLayer.self }
    var preview: AVCaptureVideoPreviewLayer {
        guard let preview = layer as? AVCaptureVideoPreviewLayer else { preconditionFailure("The camera requires a preview layer.") }
        return preview
    }
}
