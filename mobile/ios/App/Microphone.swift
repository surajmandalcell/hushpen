import AVFoundation
import Foundation

@MainActor
final class Microphone: NSObject, AVAudioRecorderDelegate {
    private var recorder: AVAudioRecorder?
    var failed: ((String) -> Void)?
    var finished: (() -> Void)?
    private var interruption: NSObjectProtocol?

    override init() {
        super.init()
        interruption = NotificationCenter.default.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { [weak self] notification in
            guard let type = notification.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
                  type == AVAudioSession.InterruptionType.began.rawValue else { return }
            Task { @MainActor in self?.failed?("Recording was interrupted. Start again from the keyboard.") }
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
            guard next.record(forDuration: 5 * 60) else {
                throw MobileError.Failed(detail: "The microphone could not start.")
            }
            recorder = next
        } catch {
            try? audio.setActive(false, options: .notifyOthersOnDeactivation)
            throw error
        }
    }

    func stop() {
        guard recorder != nil else { return }
        recorder?.delegate = nil
        recorder?.stop()
        recorder = nil
        do { try AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation) }
        catch { failed?(error.localizedDescription) }
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
