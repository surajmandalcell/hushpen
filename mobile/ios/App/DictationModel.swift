import AVFoundation
import Foundation
import Observation
import OSLog
import UIKit

@Observable @MainActor
final class DictationModel {
    private let repository: PhoneRepository
    private let store: FlowSessionStore
    private let microphone = Microphone()
    private let activity = DictationActivity()
    private var machine: Dictation
    private var flow: FlowSession?
    private var generation = UUID()
    private var audio: [UInt64: URL] = [:]
    private var notes: [UInt64: RunNotes] = [:]
    private var jobs: [UInt64: Task<Void, Never>] = [:]
    private var timer: Task<Void, Never>?
    private var observation: FlowSessionObservation?
    private var backgroundTask: UIBackgroundTaskIdentifier = .invalid
    private var active = true
    private var failedCode = ""
    private(set) var state = "idle"
    private(set) var sessionID = ""
    private(set) var result = ""
    private(set) var recoverable = false
    private(set) var busy = false
    var notice = ""
    /// Called after a finished dictation is kept in the phone history.
    var kept: (@MainActor () -> Void)?

    init(repository: PhoneRepository, store: FlowSessionStore) throws {
        self.repository = repository
        self.store = store
        machine = try Dictation(maxMinutes: 5)
        try store.cancelAbandonedRecording()
        microphone.failed = { [weak self] message in
            guard let self else { return }
            self.notice = message
            self.abort(failure: keyboardNotice("CAPTURE_FAILED"))
        }
        microphone.finished = { [weak self] in
            guard let self, self.state == "listening" else { return }
            self.send(.toggle)
        }
    }

    func open(url: URL) async {
        let requested: FlowSession
        do { requested = try store.request(for: url) }
        catch { notice = error.localizedDescription; return }
        do {
            abort()
            flow = requested
            observation = try FlowSessionObservation { [weak self] in
                Task { @MainActor in self?.handleSignal() }
            }
            sessionID = requested.id.uuidString
            generation = UUID()
            machine = try Dictation(maxMinutes: 5)
            result = ""
            notice = ""
            if AVAudioApplication.shared.recordPermission != .granted {
                guard await AVAudioApplication.requestRecordPermission() else {
                    throw MobileError.Failed(detail: "Allow the microphone in Settings.")
                }
            }
            guard flow?.id == requested.id else { return }
            guard try store.mayRecord(id: requested.id) else { abort(); return }
            send(.toggle)
        } catch {
            notice = phoneError(error)
            abort(failure: keyboardNotice(""))
        }
    }

    func finish() { send(.toggle) }
    func cancel() { send(.cancel) }
    func recover() { send(.recover) }

    func setActive(_ value: Bool) {
        active = value
        // iOS rejects GPU work from the background, so models loaded from now on run on the CPU.
        setBackground(background: !value)
        updateBackgroundTask()
        if value { handleSignal() }
    }

    /// Ends the dictation. A `failure` tells the keyboard why no text came; without one the session is cancelled.
    func abort(failure: String? = nil) {
        generation = UUID()
        timer?.cancel()
        timer = nil
        observation = nil
        activity.end()
        endBackgroundTask()
        repository.engine.cancel()
        jobs.values.forEach { $0.cancel() }
        jobs.removeAll()
        microphone.stop()
        microphone.release()
        for file in audio.values { try? FileManager.default.removeItem(at: file) }
        audio.removeAll()
        notes.removeAll()
        if let flow {
            do {
                if let failure { try store.fail(id: flow.id, message: failure) }
                else { try store.cancel(id: flow.id) }
            } catch { if notice.isEmpty { notice = error.localizedDescription } }
        }
        flow = nil
        busy = false
        recoverable = false
        state = "idle"
    }

    private func scheduleWake() {
        timer?.cancel()
        timer = nil
        guard let flow else { return }
        let expiry = UInt64(max(1, flow.expiresAt.timeIntervalSinceNow * 1000))
        let delay = min(machine.nextWakeMs(nowMs: now()) ?? expiry, expiry)
        timer = Task { [weak self] in
            do { try await Task.sleep(for: .milliseconds(Int64(clamping: delay))) }
            catch { return }
            guard let self, !Task.isCancelled, let flow = self.flow else { return }
            if flow.expiresAt <= Date() {
                self.notice = "This dictation session expired. Start again from the keyboard."
                self.abort()
                return
            }
            self.send(.tick)
        }
    }

