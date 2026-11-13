import UIKit
import XCTest
@testable import Hushpen

final class PowerTests: XCTestCase {
    func testBatteryStateMapsToWhatTheRowSays() {
        XCTAssertEqual(PowerModel.charging(.charging), true)
        XCTAssertEqual(PowerModel.charging(.full), true)
        XCTAssertEqual(PowerModel.charging(.unplugged), false)
    }

    /// A Simulator reports no battery. The row then has nothing to show, and never says "Unknown".
    func testAnUnknownBatteryStateIsNotAnAnswer() {
        XCTAssertNil(PowerModel.charging(.unknown))
    }

    @MainActor
    func testTheDeviceKindIsWhatTheDeviceIs() {
        XCTAssertEqual(deviceKind, UIDevice.current.userInterfaceIdiom == .pad ? "iPad" : "iPhone")
    }
}
