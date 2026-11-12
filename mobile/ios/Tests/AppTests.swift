import AVFoundation
import Foundation
import XCTest
@testable import Hushpen

@MainActor
final class AppTests: XCTestCase {
    private var folder: URL!
    private var repository: PhoneRepository!

    override func setUp() async throws {
        folder = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        repository = try await Task.detached { [folder = folder!] in
            try PhoneRepository(data: folder, name: "Test iPhone")
        }.value
    }

    override func tearDown() async throws {
        repository.host.stop()
        repository.engine.shutdown()
        repository.localCleanup.shutdown()
        try FileManager.default.removeItem(at: folder)
        repository = nil
    }

    func testPhoneRepositoryUsesRustCatalogAndIosUnloadRule() async {
        let state = await repository.snapshot()
        XCTAssertTrue(state.models.contains { $0.id == "small-q5_1" && $0.kind == .speech })
        XCTAssertEqual(state.models.contains { $0.id == "large-v3-turbo-q5_0" }, ProcessInfo.processInfo.physicalMemory >= 6_000_000_000)
        XCTAssertEqual(state.models.filter { $0.kind == .systemCleanup }.count, 1)
        XCTAssertEqual(state.allowedRules, [.whileServerModeOpen])
        XCTAssertFalse(state.cleanupEnabled)
        XCTAssertFalse(state.systemAvailable)
        XCTAssertEqual(state.pairedClients, [])
    }

    func testFailedModelSelectionPreservesSavedChoice() async {
        do {
            try await repository.selectModel(id: "parakeet-v3-q4_k")
            XCTFail("A missing model must not become selected.")
        } catch { XCTAssertEqual(phoneError(error), "the model has not passed verification") }
        XCTAssertEqual(repository.selectedModel, "")
    }

    func testDisabledAndBackgroundSystemCleanupFailsBeforeGeneration() throws {
        let model = NativeSystemLlm(enabled: false)
        model.foreground = true
        XCTAssertFalse(model.available())
        XCTAssertThrowsError(try model.cleanup(text: "Hello", instructions: "Keep text"))
        model.enabled = true
        model.foreground = false
        XCTAssertFalse(model.available())
        XCTAssertThrowsError(try model.cleanup(text: "Hello", instructions: "Keep text"))
    }

    func testServerCannotStartOutsideForegroundApp() async {
        let model = ServerModel(repository: repository)
        model.setOpen(true)
        model.start()
        XCTAssertFalse(model.running)
        XCTAssertFalse(model.busy)
        XCTAssertNil(model.port)
        await model.refresh()
        XCTAssertEqual(model.allowedRules, [.whileServerModeOpen])
        model.setOpen(false)
    }

    func testModelsAndSettingsReadSharedState() async {
        let models = ModelsModel(repository: repository)
        await models.refresh()
        XCTAssertEqual(models.models, repository.catalog.models())
        XCTAssertEqual(models.selectedCleanupModel, repository.selectedCleanupModel)
        XCTAssertFalse(models.speechReady)
        let settings = SettingsModel(repository: repository)
        await settings.setCleanup(true)
        XCTAssertTrue(settings.cleanupEnabled)
        XCTAssertFalse(settings.systemAvailable)
        await settings.setCleanup(false)
        XCTAssertFalse(settings.cleanupEnabled)
    }

    func testAppDataIsExcludedFromBackups() throws {
        let values = try folder.resourceValues(forKeys: [.isExcludedFromBackupKey])
        XCTAssertEqual(values.isExcludedFromBackup, true)
    }

    func testFailedPreferenceWriteKeepsTheSavedSetting() async throws {
        let destination = folder.appendingPathComponent("config/phone.json", isDirectory: true)
        try FileManager.default.createDirectory(at: destination, withIntermediateDirectories: true)
        let settings = SettingsModel(repository: repository)
        await settings.setCleanup(true)
        XCTAssertFalse(settings.cleanupEnabled)
        XCTAssertFalse(settings.notice.isEmpty)
        XCTAssertFalse(repository.nativeSystem.enabled)
    }

