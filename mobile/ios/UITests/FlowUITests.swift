import Darwin
import XCTest

@MainActor
final class FlowUITests: XCTestCase {
    private let phone = XCUIApplication(bundleIdentifier: "com.mandalsuraj.hushpen")
    private let editor = XCUIApplication(bundleIdentifier: "com.mandalsuraj.hushpen.KeyboardTestHost")

    override func setUp() async throws {
        try await super.setUp()
        continueAfterFailure = false
        phone.launchEnvironment["HUSHPEN_TEST_WAV"] = "documents"
        phone.launchEnvironment["HUSHPEN_TEST_MODEL"] = "ggml-base-q5_1.bin"
        phone.launch()
        XCTAssertTrue(phone.navigationBars["Hushpen"].waitForExistence(timeout: 30))
        enableKeyboard()
        phone.activate()
    }

    override func tearDown() async throws {
        phone.terminate()
        editor.terminate()
        try await super.tearDown()
    }

    func testKeyboardFlowTranscribesWavAndConsumesItOnce() {
        openEditor()
        element("keyboard.key.h", in: editor).tap()
        element("keyboard.key.i", in: editor).tap()
        element("keyboard.space", in: editor).tap()
        XCTAssertEqual(editor.textFields["editor.first"].value as? String, "hi ")

        phone.terminate()
        startFromKeyboard()
        element("dictation.cancel", in: phone).tap()
        element("dictation.recover", in: phone).tap()
        waitForText("done", id: "dictation.state", in: phone, timeout: 180)
        editor.activate()
        let field = editor.textFields["editor.first"]
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        let insert = element("keyboard.insert", in: editor)
        XCTAssertTrue(insert.waitForExistence(timeout: 10))
        XCTAssertEqual(field.value as? String, "hi ")
        insert.tap()
        let inserted = NSPredicate(format: "value CONTAINS[c] %@", "quick brown fox")
        expectation(for: inserted, evaluatedWith: field)
        waitForExpectations(timeout: 15)
        let once = field.value as? String
        attach(editor, name: "Returned transcript")

        XCTAssertFalse(element("keyboard.insert", in: editor).exists)
        XCTAssertEqual(field.value as? String, once)
        element("keyboard.undo", in: editor).tap()
        XCTAssertEqual(field.value as? String, "hi ")
        phone.activate()
        editor.activate()
        useKeyboard()
        XCTAssertEqual(field.value as? String, "hi ")
        let undo = element("keyboard.undo", in: editor)
        XCTAssertTrue(undo.waitForExistence(timeout: 10))
        XCTAssertFalse(undo.isEnabled)
    }

    func testSetupRefreshesWhenReturningFromTheKeyboard() {
        XCTAssertTrue(element("onboarding.keyboard", in: phone).waitForExistence(timeout: 10))
        XCTAssertFalse(phone.staticTexts["Not added yet"].exists)
        let added = phone.staticTexts["Added"].exists
        let observed = phone.staticTexts["Full Access last seen"].exists
        let unknown = phone.staticTexts["Open keyboard to check"].exists
        XCTAssertTrue(added || observed || unknown)
        print("PHONE_KEYBOARD_AFTER_SETTINGS added=\(added) full_access_observed=\(observed) unknown=\(unknown)")
        openEditor()
        phone.activate()
        XCTAssertTrue(phone.staticTexts["Full Access last seen"].waitForExistence(timeout: 10))
        element("home.settings", in: phone).tap()
        XCTAssertTrue(phone.staticTexts["Full Access last seen"].waitForExistence(timeout: 10))
    }

    func testCancelDoesNotInsertText() {
        openEditor()
        startFromKeyboard()
        element("dictation.cancel", in: phone).tap()
        phone.terminate()
        editor.activate()
        XCTAssertEqual(editor.textFields["editor.first"].value as? String, "First field")
        XCTAssertTrue(element("keyboard.mic", in: editor).waitForExistence(timeout: 10))
        attach(editor, name: "Cancelled session")
    }