    private func handleSignal() {
        guard let flow else { return }
        do {
            guard let current = try store.current(), current.id == flow.id else { abort(); return }
            if current.state == .cancelled, state != "cancelled" { send(.cancel) }
            else if current.command == .stop, state == "listening" { finish() }
        } catch { notice = error.localizedDescription; abort() }
    }

    private func updateBackgroundTask() {
        if active || !busy || state == "listening" { endBackgroundTask(); return }
        guard backgroundTask == .invalid else { return }
        backgroundTask = UIApplication.shared.beginBackgroundTask(withName: "Dictation") { [weak self] in
            Task { @MainActor in self?.abort(failure: keyboardNotice("BACKGROUND_TIME")) }
        }
    }

    private func endBackgroundTask() {
        guard backgroundTask != .invalid else { return }
        UIApplication.shared.endBackgroundTask(backgroundTask)
        backgroundTask = .invalid
    }

    private func send(_ event: DictationEvent) {
        guard flow != nil else { return }
        do {
            let time = now()
            if case .recover = event, machine.snapshot(nowMs: time).recoverableSession != nil, let flow {
                try store.resume(id: flow.id)
            }
            for effect in try machine.handle(event: event, nowMs: time) { try execute(effect) }
            let snapshot = machine.snapshot(nowMs: now())
            let cancelled = snapshot.state == "cancelled" && state != "cancelled"
            state = snapshot.state
            recoverable = snapshot.recoverableSession != nil
            busy = ["listening", "transcribing", "cleaning"].contains(state)
            if cancelled, let flow { try store.cancel(id: flow.id) }
            if busy, let flow { activity.update(session: flow, phase: state) }
            else { activity.end() }
            updateBackgroundTask()
            if !busy && !recoverable && state != "done" && flow != nil {
                abort(failure: state == "failed" ? keyboardNotice(failedCode) : nil)
            } else { scheduleWake() }
        } catch {
            notice = phoneError(error)
            abort(failure: keyboardNotice(""))
        }
    }