    func testConcurrentCleanupUpdatesKeepRuntimeAndSavedSettingsTogether() async throws {
        let phone = try XCTUnwrap(repository)
        try await withThrowingTaskGroup(of: Void.self) { group in
            for index in 0..<20 {
                group.addTask { try phone.setCleanup(index.isMultiple(of: 2)) }
            }
            try await group.waitForAll()
        }
        let saved = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: folder.appendingPathComponent("config/phone.json"))) as? [String: Any])
        XCTAssertEqual(saved["cleanup"] as? Bool, phone.nativeSystem.enabled)
    }

    func testUnavailableSystemCleanupKeepsRustRuleText() async throws {
        let system = repository.system
        let (rule, unavailable) = try await Task.detached {
            let rule = try cleanText(raw: "  hello   world  ", system: nil)
            let unavailable = try cleanText(raw: "  hello   world  ", system: system)
            return (rule, unavailable)
        }.value
        XCTAssertEqual(unavailable.text, rule.text)
        XCTAssertEqual(unavailable.outcome, "unavailable")
        XCTAssertEqual(rule.outcome, "off")
    }

    func testOldPhoneSettingsChooseTheCoreCleanupDefault() async throws {
        let root = folder.appendingPathComponent("old-settings", isDirectory: true)
        try FileManager.default.createDirectory(at: root.appendingPathComponent("config"), withIntermediateDirectories: true)
        try Data("{\"speechModel\":\"\",\"cleanup\":false}".utf8).write(to: root.appendingPathComponent("config/phone.json"))
        let phone = try await Task.detached {
            try PhoneRepository(data: root, name: "Old settings", physicalMemory: 6_000_000_000)
        }.value
        defer { phone.host.stop(); phone.engine.shutdown(); phone.localCleanup.shutdown() }
        XCTAssertEqual(phone.selectedCleanupModel, "s1-mini")
        XCTAssertFalse(phone.nativeSystem.enabled)
    }

    func testSavedCleanupOutsideTheMemoryGateDoesNotBlockStartup() async throws {
        for memory: UInt64 in [0, 3_000_000_000, 6_000_000_000] {
            let root = folder.appendingPathComponent("memory-\(memory)", isDirectory: true)
            try FileManager.default.createDirectory(at: root.appendingPathComponent("config"), withIntermediateDirectories: true)
            try Data("{\"speechModel\":\"\",\"cleanup\":true,\"cleanupModel\":\"qwen3.5-2b\"}".utf8).write(to: root.appendingPathComponent("config/phone.json"))
            let phone = try await Task.detached {
                try PhoneRepository(data: root, name: "Memory gate", physicalMemory: memory)
            }.value
            defer { phone.host.stop(); phone.engine.shutdown(); phone.localCleanup.shutdown() }
            let expected = phone.catalog.models().first { $0.kind == .cleanup }?.id ?? ""
            XCTAssertEqual(phone.selectedCleanupModel, expected)
            let state = await phone.snapshot()
            XCTAssertEqual(state.selectedCleanupModel, expected)
        }
    }

    func testMissingCleanupSelectionPreservesSpeechAndCleanupChoices() async {
        let speech = repository.selectedModel
        let cleanup = repository.selectedCleanupModel
        do {
            try await repository.selectModel(id: "s1-mini")
            XCTFail("A cleanup model must pass verification before selection.")
        } catch { XCTAssertFalse(phoneError(error).isEmpty) }
        XCTAssertEqual(repository.selectedModel, speech)
        XCTAssertEqual(repository.selectedCleanupModel, cleanup)
    }

    func testDictationNoticesExplainCoreErrors() {
        XCTAssertEqual(dictationNotice("ENGINE_NO_SPEECH"), "No speech was found. Try again.")
        XCTAssertEqual(dictationNotice("ENGINE_CRASHED"), "Speech recognition stopped. Try again.")
        XCTAssertEqual(dictationNotice("UNKNOWN_CODE"), "Dictation failed. Start again from the keyboard.")
    }

    func testFreshDictationCancelsOnlyAbandonedRecordings() throws {
        let store = try FlowSessionStore(root: folder)
        let document = UUID()
        let abandoned = try store.begin(documentIdentifier: document, context: "cursor")
        _ = try store.request(for: abandoned.url)
        _ = try DictationModel(repository: repository, store: store)
        XCTAssertEqual(try store.current()?.state, .cancelled)

        let pending = try store.begin(documentIdentifier: document, context: "cursor")
        _ = try DictationModel(repository: repository, store: store)
        XCTAssertEqual(try store.request(for: pending.url).state, .recording)
        try store.complete(id: pending.id, text: "Ready")
        _ = try DictationModel(repository: repository, store: store)
        XCTAssertEqual(try store.consume(id: pending.id, documentIdentifier: document, context: "cursor"), "Ready")
    }

    func testDamagedSessionDoesNotBlockFreshDictation() throws {
        let store = try FlowSessionStore(root: folder)
        let file = folder.appendingPathComponent("Flow/session.json")
        let damaged = Data("damaged".utf8)
        try damaged.write(to: file)
        _ = try DictationModel(repository: repository, store: store)
        XCTAssertEqual(try Data(contentsOf: file), damaged)
        XCTAssertEqual(try store.begin(documentIdentifier: UUID(), context: "cursor").state, .pending)
    }

    func testOldRecorderCallbacksCannotFinishOrFailAnInactiveMicrophone() async throws {
        let microphone = Microphone()
        let oldRecorder = try AVAudioRecorder(url: folder.appendingPathComponent("old.wav"), settings: [
            AVFormatIDKey: kAudioFormatLinearPCM,
            AVSampleRateKey: 16_000,
            AVNumberOfChannelsKey: 1,
            AVLinearPCMBitDepthKey: 16,
        ])
        let finished = expectation(description: "An old recorder must not finish the session")
        let failed = expectation(description: "An old recorder must not fail the session")
        finished.isInverted = true
        failed.isInverted = true
        microphone.finished = { finished.fulfill() }
        microphone.failed = { _ in failed.fulfill() }
        microphone.audioRecorderDidFinishRecording(oldRecorder, successfully: true)
        microphone.audioRecorderEncodeErrorDidOccur(oldRecorder, error: nil)
        await fulfillment(of: [finished, failed], timeout: 0.1)
    }
}
