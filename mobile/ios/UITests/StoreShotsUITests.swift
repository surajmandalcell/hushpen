import XCTest

/// Captures the App Store screenshots from the real app on a clean simulator.
/// Every test skips unless `TEST_RUNNER_HUSHPEN_SHOTS_DIR` names the output folder.
/// The methods depend on each other's state, so run them in name order on one
/// simulator; `mobile/README.md` lists the steps.
@MainActor
final class StoreShotsUITests: XCTestCase {
    private let phone = XCUIApplication(bundleIdentifier: "com.mandalsuraj.hushpen")
    private let messages = XCUIApplication(bundleIdentifier: "com.apple.MobileSMS")
    private var folder = ""

    override func setUp() async throws {
        try await super.setUp()
        continueAfterFailure = false
        let requested = ProcessInfo.processInfo.environment["HUSHPEN_SHOTS_DIR"]
        try XCTSkipUnless(requested != nil, "HUSHPEN_SHOTS_DIR is not set")
        folder = requested ?? ""
        try FileManager.default.createDirectory(atPath: folder, withIntermediateDirectories: true)
        phone.launchEnvironment["HUSHPEN_TEST_WAV"] = "documents"
        phone.launchEnvironment["HUSHPEN_TEST_MODEL"] = "ggml-base-q5_1.bin"
    }

    func test1EnableKeyboard() {
        phone.launch()
        XCTAssertTrue(phone.navigationBars["Hushpen"].waitForExistence(timeout: 30))
        let settings = XCUIApplication(bundleIdentifier: "com.apple.Preferences")
        settings.launch()
        let general = settings.staticTexts["General"].firstMatch
        XCTAssertTrue(general.waitForExistence(timeout: 10))
        general.tap()
        let keyboard = settings.staticTexts["Keyboard"].firstMatch
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
        }
        settings.terminate()
    }

    func test2MessagesDictation() {
        phone.launch()
        XCTAssertTrue(phone.navigationBars["Hushpen"].waitForExistence(timeout: 30))
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
        shot("messages-keyboard")
        microphone.tap()
        waitForText("listening", id: "dictation.state", in: phone, timeout: 30)
        sleep(3)
        shot("dictation-listening")
        element("dictation.finish", in: phone).tap()
        waitForText("done", id: "dictation.state", in: phone, timeout: 180)
        sleep(1)
        shot("dictation-done")
        XCUIDevice.shared.press(.home)
        messages.activate()
        let insert = element("keyboard.insert", in: messages)
        XCTAssertTrue(insert.waitForExistence(timeout: 15))
        sleep(1)
        shot("messages-insert")
        insert.tap()
        let inserted = NSPredicate(format: "value CONTAINS[c] %@", "final draft")
        expectation(for: inserted, evaluatedWith: field)
        waitForExpectations(timeout: 15)
        sleep(1)
        shot("messages-inserted")
    }

    func test3Screens() {
        phone.terminate()
        messages.terminate()
        XCUIDevice.shared.press(.home)
        phone.launch()
        XCTAssertTrue(phone.navigationBars["Hushpen"].waitForExistence(timeout: 30))
        XCTAssertTrue(element("home.models", in: phone).waitForExistence(timeout: 15))
        sleep(2)
        shot("home")
        element("home.models", in: phone).tap()
        XCTAssertTrue(phone.navigationBars["Models"].waitForExistence(timeout: 10))
        sleep(2)
        shot("models")
        phone.navigationBars.buttons.firstMatch.tap()
        element("home.devices", in: phone).tap()
        XCTAssertTrue(phone.navigationBars["Devices"].waitForExistence(timeout: 10))
        phone.switches["devices.server"].switches.firstMatch.tap()
        sleep(3)
        shot("devices")
        phone.switches["devices.server"].switches.firstMatch.tap()
        sleep(1)
        phone.navigationBars.buttons.firstMatch.tap()
        element("home.settings", in: phone).tap()
        XCTAssertTrue(phone.navigationBars["Settings"].waitForExistence(timeout: 10))
        sleep(2)
        shot("settings")
        phone.navigationBars.buttons.firstMatch.tap()
        phone.switches["server.toggle"].switches.firstMatch.tap()
        waitForText("Serving", id: "server.state", in: phone, timeout: 20)
        sleep(2)
        shot("server")
        element("server.stop", in: phone).tap()
    }

    func test4LiveActivity() {
        phone.launch()
        XCTAssertTrue(phone.navigationBars["Hushpen"].waitForExistence(timeout: 30))
        XCUIDevice.shared.press(.home)
        messages.launch()
        let conversation = messages.cells.firstMatch
        XCTAssertTrue(conversation.waitForExistence(timeout: 20))
        conversation.tap()
        let field = messages.textFields["messageBodyField"]
        XCTAssertTrue(field.waitForExistence(timeout: 15))
        field.tap()
        useKeyboard()
        element("keyboard.mic", in: messages).tap()
        waitForText("listening", id: "dictation.state", in: phone, timeout: 30)
        sleep(2)
        XCUIDevice.shared.press(.home)
        sleep(3)
        shot("island-home")
        XCUIDevice.shared.perform(NSSelectorFromString("pressLockButton"))
        sleep(3)
        let allow = XCUIApplication(bundleIdentifier: "com.apple.springboard").buttons["Allow"]
        if allow.waitForExistence(timeout: 4) { allow.tap() }
        sleep(3)
        shot("lock")
    }

    func test5Cleanup() {
        phone.terminate()
        messages.terminate()
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

    private func shot(_ name: String) {
        let png = XCUIScreen.main.screenshot().pngRepresentation
        try? png.write(to: URL(fileURLWithPath: folder).appendingPathComponent("\(name).png"))
    }

    private func element(_ id: String, in app: XCUIApplication) -> XCUIElement {
        app.descendants(matching: .any).matching(identifier: id).firstMatch
    }
}