    func testKeyboardStopsBackgroundFlowWithoutOpeningTheApp() {
        openEditor()
        startFromKeyboard()
        editor.activate()
        useKeyboard()
        let microphone = element("keyboard.mic", in: editor)
        XCTAssertTrue(microphone.waitForExistence(timeout: 10))
        microphone.tap()
        let insert = element("keyboard.insert", in: editor)
        XCTAssertTrue(insert.waitForExistence(timeout: 180))
        insert.tap()
        let field = editor.textFields["editor.first"]
        let inserted = NSPredicate(format: "value CONTAINS[c] %@", "quick brown fox")
        expectation(for: inserted, evaluatedWith: field)
        waitForExpectations(timeout: 15)
        XCTAssertFalse(insert.exists)
    }

    func testChangingFieldsWithdrawsUndoAndPreventsReplay() {
        openEditor()
        startFromKeyboard()
        element("dictation.finish", in: phone).tap()
        waitForText("done", id: "dictation.state", in: phone, timeout: 180)
        editor.activate()
        let first = editor.textFields["editor.first"]
        XCTAssertTrue(first.waitForExistence(timeout: 10))
        attach(editor, name: "Keyboard returned")
        let insert = element("keyboard.insert", in: editor)
        XCTAssertTrue(insert.waitForExistence(timeout: 10))
        XCTAssertEqual(first.value as? String, "First field")
        editor.textFields["editor.second"].tap()
        useKeyboard()
        XCTAssertTrue(element("keyboard.mic", in: editor).waitForExistence(timeout: 10))
        XCTAssertFalse(insert.exists)
        XCTAssertEqual(first.value as? String, "First field")
        XCTAssertEqual(editor.textFields["editor.second"].value as? String, "Second field")
        editor.textFields["editor.first"].tap()
        useKeyboard()
        startFromKeyboard()
        element("dictation.finish", in: phone).tap()
        waitForText("done", id: "dictation.state", in: phone, timeout: 180)
        editor.activate()
        XCTAssertTrue(insert.waitForExistence(timeout: 10))
        insert.tap()
        let inserted = NSPredicate(format: "value CONTAINS[c] %@", "quick brown fox")
        expectation(for: inserted, evaluatedWith: first)
        waitForExpectations(timeout: 15)
        editor.textFields["editor.second"].tap()
        useKeyboard()
        XCTAssertEqual(editor.textFields["editor.second"].value as? String, "Second field")
        attach(editor, name: "Changed field")
        let undo = element("keyboard.undo", in: editor)
        XCTAssertTrue(undo.waitForExistence(timeout: 10))
        XCTAssertFalse(undo.isEnabled)
    }

    func testServerStopsWhenTheAppLeavesTheForeground() {
        XCTAssertTrue(phone.wait(for: .runningForeground, timeout: 10))
        phone.switches["server.toggle"].switches.firstMatch.tap()
        waitForText("Serving", id: "server.state", in: phone, timeout: 20)
        XCTAssertEqual(element("server.port", in: phone).label, "7342")
        XCTAssertTrue(serverAcceptsConnections())
        attach(phone, name: "Server running")
        editor.launch()
        XCTAssertTrue(editor.wait(for: .runningForeground, timeout: 10))
        XCTAssertTrue(phone.wait(for: .runningBackground, timeout: 10))
        let closed = NSPredicate { _, _ in !self.serverAcceptsConnections() }
        expectation(for: closed, evaluatedWith: nil)
        waitForExpectations(timeout: 10)
        phone.activate()
        XCTAssertTrue(phone.wait(for: .runningForeground, timeout: 10))
        waitForText("Stopped", id: "server.state", in: phone, timeout: 10)
        XCTAssertEqual(element("server.port", in: phone).label, "None")
        XCTAssertFalse(serverAcceptsConnections())
        attach(phone, name: "Server stopped on background")
    }

