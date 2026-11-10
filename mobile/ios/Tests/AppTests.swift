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
        try FileManager.default.removeItem(at: folder)
        repository = nil
    }

    func testPhoneRepositoryUsesRustCatalogAndIosUnloadRule() async {
        let state = await repository.snapshot()
        XCTAssertEqual(state.models.filter { $0.kind == .speech }.count, 2)
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
        XCTAssertEqual(models.models.count, 3)
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
