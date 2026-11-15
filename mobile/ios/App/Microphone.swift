import AVFoundation
import Foundation
import OSLog

@MainActor
final class Microphone: NSObject, AVAudioRecorderDelegate {
    private var recorder: AVAudioRecorder?
    private var holder: AVAudioEngine?
    var failed: ((String) -> Void)?
    var finished: (() -> Void)?
    private var interruption: NSObjectProtocol?
    private var sessionActive = false
    private let level = FlowLevel.Publisher()
    private var meterTimer: Timer?

    override init() {
        super.init()
        interruption = NotificationCenter.default.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { [weak self] notification in
            guard let type = notification.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
                  type == AVAudioSession.InterruptionType.began.rawValue else { return }
            // The session stays active while text is made, so only a live recording counts as interrupted.
            Task { @MainActor in
                guard let self, self.recorder != nil else { return }
                self.failed?("Recording was interrupted. Start again from the keyboard.")
            }
        }
    }

    isolated deinit {
        if let interruption { NotificationCenter.default.removeObserver(interruption) }
    }

    func start(url: URL) throws {
        guard AVAudioApplication.shared.recordPermission == .granted else {
            throw MobileError.Failed(detail: "Allow the microphone in Settings.")
        }
        let audio = AVAudioSession.sharedInstance()
        try audio.setCategory(.record, mode: .measurement)
        try audio.setActive(true)
        sessionActive = true
        do {
            let next = try AVAudioRecorder(url: url, settings: [
                AVFormatIDKey: kAudioFormatLinearPCM,
                AVSampleRateKey: 16_000,
                AVNumberOfChannelsKey: 1,
                AVLinearPCMBitDepthKey: 16,
                AVLinearPCMIsFloatKey: false,
                AVLinearPCMIsBigEndianKey: false,
            ])
            next.delegate = self
            next.isMeteringEnabled = true
            guard next.record(forDuration: 5 * 60) else {
                throw MobileError.Failed(detail: "The microphone could not start.")
            }
            recorder = next
            startMeter {
                next.updateMeters()
                return FlowLevel.normalized(decibels: next.averagePower(forChannel: 0))
            }
        } catch {
            release()
            throw error
        }
    }

    /// Ends the take and closes its file. With `keepAlive`, the microphone stays open (its sound is thrown away) until
    /// `release()`: iOS keeps a background app running only while it records, and a background task ends after
    /// about 30 seconds, which a slow CPU transcription can pass.
    func stop(keepAlive: Bool = false) {
        stopMeter()
        guard let recorder else { return }
        // The open microphone must start before the take stops, while the app still records in the background.
        if keepAlive { holdMicrophone() }
        recorder.delegate = nil
        recorder.stop()
        self.recorder = nil
    }

    /// Whether the microphone stays open after the take, to keep the app running.
    var holdingMicrophone: Bool { holder != nil }

    /// Runs on the audio thread, so it must not inherit the main actor (Swift traps on that).
    private nonisolated static func discard(_: AVAudioPCMBuffer, _: AVAudioTime) {}

    private func holdMicrophone() {
        guard holder == nil else { return }
        let engine = AVAudioEngine()
        let input = engine.inputNode
        input.installTap(onBus: 0, bufferSize: 4_096, format: input.outputFormat(forBus: 0), block: Self.discard)
        do {
            try engine.start()
            holder = engine
        } catch {
            input.removeTap(onBus: 0)
            Logger(subsystem: "com.mandalsuraj.hushpen", category: "Microphone").error("Microphone hold did not start: \(error.localizedDescription, privacy: .public)")
        }
    }

    /// Sends the loudness to the keyboard about 12 times a second while the take runs.
    private func startMeter(_ read: @escaping @MainActor () -> Double) {
        meterTimer?.invalidate()
        meterTimer = Timer.scheduledTimer(withTimeInterval: 0.08, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.level.set(read()) }
        }
    }

    private func stopMeter() {
        guard meterTimer != nil else { return }
        meterTimer?.invalidate()
        meterTimer = nil
        level.set(0)
    }

    #if DEBUG
    /// A test recording has no live microphone, so its own loudness drives the level.
    func meter(fixture url: URL) {
        guard let data = try? Data(contentsOf: url), data.count > 44 else { return }
        let begin = Date()
        startMeter {
            let start = Int(Date().timeIntervalSince(begin) * 16_000)
            let count = min(1_600, (data.count - 44) / 2 - start)
            guard count > 0 else { return 0 }
            var sum = 0.0
            for index in start..<(start + count) {
                let sample = Double(data.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 44 + 2 * index, as: Int16.self) })
                sum += sample * sample
            }
            let rms = (sum / Double(count)).squareRoot() / 32_768
            return FlowLevel.normalized(decibels: Float(20 * log10(max(rms, 1e-5))))
        }
    }
    #endif

    func release() {
        if let holder {
            holder.inputNode.removeTap(onBus: 0)
            holder.stop()
            self.holder = nil
        }
        guard sessionActive else { return }
        sessionActive = false
        do { try AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation) }
        catch { Logger(subsystem: "com.mandalsuraj.hushpen", category: "Microphone").error("Audio session did not stop: \(error.localizedDescription, privacy: .public)") }
    }

    nonisolated func audioRecorderEncodeErrorDidOccur(_ recorder: AVAudioRecorder, error: Error?) {
        let recorderID = ObjectIdentifier(recorder)
        let message = error?.localizedDescription ?? "Microphone recording failed."
        Task { @MainActor [weak self] in
            guard let self, self.recorder.map({ ObjectIdentifier($0) }) == recorderID else { return }
            self.failed?(message)
        }
    }

    nonisolated func audioRecorderDidFinishRecording(_ recorder: AVAudioRecorder, successfully flag: Bool) {
        let recorderID = ObjectIdentifier(recorder)
        Task { @MainActor [weak self] in
            guard let self, self.recorder.map({ ObjectIdentifier($0) }) == recorderID else { return }
            if flag { self.finished?() }
            else { self.failed?("Microphone recording failed.") }
        }
    }
}
