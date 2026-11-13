import AVFoundation
import Observation
import UIKit

@Observable @MainActor
final class OnboardingModel {
    private(set) var microphone = "Not requested"
    private(set) var keyboardUsed = false
    private(set) var keyboardAdded = false
    /// `nil` until the keyboard has run lately and could say.
    private(set) var fullAccess: Bool?
    private(set) var keyboardStatus = "Open keyboard to check"

    var microphoneGranted: Bool { microphone == "Allowed" }
    var setupDone: Bool { microphoneGranted && keyboardUsed }

    func refresh() {
        switch AVAudioApplication.shared.recordPermission {
        case .granted: microphone = "Allowed"
        case .denied: microphone = "Denied"
        case .undetermined: microphone = "Not requested"
        @unknown default: microphone = "Unavailable"
        }
        let status = KeyboardStatus(
            modes: UITextInputMode.activeInputModes.map(\.description),
            heartbeat: KeyboardHeartbeat.read(root: KeyboardStatus.groupRoot),
            legacySeenAt: UserDefaults(suiteName: FlowSessionStore.groupIdentifier)?.object(forKey: "keyboardLastSeen") as? Date
        )
        keyboardUsed = status.added && status.fullAccess != false
        keyboardAdded = status.added
        fullAccess = status.fullAccess
        keyboardStatus = status.text
    }

    func requestMicrophone() async {
        _ = await AVAudioApplication.requestRecordPermission()
        refresh()
    }

    func openSettings() {
        guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
        UIApplication.shared.open(url)
    }
}

@Observable @MainActor
final class ModelsModel {
    private let repository: PhoneRepository
    private(set) var models: [Model] = []
    private(set) var details: [String: ModelDetails] = [:]
    private(set) var selectedModel = ""
    private(set) var selectedCleanupModel = ""
    private(set) var downloading: String?
    private(set) var progress: UInt64 = 0
    private(set) var total: UInt64 = 0
    private(set) var verifying = false
    private(set) var busy = false
    var notice = ""

    init(repository: PhoneRepository) { self.repository = repository }

    var speechReady: Bool { models.contains { $0.id == selectedModel && $0.status == .ready } }

    func refresh() async {
        let value = await repository.snapshot()
        models = value.models
        details = value.details
        selectedModel = value.selectedModel
        selectedCleanupModel = value.selectedCleanupModel
    }

    func download(_ id: String) async {
        guard downloading == nil else { return }
        downloading = id
        notice = ""
        progress = 0
        total = 0
        verifying = false
        defer { downloading = nil; verifying = false }
        do {
            let callback = ModelProgress { [weak self] done, total, verifying in
                Task { @MainActor in
                    guard let self, self.downloading == id else { return }
                    self.progress = done
                    self.total = total
                    self.verifying = verifying
                }
            }
            try await repository.download(id: id, progress: callback)
        } catch { notice = phoneError(error) }
        await refresh()
    }

    func cancelDownload() { repository.catalog.cancelDownload() }

    func select(_ id: String) async {
        guard !busy else { return }
        busy = true
        defer { busy = false }
        do { try await repository.selectModel(id: id) }
        catch { notice = phoneError(error) }
        await refresh()
    }

    func verify(_ id: String) async {
        guard !busy else { return }
        busy = true
        defer { busy = false }
        do {
            if try await repository.verify(id: id) == false { notice = "Model verification failed. Download it again." }
        } catch { notice = phoneError(error) }
        await refresh()
    }
}

/// Counts each time a desktop job begins; the core reports a job starting and ending.
private final class ServedJobs: JobActivity, Sendable {
    private let began: @MainActor @Sendable () -> Void
    init(began: @escaping @MainActor @Sendable () -> Void) { self.began = began }
    func changed(active: Bool) {
        if active { Task { @MainActor in began() } }
    }
}

private final class ModelProgress: DownloadProgress, Sendable {
    private let changed: @Sendable (UInt64, UInt64, Bool) -> Void
    init(changed: @escaping @Sendable (UInt64, UInt64, Bool) -> Void) { self.changed = changed }
    func bytes(done: UInt64, total: UInt64) { changed(done, total, false) }
    func verifying() { changed(0, 0, true) }
}

@Observable @MainActor
final class SettingsModel {
    private let repository: PhoneRepository
    private(set) var cleanupEnabled = false
    private(set) var systemAvailable = false
    private(set) var busy = false
    var notice = ""

    init(repository: PhoneRepository) { self.repository = repository }

    func refresh() async {
        let value = await repository.snapshot()
        cleanupEnabled = value.cleanupEnabled
        systemAvailable = value.systemAvailable
    }

    func setCleanup(_ enabled: Bool) async {
        guard !busy else { return }
        busy = true
        defer { busy = false }
        do {
            try await Task.detached { [repository] in try repository.setCleanup(enabled) }.value
            notice = ""
        }
        catch { notice = phoneError(error) }
        await refresh()
    }
}

@Observable @MainActor
final class ServerModel {
    private let repository: PhoneRepository
    private(set) var running = false
    private(set) var port: UInt16?
    private(set) var busy = false
    private(set) var active = false
    private(set) var open = false
    private(set) var allowedRules: [HostUnloadRule] = []
    private(set) var selectedModel = ""
    /// Jobs desktops started while Server mode ran, since it last started.
    private(set) var served = 0
    var notice = ""
    private var generation = UUID()
    private var priorBrightness: CGFloat?
    private var priorIdleTimer: Bool?
    private weak var screen: UIScreen?
    private weak var dimmedScreen: UIScreen?
    private var operation: Task<Void, Never>?

