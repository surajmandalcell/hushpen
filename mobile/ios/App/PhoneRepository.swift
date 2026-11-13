import Foundation
import OSLog
import UIKit

struct PhoneSnapshot: Sendable {
    let models: [Model]
    let selectedModel: String
    let selectedCleanupModel: String
    let cleanupEnabled: Bool
    let systemAvailable: Bool
    let pairedClients: [String]
    let allowedRules: [HostUnloadRule]
    let details: [String: ModelDetails]
}

private struct PhoneSettings: Codable {
    var speechModel = ""
    var cleanup = false
    var cleanupModel: String?
}

final class PhoneRepository: @unchecked Sendable {
    let catalog: ModelRepository
    let engine: Engine
    let host: Host
    let system: SystemCleanup
    let localCleanup: LocalCleanup
    let nativeSystem: NativeSystemLlm
    let history: History
    let data: URL
    private let settingsURL: URL
    private var settings: PhoneSettings
    private let lock = NSLock()
    private let speech = NSLock()

    init(data: URL, name: String, physicalMemory: UInt64 = ProcessInfo.processInfo.physicalMemory) throws {
        self.data = data
        settingsURL = data.appendingPathComponent("config/phone.json")
        if FileManager.default.fileExists(atPath: settingsURL.path) {
            settings = try JSONDecoder().decode(PhoneSettings.self, from: Data(contentsOf: settingsURL))
        } else {
            settings = PhoneSettings()
        }
        try FileManager.default.createDirectory(at: data, withIntermediateDirectories: true)
        var privateData = data
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        try privateData.setResourceValues(values)
        catalog = try ModelRepository.withMemory(data: data.path, physicalMemoryBytes: physicalMemory)
        let cleanupModels = catalog.models().filter { $0.kind == .cleanup }
        let savedCleanup = settings.cleanupModel
        settings.cleanupModel = cleanupModels.contains { $0.id == savedCleanup }
            ? savedCleanup : (cleanupModels.first(where: \.recommended) ?? cleanupModels.first)?.id
        engine = try Engine(dataDir: data.path)
        localCleanup = try LocalCleanup(dataDir: data.path, physicalMemoryBytes: physicalMemory)
        localCleanup.setMemoryBudget(bytes: physicalMemory)
        if let id = settings.cleanupModel { try localCleanup.setModel(modelId: id) }
        nativeSystem = NativeSystemLlm(enabled: settings.cleanup)
        system = SystemCleanup(system: nativeSystem)
        system.setLocal(local: settings.cleanup ? localCleanup : nil)
        host = try Host(engine: engine, name: name, system: system)
        host.setForeground(foreground: false)
        history = try History(dataDir: data.path)
    }

