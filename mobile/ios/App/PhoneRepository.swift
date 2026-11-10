import Foundation
import UIKit

struct PhoneSnapshot: Sendable {
    let models: [Model]
    let selectedModel: String
    let cleanupEnabled: Bool
    let systemAvailable: Bool
    let pairedClients: [String]
    let allowedRules: [HostUnloadRule]
}

private struct PhoneSettings: Codable {
    var speechModel = ""
    var cleanup = false
}

final class PhoneRepository: @unchecked Sendable {
    let catalog: ModelRepository
    let engine: Engine
    let host: Host
    let system: SystemCleanup
    let nativeSystem: NativeSystemLlm
    let data: URL
    private let settingsURL: URL
    private var settings: PhoneSettings
    private let lock = NSLock()
    private let speech = NSLock()

    init(data: URL, name: String) throws {
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
        catalog = try ModelRepository(data: data.path)
        engine = try Engine(dataDir: data.path)
        nativeSystem = NativeSystemLlm(enabled: settings.cleanup)
        system = SystemCleanup(system: nativeSystem)
        host = try Host(engine: engine, name: name, system: system)
        host.setForeground(foreground: false)
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
            #endif
            return repository
        }.value
    }

    var selectedModel: String {
        lock.withLock { settings.speechModel }
    }

    func snapshot() async -> PhoneSnapshot {
        await Task.detached { [self] in
            PhoneSnapshot(
                models: catalog.models(), selectedModel: selectedModel,
                cleanupEnabled: nativeSystem.enabled, systemAvailable: system.available(),
                pairedClients: host.pairedClients(), allowedRules: host.allowedUnloadRules()
            )
        }.value
    }

    func download(id: String, progress: DownloadProgress?) async throws {
        try await Task.detached { [self] in
            if try catalog.download(id: id, progress: progress) {
                let next = try catalog.selectionAfterDownload(current: selectedModel, downloaded: id)
                if next != selectedModel { try select(id: next) }
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
        _ = try catalog.modelPath(id: id)
        try speech.withLock {
            try engine.unload()
            try lock.withLock {
                var next = settings
                next.speechModel = id
                try save(next)
            }
        }
    }

    func transcribe(wav: URL) async throws -> String {
        try Task.checkCancellation()
        let task = Task.detached { [self] in
            try speech.withLock {
                try Task.checkCancellation()
                try engine.load(modelId: selectedModel)
                try Task.checkCancellation()
                return try engine.transcribe(wavPath: wav.path, language: nil).text
            }
        }
        return try await withTaskCancellationHandler {
            try await task.value
        } onCancel: { [engine] in
            task.cancel()
            engine.cancel()
        }
    }

    func cleanup(text: String) async throws -> String {
        try await Task.detached { [self] in
            try cleanText(raw: text, system: nativeSystem.enabled ? system : nil).text
        }.value
    }

    func setCleanup(_ enabled: Bool) throws {
        try lock.withLock {
            var next = settings
            next.cleanup = enabled
            try save(next)
        }
        nativeSystem.enabled = enabled
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