    init(repository: PhoneRepository) {
        self.repository = repository
        repository.host.setJobActivity(activity: ServedJobs { [weak self] in self?.served += 1 })
    }

    func refresh() async {
        let value = await repository.snapshot()
        allowedRules = value.allowedRules
        selectedModel = value.selectedModel
    }

    func setActive(_ value: Bool, screen: UIScreen?) {
        active = value
        if let screen { self.screen = screen }
        repository.nativeSystem.foreground = value
        if !value {
            let interrupted = running || busy
            stop()
            if interrupted { notice = "Server mode stopped. Keep Hushpen in front and start it again." }
        }
    }

    func setOpen(_ value: Bool) {
        open = value
        if !value { stop() }
    }

    func start() {
        guard active && open && !running && !busy else { return }
        let token = UUID()
        generation = token
        busy = true
        notice = ""
        served = 0
        let previous = operation
        operation = Task { [weak self, repository] in
            await previous?.value
            do {
                let port = try await Task.detached {
                    repository.host.setForeground(foreground: true)
                    try repository.host.validateUnloadRule(rule: .whileServerModeOpen)
                    return try repository.host.start(listen: "0.0.0.0:7342", rule: .whileServerModeOpen)
                }.value
                guard let self, self.generation == token, self.active, self.open else {
                    await Task.detached { repository.host.setForeground(foreground: false) }.value
                    return
                }
                self.port = port
                self.running = true
                self.busy = false
                self.keepAwake()
            } catch {
                guard let self, self.generation == token else { return }
                self.busy = false
                self.notice = phoneError(error)
                self.restoreScreen()
            }
        }
    }

    func stop() {
        generation = UUID()
        running = false
        busy = false
        port = nil
        restoreScreen()
        let previous = operation
        operation = Task { [repository] in
            await previous?.value
            await Task.detached { repository.host.setForeground(foreground: false) }.value
        }
    }

    private func keepAwake() {
        priorIdleTimer = UIApplication.shared.isIdleTimerDisabled
        UIApplication.shared.isIdleTimerDisabled = true
        if let screen {
            dimmedScreen = screen
            priorBrightness = screen.brightness
            screen.brightness = min(screen.brightness, 0.1)
        }
    }

    private func restoreScreen() {
        if let priorIdleTimer { UIApplication.shared.isIdleTimerDisabled = priorIdleTimer }
        if let priorBrightness { dimmedScreen?.brightness = priorBrightness }
        dimmedScreen = nil
        priorBrightness = nil
        priorIdleTimer = nil
    }
}

@Observable @MainActor
final class DevicesModel {
    private let repository: PhoneRepository
    let server: ServerModel
    private(set) var clients: [PairedDesktop] = []
    private(set) var busy = false
    /// The number both screens show while a computer pairs through Nearby; `nil` otherwise.
    private(set) var comparing: String?
    var notice = ""
    var scannerOpen = false
    private var watchers = 0
    private var watching: Task<Void, Never>?

    init(repository: PhoneRepository, server: ServerModel) {
        self.repository = repository
        self.server = server
        #if DEBUG
        // Screenshots of the comparison need a computer; this shows the same state without one.
        if ProcessInfo.processInfo.environment["HUSHPEN_TEST_PAIRING"] == "compare" { comparing = "482913" }
        #endif
    }

    func refresh() async { clients = await repository.snapshot().pairedClients }

    /// The Nearby window stays open while a screen that can pair is in front, and closes with it.
    func watch() {
        watchers += 1
        if watching == nil { watching = Task { await listen() } }
    }

    func unwatch() {
        watchers = max(0, watchers - 1)
        guard watchers == 0 else { return }
        watching?.cancel()
        watching = nil
        comparing = nil
        Task { await repository.closePairing() }
    }

    private func listen() async {
        while !Task.isCancelled {
            if server.running {
                let state = await repository.pairingState()
                if Task.isCancelled { return }
                comparing = if case let .comparing(number) = state { number } else { nil }
                switch state {
                case .closed: await repository.openPairing()
                case .paired:
                    notice = "Computer paired."
                    await refresh()
                    await repository.openPairing()
                case let .failed(message):
                    notice = message
                    await repository.openPairing()
                case .listening, .comparing: break
                }
            } else {
                comparing = nil
            }
            try? await Task.sleep(for: .milliseconds(400))
        }
    }

    func answer(_ approve: Bool) {
        comparing = nil
        Task { await repository.answerPairing(approve) }
    }

    /// Scans a QR, or types the 8-digit code (with an address, or empty to look nearby).
    func pair(payload: String? = nil, code: String = "", address: String = "") async -> Bool {
        guard !busy else { return false }
        busy = true
        defer { busy = false }
        do {
            let name = if let payload { try await repository.pair(payload: payload) } else { try await repository.pair(code: code, address: address) }
            notice = name.isEmpty ? "Computer paired." : "\(name) paired."
            await refresh()
            return true
        } catch {
            notice = phoneError(error)
            return false
        }
    }

    func forget(_ id: String) async {
        guard !busy else { return }
        busy = true
        defer { busy = false }
        do {
            try await repository.forget(id: id)
            server.stop()
        } catch { notice = phoneError(error) }
        await refresh()
    }
}
