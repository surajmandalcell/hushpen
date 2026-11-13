import XCTest

/// Dictation into the system Messages field. Messages edits the field without telling the keyboard,
/// so the keyboard reads the old text for a long time. A text that landed must not end as a paste failure.
@MainActor
final class MessagesUITests: XCTestCase {
    private let phone = XCUIApplication(bundleIdentifier: "com.mandalsuraj.hushpen")
    private let messages = XCUIApplication(bundleIdentifier: "com.apple.MobileSMS")

    override func setUp() async throws {
        try await super.setUp()
        continueAfterFailure = false
        phone.launchEnvironment["HUSHPEN_TEST_WAV"] = "documents"
        phone.launchEnvironment["HUSHPEN_TEST_MODEL"] = "ggml-base-q5_1.bin"
    }

    override func tearDown() async throws {
        phone.terminate()
        messages.terminate()
        try await super.tearDown()
    }

    func testDictatedTextLandsInAnEmptyMessageWithoutAPasteFailure() {
        dictateIntoMessages(typing: [])
    }

    func testDictatedTextLandsAfterTypedTextWithoutAPasteFailure() {
        dictateIntoMessages(typing: ["h", "i"])
    }

    private func dictateIntoMessages(typing keys: [String]) {
        phone.launch()
        XCTAssertTrue(element("home.words", in: phone).waitForExistence(timeout: 30))
        XCUIDevice.shared.press(.home)
        messages.launch()
        let conversation = messages.cells.firstMatch
        XCTAssertTrue(conversation.waitForExistence(timeout: 20))
        conversation.tap()
        let field = messages.textFields["messageBodyField"]
        XCTAssertTrue(field.waitForExistence(timeout: 15))
        field.tap()
        let intro = messages.buttons["Continue"]
        if intro.waitForExistence(timeout: 4) { intro.tap() }
        useKeyboard()
        let microphone = element("keyboard.mic", in: messages)
        XCTAssertTrue(microphone.waitForExistence(timeout: 15))
        for key in keys { element("keyboard.key.\(key)", in: messages).tap() }
        microphone.tap()
        waitForText("listening", id: "dictation.state", in: phone, timeout: 30)
        sleep(2)
        element("dictation.finish", in: phone).tap()
        waitForText("done", id: "dictation.state", in: phone, timeout: 180)
        XCUIDevice.shared.press(.home)
        messages.activate()
        let inserted = NSPredicate(format: "value CONTAINS[c] %@", "quick brown fox")
        expectation(for: inserted, evaluatedWith: field)
        waitForExpectations(timeout: 30)
        // The keyboard judges the insert within four seconds. A failure stays until it is dismissed.
        let status = element("keyboard.status", in: messages)
        XCTAssertTrue(status.waitForExistence(timeout: 10))
        sleep(6)
        XCTAssertFalse(status.label.hasPrefix("Paste failed"), status.label)
        XCTAssertFalse(element("keyboard.dismiss", in: messages).exists)
        // Moving the cursor to read the field must put it back at the end of the text.
        element("keyboard.key.z", in: messages).tap()
        let typed = NSPredicate(format: "value ENDSWITH %@", "5.z")
        expectation(for: typed, evaluatedWith: field)
        waitForExpectations(timeout: 10)
        XCTAssertTrue((field.value as? String)?.hasPrefix(keys.joined()) == true)
    }

    private func useKeyboard() {
        if !element("keyboard.status", in: messages).waitForExistence(timeout: 2) {
            let globe = messages.buttons.matching(NSPredicate(format: "(label == %@ OR label BEGINSWITH[c] %@) AND identifier != %@", "Emoji", "Next keyboard", "keyboard.next")).firstMatch
            XCTAssertTrue(globe.waitForExistence(timeout: 5))
            globe.press(forDuration: 1)
            let keyboard = messages.descendants(matching: .any).matching(NSPredicate(format: "label == %@", "Hushpen")).firstMatch
            XCTAssertTrue(keyboard.waitForExistence(timeout: 5))
            keyboard.tap()
        }
        XCTAssertTrue(element("keyboard.status", in: messages).waitForExistence(timeout: 15))
    }

    private func waitForText(_ text: String, id: String, in app: XCUIApplication, timeout: TimeInterval) {
        let value = element(id, in: app)
        XCTAssertTrue(value.waitForExistence(timeout: timeout))
        expectation(for: NSPredicate(format: "label CONTAINS[c] %@", text), evaluatedWith: value)
        waitForExpectations(timeout: timeout)
    }

    private func element(_ id: String, in app: XCUIApplication) -> XCUIElement {
        app.descendants(matching: .any).matching(identifier: id).firstMatch
    }
}
