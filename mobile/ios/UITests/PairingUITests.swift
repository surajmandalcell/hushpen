import XCTest

/// The phone side of D-57. Shots need `TEST_RUNNER_HUSHPEN_SHOTS_DIR`; `TEST_RUNNER_HUSHPEN_SHOTS_SUFFIX` ends each name.
/// The Nearby test also needs a computer: run the ignored `nearby_peer` test of hushpen-mobile and set
/// `TEST_RUNNER_HUSHPEN_PAIR_NUMBER_FILE` to the file that peer reads.
@MainActor
final class PairingUITests: XCTestCase {
    private let phone = XCUIApplication(bundleIdentifier: "com.mandalsuraj.hushpen")
    private let environment = ProcessInfo.processInfo.environment

    private func launch(_ extra: [String: String] = [:]) {
        continueAfterFailure = false
        phone.launchEnvironment["HUSHPEN_TEST_BATTERY"] = "64,charging"
        for (key, value) in extra { phone.launchEnvironment[key] = value }
        phone.launch()
    }

    private func shot(_ name: String) {
        guard let folder = environment["HUSHPEN_SHOTS_DIR"] else { return }
        try? FileManager.default.createDirectory(atPath: folder, withIntermediateDirectories: true)
        let suffix = environment["HUSHPEN_SHOTS_SUFFIX"] ?? "light"
        try? XCUIScreen.main.screenshot().pngRepresentation.write(to: URL(fileURLWithPath: folder).appendingPathComponent("\(name)-\(suffix).png"))
    }

    private func element(_ id: String) -> XCUIElement {
        phone.descendants(matching: .any).matching(identifier: id).firstMatch
    }

    private func openDevicesWithServer() {
        XCTAssertTrue(element("home.words").waitForExistence(timeout: 30))
        phone.tabBars.buttons["Settings"].tap()
        element("settings.devices").tap()
        XCTAssertTrue(phone.windows.firstMatch.wait(for: \.frame, toEqual: phone.frame, timeout: 10))
        shot("pair-1-devices-off")
        phone.switches["devices.server"].switches.firstMatch.tap()
        XCTAssertTrue(element("devices.screen").waitForExistence(timeout: 20))
        sleep(1)
    }

    func testScanAndTypeCodeFallback() throws {
        launch()
        openDevicesWithServer()
        shot("pair-2-devices-listening")
        element("devices.scan").tap()
        XCTAssertTrue(element("scanner.type").waitForExistence(timeout: 10))
        sleep(1)
        shot("pair-3-scan")
        element("scanner.type").tap()
        let code = element("type.code")
        XCTAssertTrue(code.waitForExistence(timeout: 10))
        XCTAssertFalse(element("type.pair").isEnabled)
        code.typeText("48291637")
        XCTAssertEqual(code.value as? String, "4829 1637")
        XCTAssertTrue(element("type.pair").isEnabled)
        shot("pair-4-type-code")
        element("type.pair").tap()
        let hint = phone.staticTexts.matching(NSPredicate(format: "label BEGINSWITH 'Type the code'")).firstMatch
        XCTAssertTrue(hint.waitForNonExistence(timeout: 30), "Pairing with nobody must show an error")
        shot("pair-5-type-code-error")
    }

    func testNumberComparisonCancelAndPair() throws {
        launch(["HUSHPEN_TEST_PAIRING": "compare"])
        let number = element("pairing.number")
        XCTAssertTrue(number.waitForExistence(timeout: 30))
        XCTAssertEqual(number.label, "482 913")
        shot("pair-6-compare")
        element("pairing.cancel").tap()
        XCTAssertTrue(number.waitForNonExistence(timeout: 10))
    }

    /// A real computer connects through Nearby; both sides compare, and this test taps Pair.
    func testNearbyPairingWithARealComputer() throws {
        let file = try XCTUnwrap(environment["HUSHPEN_PAIR_NUMBER_FILE"], "HUSHPEN_PAIR_NUMBER_FILE is not set")
        launch()
        openDevicesWithServer()
        let number = element("pairing.number")
        XCTAssertTrue(number.waitForExistence(timeout: 120))
        let shown = number.label.replacingOccurrences(of: " ", with: "")
        XCTAssertEqual(shown.count, 6)
        try shown.write(toFile: file, atomically: true, encoding: .utf8)
        shot("pair-7-nearby-compare")
        element("pairing.pair").tap()
        XCTAssertTrue(number.waitForNonExistence(timeout: 20))
        let row = phone.staticTexts.matching(NSPredicate(format: "label BEGINSWITH 'Desktop '")).firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 20))
        sleep(1)
        shot("pair-8-paired")
    }
}
