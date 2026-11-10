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
        XCTAssertTrue(phone.tabBars.firstMatch.waitForExistence(timeout: 30))
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
        let inserted = NSPredicate(format: "value CONTAINS[c] %@", "quick brown fox")
        expectation(for: inserted, evaluatedWith: field)
        waitForExpectations(timeout: 15)
        let once = field.value as? String
        attach(editor, name: "Returned transcript")

        phone.activate()
        editor.activate()
        XCTAssertEqual(field.value as? String, once)
        element("keyboard.undo", in: editor).tap()
        XCTAssertEqual(field.value as? String, "hi ")
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

    func testChangingFieldsWithdrawsUndoAndPreventsReplay() {
        openEditor()
        startFromKeyboard()
        element("dictation.finish", in: phone).tap()
        waitForText("done", id: "dictation.state", in: phone, timeout: 180)
        editor.activate()
        let first = editor.textFields["editor.first"]
        XCTAssertTrue(first.waitForExistence(timeout: 10))
        let inserted = NSPredicate(format: "value CONTAINS[c] %@", "quick brown fox")
        expectation(for: inserted, evaluatedWith: first)
        waitForExpectations(timeout: 15)
        editor.textFields["editor.second"].tap()
        XCTAssertEqual(editor.textFields["editor.second"].value as? String, "Second field")
        XCTAssertFalse(element("keyboard.undo", in: editor).isEnabled)
        attach(editor, name: "Changed field")
    }

    func testServerStopsWhenTheAppLeavesTheForeground() {
        phone.tabBars.buttons["Server"].tap()
        phone.switches["server.toggle"].switches.firstMatch.tap()
        waitForText("Running", id: "server.state", in: phone, timeout: 20)
        XCTAssertEqual(element("server.port", in: phone).label, "7342")
        attach(phone, name: "Server running")
        editor.launch()
        phone.activate()
        waitForText("Stopped", id: "server.state", in: phone, timeout: 10)
        XCTAssertEqual(element("server.port", in: phone).label, "None")
        attach(phone, name: "Server stopped on background")
    }

    private func openEditor() {
        editor.launch()
        let field = editor.textFields["editor.first"]
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        field.tap()
        if !element("keyboard.mic", in: editor).waitForExistence(timeout: 2) {
            let globe = editor.buttons.matching(NSPredicate(format: "label == %@ OR label BEGINSWITH[c] %@", "Emoji", "Next keyboard")).firstMatch
            XCTAssertTrue(globe.waitForExistence(timeout: 5))
            globe.press(forDuration: 1)
            let keyboard = editor.descendants(matching: .any).matching(NSPredicate(format: "label == %@", "Hushpen")).firstMatch
            XCTAssertTrue(keyboard.waitForExistence(timeout: 5))
            keyboard.tap()
        }
        XCTAssertTrue(element("keyboard.mic", in: editor).waitForExistence(timeout: 15))
        attach(editor, name: "Keyboard ready")
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
        element("keyboard.mic", in: editor).tap()
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
