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
        XCTAssertTrue(element("home.words", in: phone).waitForExistence(timeout: 30))
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
        cancelFromKeyboard()
        phone.activate()
        element("dictation.recover", in: phone).tap()
        waitForText("done", id: "dictation.state", in: phone, timeout: 180)
        editor.activate()
        let field = editor.textFields["editor.first"]
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        waitForInsertion(in: field)
        let once = field.value as? String
        XCTAssertTrue((once ?? "").hasPrefix("hi "))
        let undo = element("keyboard.undo", in: editor)
        XCTAssertTrue(undo.waitForExistence(timeout: 10))
        attach(editor, name: "Inserted", keep: true)

        XCTAssertEqual(field.value as? String, once)
        undo.tap()
        XCTAssertEqual(field.value as? String, "hi ")
        phone.activate()
        editor.activate()
        useKeyboard()
        XCTAssertEqual(field.value as? String, "hi ")
        XCTAssertFalse(element("keyboard.undo", in: editor).exists)
    }

    func testSetupRefreshesWhenReturningFromTheKeyboard() {
        phone.tabBars.buttons["Settings"].tap()
        XCTAssertTrue(element("onboarding.keyboard", in: phone).waitForExistence(timeout: 10))
        XCTAssertTrue(element("onboarding.fullAccess", in: phone).exists)
        let added = phone.staticTexts["On"].exists
        let observed = phone.staticTexts["Allowed"].exists
        print("PHONE_KEYBOARD_AFTER_SETTINGS added=\(added) full_access_observed=\(observed)")
        openEditor()
        phone.activate()
        XCTAssertTrue(phone.staticTexts["On"].waitForExistence(timeout: 10))
        XCTAssertTrue(phone.staticTexts["Allowed"].waitForExistence(timeout: 10))
    }

    func testCancelDoesNotInsertText() {
        openEditor()
        startFromKeyboard()
        cancelFromKeyboard()
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
        attach(editor, name: "Listening", keep: true)
        microphone.tap()
        waitForInsertion(in: editor.textFields["editor.first"])
    }

    func testTypingWhileTranscribingKeepsTheDictation() {
        openEditor()
        startFromKeyboard()
        editor.activate()
        XCTAssertTrue(element("keyboard.finish", in: editor).waitForExistence(timeout: 10))
        attach(editor, name: "Listening", keep: true)
        // Shift is armed first so the letter follows the finish tap at once, while the app still transcribes.
        element("keyboard.shift", in: editor).tap()
        element("keyboard.finish", in: editor).tap()
        element("keyboard.key.z", in: editor).tap()
        // Typing must not end the pill: it still says Transcribing with the letter in the field.
        let status = element("keyboard.status", in: editor)
        attach(editor, name: "Typed while transcribing", keep: true)
        XCTAssertEqual(status.label, "Transcribing")
        element("keyboard.shift", in: editor).tap()
        element("keyboard.key.q", in: editor).tap()
        let field = editor.textFields["editor.first"]
        waitForInsertion(in: field)
        // The dictation holds no capital Z or Q, so both stay, wherever the cursor was when it arrived.
        let value = field.value as? String ?? ""
        XCTAssertTrue(value.contains("Z") && value.contains("Q"), value)
    }

    func testFailedInsertCopiesTheTextAndOffersIt() {
        openEditor()
        let refusing = element("editor.refusing", in: editor)
        refusing.tap()
        useKeyboard()
        startFromKeyboard()
        editor.activate()
        element("keyboard.finish", in: editor).tap()
        phone.activate()
        waitForText("done", id: "dictation.state", in: phone, timeout: 180)
        editor.activate()
        let status = element("keyboard.status", in: editor)
        XCTAssertTrue(status.waitForExistence(timeout: 10))
        let failed = NSPredicate(format: "label BEGINSWITH %@", "Paste failed.")
        expectation(for: failed, evaluatedWith: status)
        waitForExpectations(timeout: 30)
        XCTAssertTrue(element("keyboard.dismiss", in: editor).exists || element("keyboard.copy", in: editor).exists)
        XCTAssertEqual(editor.textFields["editor.first"].value as? String, "First field")
        attach(editor, name: "Paste failed", keep: true)
        if element("keyboard.copy", in: editor).exists {
            element("keyboard.copy", in: editor).tap()
        }
        element("keyboard.dismiss", in: editor).tap()
        XCTAssertFalse(element("keyboard.dismiss", in: editor).exists)
    }

    func testServerStopsWhenTheAppLeavesTheForeground() {
        XCTAssertTrue(phone.wait(for: .runningForeground, timeout: 10))
        XCTAssertTrue(phone.windows.firstMatch.wait(for: \.frame, toEqual: phone.frame, timeout: 10))
        phone.tabBars.buttons["Settings"].tap()
        element("settings.devices", in: phone).tap()
        phone.switches["devices.server"].switches.firstMatch.tap()
        element("devices.screen", in: phone).tap()
        waitForText("Serving", id: "server.state", in: phone, timeout: 20)
        XCTAssertEqual(element("server.port", in: phone).label, "Port, 7342")
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
        XCTAssertEqual(element("server.port", in: phone).label, "Port, None")
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
        if element("keyboard.cancel", in: editor).exists {
            element("keyboard.cancel", in: editor).tap()
        }
        XCTAssertTrue(element("keyboard.mic", in: editor).waitForExistence(timeout: 15))
        attach(editor, name: "Keyboard ready", keep: true)
    }

    /// The keyboard inserts finished text on its own, at the cursor the field has then.
    private func waitForInsertion(in field: XCUIElement) {
        let inserted = NSPredicate(format: "value CONTAINS[c] %@", "quick brown fox")
        expectation(for: inserted, evaluatedWith: field)
        waitForExpectations(timeout: 180)
    }

    private func useKeyboard() {
        if !element("keyboard.status", in: editor).waitForExistence(timeout: 2) {
            let globe = editor.buttons.matching(NSPredicate(format: "(label == %@ OR label BEGINSWITH[c] %@) AND identifier != %@", "Emoji", "Next keyboard", "keyboard.next")).firstMatch
            XCTAssertTrue(globe.waitForExistence(timeout: 5))
            globe.press(forDuration: 1)
            attach(editor, name: "Keyboard picker")
            let keyboard = editor.descendants(matching: .any).matching(NSPredicate(format: "label BEGINSWITH %@", "Hushpen")).firstMatch
            XCTAssertTrue(keyboard.waitForExistence(timeout: 10))
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
    }

    private func startFromKeyboard() {
        let microphone = element("keyboard.mic", in: editor)
        XCTAssertTrue(microphone.waitForExistence(timeout: 10))
        microphone.tap()
        waitForText("listening", id: "dictation.state", in: phone, timeout: 30)
        sleep(1)
        attach(phone, name: "App recording", keep: true)
    }

    /// The hand-off page has no Cancel or Done; the pill in the keyboard holds them.
    private func cancelFromKeyboard() {
        editor.activate()
        XCTAssertTrue(element("keyboard.cancel", in: editor).waitForExistence(timeout: 10))
        element("keyboard.cancel", in: editor).tap()
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

    private func attach(_ app: XCUIApplication, name: String, keep: Bool = false) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = keep ? .keepAlways : .deleteOnSuccess
        add(attachment)
    }
}
