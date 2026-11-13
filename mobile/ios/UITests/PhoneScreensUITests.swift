import XCTest

/// Saves a screenshot of every phone screen with demo history. Skips unless `TEST_RUNNER_HUSHPEN_SHOTS_DIR`
/// names the output folder. `TEST_RUNNER_HUSHPEN_SHOTS_SUFFIX` ends each file name (`light`, `dark`, `large`);
/// "large" also sets a big Dynamic Type size and keeps only the History list.
@MainActor
final class PhoneScreensUITests: XCTestCase {
    private let phone = XCUIApplication(bundleIdentifier: "com.mandalsuraj.hushpen")

    func testScreens() throws {
        let environment = ProcessInfo.processInfo.environment
        let folder = try XCTUnwrap(environment["HUSHPEN_SHOTS_DIR"], "HUSHPEN_SHOTS_DIR is not set")
        let suffix = environment["HUSHPEN_SHOTS_SUFFIX"] ?? "light"
        try FileManager.default.createDirectory(atPath: folder, withIntermediateDirectories: true)
        continueAfterFailure = false
        phone.launchEnvironment["HUSHPEN_TEST_HISTORY"] = "demo"
        phone.launchEnvironment["HUSHPEN_TEST_BATTERY"] = "64,charging"
        if suffix == "large" { phone.launchArguments += ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXL"] }
        phone.launch()
        XCTAssertTrue(element("home.words").waitForExistence(timeout: 30))
        sleep(2)
        func shot(_ name: String) {
            try? XCUIScreen.main.screenshot().pngRepresentation.write(to: URL(fileURLWithPath: folder).appendingPathComponent("\(name)-\(suffix).png"))
        }
        if suffix != "large" { shot("home") }
        phone.tabBars.buttons["History"].tap()
        sleep(1)
        shot("history")
        guard suffix != "large" else { return }
        phone.scrollViews.descendants(matching: .any).matching(identifier: "history.card").firstMatch.tap()
        XCTAssertTrue(element("history.copy").waitForExistence(timeout: 10))
        sleep(1)
        shot("history-detail")
        phone.navigationBars.buttons.firstMatch.tap()
        phone.tabBars.buttons["Stats"].tap()
        sleep(2)
        shot("stats")
        phone.tabBars.buttons["Settings"].tap()
        sleep(1)
        shot("settings")
        element("settings.models").tap()
        sleep(2)
        shot("models")
        phone.navigationBars.buttons.firstMatch.tap()
        element("settings.devices").tap()
        XCTAssertTrue(phone.windows.firstMatch.wait(for: \.frame, toEqual: phone.frame, timeout: 10))
        sleep(1)
        shot("devices-off")
        phone.switches["devices.server"].switches.firstMatch.tap()
        let screen = element("devices.screen")
        XCTAssertTrue(screen.waitForExistence(timeout: 20))
        sleep(2)
        shot("devices")
        screen.tap()
        XCTAssertTrue(element("server.state").waitForExistence(timeout: 10))
        sleep(2)
        shot("server")
        element("server.stop").tap()
    }

    private func element(_ id: String) -> XCUIElement {
        phone.descendants(matching: .any).matching(identifier: id).firstMatch
    }
}