    nonisolated private func serverAcceptsConnections() -> Bool {
        let socket = Darwin.socket(AF_INET, SOCK_STREAM, 0)
        guard socket >= 0 else { XCTFail("The server probe could not open a socket."); return false }
        defer { Darwin.close(socket) }
        var address = sockaddr_in()
        address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        address.sin_family = sa_family_t(AF_INET)
        address.sin_port = UInt16(7342).bigEndian
        address.sin_addr = in_addr(s_addr: inet_addr("127.0.0.1"))
        return withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.connect(socket, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) == 0
            }
        }
    }

    private func openEditor() {
        editor.launch()
        let field = editor.textFields["editor.first"]
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        field.tap()
        useKeyboard()
        if element("keyboard.insert", in: editor).exists {
            element("keyboard.cancel", in: editor).tap()
        }
        XCTAssertTrue(element("keyboard.mic", in: editor).waitForExistence(timeout: 15))
        attach(editor, name: "Keyboard ready")
    }

    private func useKeyboard() {
        if !element("keyboard.status", in: editor).waitForExistence(timeout: 2) {
            let globe = editor.buttons.matching(NSPredicate(format: "(label == %@ OR label BEGINSWITH[c] %@) AND identifier != %@", "Emoji", "Next keyboard", "keyboard.next")).firstMatch
            XCTAssertTrue(globe.waitForExistence(timeout: 5))
            globe.press(forDuration: 1)
            attach(editor, name: "Keyboard picker")
            let keyboard = editor.descendants(matching: .any).matching(NSPredicate(format: "label == %@", "Hushpen")).firstMatch
            XCTAssertTrue(keyboard.waitForExistence(timeout: 5))
            keyboard.tap()
        }
        XCTAssertTrue(element("keyboard.status", in: editor).waitForExistence(timeout: 15))
    }

    private func enableKeyboard() {
        let settings = XCUIApplication(bundleIdentifier: "com.apple.Preferences")
        settings.launch()
        let general = settings.staticTexts["General"]
        XCTAssertTrue(general.waitForExistence(timeout: 10))
        general.tap()
        let keyboard = settings.staticTexts["Keyboard"]
        XCTAssertTrue(keyboard.waitForExistence(timeout: 10))
        keyboard.tap()
        let keyboards = settings.cells.containing(.staticText, identifier: "Keyboards").firstMatch
        XCTAssertTrue(keyboards.waitForExistence(timeout: 10))
        keyboards.tap()
        let hushpen = settings.staticTexts["Hushpen"].firstMatch
        if !hushpen.waitForExistence(timeout: 2) {
            let add = settings.buttons["AddNewKeyboard"]
            XCTAssertTrue(add.waitForExistence(timeout: 5))
            add.tap()
            XCTAssertTrue(hushpen.waitForExistence(timeout: 5))
            hushpen.tap()
        }
        XCTAssertTrue(hushpen.waitForExistence(timeout: 5))
        hushpen.tap()
        let access = settings.switches["Allow Full Access"].switches.firstMatch
        XCTAssertTrue(access.waitForExistence(timeout: 5))
        if access.value as? String == "0" {
            access.tap()
            let allow = settings.alerts.buttons["Allow"]
            XCTAssertTrue(allow.waitForExistence(timeout: 5))
            allow.tap()
            XCTAssertEqual(access.value as? String, "1")
        }
        settings.terminate()
    }

    private func startFromKeyboard() {
        let microphone = element("keyboard.mic", in: editor)
        XCTAssertTrue(microphone.waitForExistence(timeout: 10))
        microphone.tap()
        waitForText("listening", id: "dictation.state", in: phone, timeout: 30)
        attach(phone, name: "App recording")
    }

    private func element(_ id: String, in app: XCUIApplication) -> XCUIElement {
        app.descendants(matching: .any).matching(identifier: id).firstMatch
    }

    private func waitForText(_ text: String, id: String, in app: XCUIApplication, timeout: TimeInterval) {
        let value = element(id, in: app)
        XCTAssertTrue(value.waitForExistence(timeout: timeout))
        let predicate = NSPredicate(format: "label CONTAINS[c] %@", text)
        expectation(for: predicate, evaluatedWith: value)
        waitForExpectations(timeout: timeout)
    }

    private func attach(_ app: XCUIApplication, name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .deleteOnSuccess
        add(attachment)
    }
}