    private func execute(_ effect: DictationEffect) throws {
        switch effect {
        case let .startCapture(session):
            let folder = FileManager.default.temporaryDirectory.appendingPathComponent("dictation", isDirectory: true)
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true, attributes: [.protectionKey: FileProtectionType.complete])
            let file = folder.appendingPathComponent("session-\(UUID().uuidString).wav")
            audio[session] = file
            notes[session] = RunNotes()
            #if DEBUG
            let documentsWav = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0].appendingPathComponent("flow-test.wav")
            let fixture = ProcessInfo.processInfo.environment["HUSHPEN_TEST_WAV"].map {
                $0 == "documents" ? documentsWav : URL(fileURLWithPath: $0)
            } ?? (FileManager.default.fileExists(atPath: documentsWav.path) ? documentsWav : nil)
            if let source = fixture {
                try FileManager.default.copyItem(at: source, to: file)
                try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete], ofItemAtPath: file.path)
                return
            }
            #endif
            try microphone.start(url: file)
            try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete], ofItemAtPath: file.path)
        case let .stopCapture(session, keep):
            // The audio session stays active while the text is made: that keeps iOS from suspending the app in the background.
            microphone.stop()
            notes[session]?.stoppedAt = Date()
            if keep, let flow { try store.stop(id: flow.id) }
            if !keep {
                microphone.release()
                dropAudio(session)
            }
        case .holdCapture:
            microphone.stop()
            microphone.release()
        case let .dropAudio(session):
            dropAudio(session)
        case let .transcribe(session):
            guard let file = audio[session] else { throw MobileError.Failed(detail: "The recording is missing.") }
            let token = generation
            jobs[session] = Task { [weak self, repository] in
                do {
                    let transcript = try await repository.transcribe(wav: file)
                    guard let self, self.generation == token, !Task.isCancelled else { return }
                    self.notes[session]?.heard(transcript)
                    self.send(.transcribed(session: session, text: transcript.text))
                } catch {
                    guard let self, self.generation == token, !Task.isCancelled else { return }
                    Logger(subsystem: "com.mandalsuraj.hushpen", category: "Dictation").error("Transcription failed: \(error.localizedDescription, privacy: .public)")
                    self.send(.transcribeFailed(session: session))
                    self.abort()
                }
            }
        case let .clean(session, raw):
            let token = generation
            jobs[session] = Task { [weak self, repository] in
                do {
                    let cleaned = try await repository.cleanup(text: raw)
                    guard let self, self.generation == token, !Task.isCancelled else { return }
                    self.notes[session]?.cleanup = cleaned.outcome
                    self.send(.cleaned(session: session, text: cleaned.text))
                } catch {
                    guard let self, self.generation == token, !Task.isCancelled else { return }
                    self.notice = phoneError(error)
                    self.notes[session]?.cleanup = "error"
                    self.send(.cleaned(session: session, text: raw))
                }
            }
        case let .cancelTranscribe(session):
            repository.engine.cancel()
            jobs.removeValue(forKey: session)?.cancel()
        case let .stopLlm(session):
            jobs.removeValue(forKey: session)?.cancel()
        case let .insert(session, text, copy):
            guard let flow, !copy else { throw MobileError.Failed(detail: "This keyboard session cannot receive text.") }
            try store.complete(id: flow.id, text: text)
            result = text
            notice = "Return to your previous app. The text goes in at the cursor."
            send(.inserted(session: session))
            self.flow = nil
            observation = nil
            activity.end()
            endBackgroundTask()
            microphone.release()
            timer?.cancel()
            timer = nil
        case let .saveRow(session, status, text, _):
            jobs.removeValue(forKey: session)
            dropAudio(session)
            if let run = notes.removeValue(forKey: session), status == "done", let text {
                keep(run, text: text)
            }
        case let .notify(_, code):
            failedCode = code
            notice = dictationNotice(code)
        case let .maxDurationWarning(secondsLeft): notice = "\(secondsLeft) seconds left."
        case .edit: throw MobileError.Failed(detail: "Keyboard editing is unavailable.")
        case .cue, .updatePasteLast, .pasteLast: break
        }
    }

    /// A history failure never blocks the text the keyboard waits for.
    private func keep(_ run: RunNotes, text: String) {
        let entry = run.entry(text: text, model: repository.selectedModel)
        Task { [repository, weak self] in
            do {
                _ = try await Task.detached { try repository.history.save(entry: entry) }.value
                self?.kept?()
            } catch {
                Logger(subsystem: "com.mandalsuraj.hushpen", category: "History").error("History was not saved: \(error.localizedDescription, privacy: .public)")
            }
        }
    }

    private func dropAudio(_ session: UInt64) {
        guard let file = audio.removeValue(forKey: session) else { return }
        do { try FileManager.default.removeItem(at: file) }
        catch { notice = error.localizedDescription }
    }

    private func now() -> UInt64 { UInt64(ProcessInfo.processInfo.systemUptime * 1000) }
}

/// What one dictation tells the history: the engine's words and times, and how cleanup went.
private struct RunNotes {
    var raw = ""
    var language = ""
    var spokenMs: UInt64 = 0
    var cleanup = "off"
    var stoppedAt: Date?

    mutating func heard(_ transcript: Transcript) {
        raw = transcript.text
        language = transcript.language
        spokenMs = transcript.audioMs
    }

    func entry(text: String, model: String, now: Date = Date()) -> NewHistoryEntry {
        let tookMs = stoppedAt.map { max(0, now.timeIntervalSince($0) * 1000) } ?? 0
        return NewHistoryEntry(
            createdMs: Int64((stoppedAt ?? now).timeIntervalSince1970 * 1000), spokenMs: spokenMs, tookMs: UInt64(tookMs),
            rawText: raw, text: text, modelId: model, language: language, cleanup: cleanup
        )
    }
}

/// What the keyboard pill says when a dictation fails: plain words, never an error string.
func keyboardNotice(_ code: String) -> String {
    switch code {
    case "ENGINE_NO_SPEECH": "No speech heard"
    case "ENGINE_CRASHED": "Transcription failed. Open Hushpen to retry."
    case "CAPTURE_FAILED": "Microphone stopped. Try again."
    case "BACKGROUND_TIME": "Hushpen ran out of time. Try again."
    default: "Dictation failed. Try again."
    }
}

func dictationNotice(_ code: String) -> String {
    switch code {
    case "ENGINE_NO_SPEECH": "No speech was found. Try again."
    case "ENGINE_CRASHED": "Speech recognition stopped. Try again."
    case "ENGINE_NO_MODEL": "Choose a downloaded speech model."
    case "CAPTURE_FAILED": "Microphone recording failed. Try again."
    case "INSERT_NO_RECEIPT": "The keyboard could not receive the text. Start again."
    default: "Dictation failed. Start again from the keyboard."
    }
}