    static func open() async throws -> PhoneRepository {
        let name = await MainActor.run { String(UIDevice.current.name.prefix(32)) }
        return try await Task.detached {
            let base = try FileManager.default.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
            let sessions = FileManager.default.temporaryDirectory.appendingPathComponent("dictation", isDirectory: true)
            if FileManager.default.fileExists(atPath: sessions.path) { try FileManager.default.removeItem(at: sessions) }
            let repository = try PhoneRepository(data: base.appendingPathComponent("hushpen", isDirectory: true), name: name)
            #if DEBUG
            if let source = ProcessInfo.processInfo.environment["HUSHPEN_TEST_MODEL"] {
                let target = repository.data.appendingPathComponent("models/whisper/ggml-base-q5_1.bin")
                if !FileManager.default.fileExists(atPath: target.path) {
                    let fixture = source.hasPrefix("/") ? URL(fileURLWithPath: source) : FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0].appendingPathComponent(source)
                    try FileManager.default.copyItem(at: fixture, to: target)
                }
                guard try repository.catalog.verify(id: "base-q5_1") else {
                    throw MobileError.Failed(detail: "The test model failed verification.")
                }
                try repository.select(id: "base-q5_1")
            }
            if ProcessInfo.processInfo.environment["HUSHPEN_TEST_HISTORY"] == "demo" { try repository.seedDemoHistory() }
            #endif
            return repository
        }.value
    }

    var selectedModel: String {
        lock.withLock { settings.speechModel }
    }

    var selectedCleanupModel: String {
        lock.withLock { settings.cleanupModel ?? "" }
    }

    func snapshot() async -> PhoneSnapshot {
        await Task.detached { [self] in
            let models = catalog.models()
            return PhoneSnapshot(
                models: models, selectedModel: selectedModel,
                selectedCleanupModel: selectedCleanupModel,
                cleanupEnabled: nativeSystem.enabled, systemAvailable: system.available(),
                pairedClients: host.pairedClients(), allowedRules: host.allowedUnloadRules(),
                details: Dictionary(uniqueKeysWithValues: models.compactMap { model in
                    catalog.details(id: model.id).map { (model.id, $0) }
                })
            )
        }.value
    }

    func download(id: String, progress: DownloadProgress?) async throws {
        try await Task.detached { [self] in
            if try catalog.download(id: id, progress: progress) {
                if catalog.models().contains(where: { $0.id == id && $0.kind == .speech }) {
                    let next = try catalog.selectionAfterDownload(current: selectedModel, downloaded: id)
                    if next != selectedModel { try select(id: next) }
                }
            }
        }.value
    }

    func verify(id: String) async throws -> Bool {
        try await Task.detached { [self] in try catalog.verify(id: id) }.value
    }

    func selectModel(id: String) async throws {
        try await Task.detached { [self] in try select(id: id) }.value
    }

    private func select(id: String) throws {
        guard let model = catalog.models().first(where: { $0.id == id }) else {
            throw MobileError.InvalidInput(detail: "This model is not available for this phone.")
        }
        _ = try catalog.modelPath(id: id)
        if model.kind == .cleanup {
            try lock.withLock {
                let previous = settings.cleanupModel
                try localCleanup.setModel(modelId: id)
                do {
                    var next = settings
                    next.cleanupModel = id
                    try save(next)
                } catch {
                    if let previous { try localCleanup.setModel(modelId: previous) }
                    throw error
                }
            }
            return
        }
        guard model.kind == .speech else {
            throw MobileError.InvalidInput(detail: "Choose a downloaded speech or cleanup model.")
        }
        try speech.withLock {
            try engine.unload()
            try lock.withLock {
                var next = settings
                next.speechModel = id
                try save(next)
            }
        }
    }

    func transcribe(wav: URL) async throws -> Transcript {
        try Task.checkCancellation()
        let task = Task.detached { [self] in
            try speech.withLock {
                defer {
                    do { try host.releaseLocalModel() }
                    catch { Logger(subsystem: "com.mandalsuraj.hushpen", category: "Dictation").error("Speech model could not unload: \(error.localizedDescription, privacy: .public)") }
                }
                try Task.checkCancellation()
                try engine.load(modelId: selectedModel)
                try Task.checkCancellation()
                return try engine.transcribe(wavPath: wav.path, language: nil)
            }
        }
        return try await withTaskCancellationHandler {
            try await task.value
        } onCancel: { [engine] in
            task.cancel()
            engine.cancel()
        }
    }

    func cleanup(text: String) async throws -> CleanupResult {
        try await Task.detached { [self] in
            try cleanText(raw: text, system: nativeSystem.enabled ? system : nil)
        }.value
    }

    func setCleanup(_ enabled: Bool) throws {
        try lock.withLock {
            var next = settings
            next.cleanup = enabled
            try save(next)
            nativeSystem.enabled = enabled
            system.setLocal(local: enabled ? localCleanup : nil)
            if !enabled { localCleanup.unload() }
        }
    }

    private func save(_ next: PhoneSettings) throws {
        try JSONEncoder().encode(next).write(to: settingsURL, options: [.atomic, .completeFileProtection])
        try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: settingsURL.path)
        settings = next
    }

    func pair(uri: String) async throws {
        try await Task.detached { [host] in try host.pair(uri: uri) }.value
    }

    func forget(id: String) async throws {
        try await Task.detached { [host] in try host.forget(publicKey: id) }.value
    }
}

func phoneError(_ error: Error) -> String {
    switch error {
    case let MobileError.InvalidInput(detail), let MobileError.Failed(detail): detail
    default: error.localizedDescription
    }
}

#if DEBUG
extension PhoneRepository {
    /// Screenshot and test data: a streak of days with a few dictations each. Runs once on an empty history.
    func seedDemoHistory(now: Date = Date()) throws {
        guard try history.page(query: "", after: nil, limit: 1).entries.isEmpty else { return }
        let texts = [
            "Running about ten minutes late. Start without me and keep me a seat by the window.",
            "Remind me to call the dentist on Monday morning and ask about the invoice.",
            "Hi Priya, thanks for the notes. I read them on the train and the second section needs a clearer example before we send it to the team. Can you add the numbers from last week and a short line on what changed? I would also move the risks to the top, because that is what Ravi will read first. Let me know when it is ready and I will look tonight.",
            "Pick up oat milk, lemons and the parcel from the locker.",
            "Meeting notes. We agreed to ship the beta on the third of June. Ravi owns the release checklist.",
        ]
        let calendar = Calendar.current
        for day in 0..<11 {
            for slot in 0..<(day == 0 ? 3 : 2) {
                let text = texts[(day + slot * 2) % texts.count]
                let at = (calendar.date(byAdding: .day, value: -day, to: now) ?? now).addingTimeInterval(-Double(slot) * 5_400 - 600)
                let words = text.split(separator: " ").count
                _ = try history.save(entry: NewHistoryEntry(
                    createdMs: Int64(at.timeIntervalSince1970 * 1000), spokenMs: UInt64(words) * 400 + 1_000,
                    tookMs: UInt64(900 + words * 40), rawText: text.lowercased(), text: text,
                    modelId: "parakeet-v3-q4_k", language: "en", cleanup: "used"
                ))
            }
        }
    }
}
#endif
