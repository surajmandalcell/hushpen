import Foundation
import XCTest
@testable import Hushpen

final class KeyboardStatusTests: XCTestCase {
    private let now = Date(timeIntervalSince1970: 1_000_000)

    func testHeartbeatPersistsFullAccessAndForegroundReadsFreshData() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let allowed = KeyboardHeartbeat(seenAt: now, hasFullAccess: true)
        try allowed.write(root: root)
        XCTAssertEqual(KeyboardHeartbeat.read(root: root, now: now), allowed)
        XCTAssertEqual(status(heartbeat: allowed).text, "Full Access last seen")
        let denied = KeyboardHeartbeat(seenAt: now, hasFullAccess: false)
        try denied.write(root: root)
        XCTAssertEqual(KeyboardHeartbeat.read(root: root, now: now), denied)
        XCTAssertEqual(status(heartbeat: denied).text, "Allow Full Access")
    }

    func testEnabledInputModeDoesNotClaimFullAccess() {
        let value = status(modes: ["<UIKeyboardExtensionInputMode: identifier=\(KeyboardStatus.bundleIdentifier)>"])
        XCTAssertTrue(value.added)
        XCTAssertNil(value.fullAccess)
        XCTAssertEqual(value.text, "Added")
        XCTAssertFalse(status(modes: ["\(KeyboardStatus.bundleIdentifier).other"]).added)
        XCTAssertFalse(status(modes: ["en-US"]).added)
    }

    func testOldHeartbeatPreservesAddedStateWithoutClaimingRecentAccess() {
        let old = KeyboardHeartbeat(seenAt: now.addingTimeInterval(-86_400), hasFullAccess: true)
        XCTAssertTrue(status(heartbeat: old).added)
        XCTAssertNil(status(heartbeat: old).fullAccess)
        XCTAssertEqual(status(heartbeat: old).text, "Added")
        let recent = KeyboardHeartbeat(seenAt: now.addingTimeInterval(-86_399), hasFullAccess: true)
        XCTAssertEqual(status(heartbeat: recent).fullAccess, true)
    }

    func testFutureAndMalformedHeartbeatsCannotCompleteSetup() throws {
        let future = KeyboardHeartbeat(seenAt: now.addingTimeInterval(1), hasFullAccess: true)
        XCTAssertFalse(status(heartbeat: future).added)
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try future.write(root: root)
        XCTAssertNil(KeyboardHeartbeat.read(root: root, now: now))
        try Data("damaged".utf8).write(to: root.appendingPathComponent("keyboard-status.json"))
        XCTAssertNil(KeyboardHeartbeat.read(root: root, now: now))
        XCTAssertNil(KeyboardHeartbeat.read(root: nil, now: now))
    }

    func testLegacyObservationSurvivesUpgradeWithoutClaimingCurrentAccess() {
        let value = status(legacySeenAt: now.addingTimeInterval(-100))
        XCTAssertTrue(value.added)
        XCTAssertNil(value.fullAccess)
        XCTAssertFalse(status(legacySeenAt: now.addingTimeInterval(1)).added)
    }

    private func status(modes: [String] = [], heartbeat: KeyboardHeartbeat? = nil, legacySeenAt: Date? = nil) -> KeyboardStatus {
        KeyboardStatus(modes: modes, heartbeat: heartbeat, legacySeenAt: legacySeenAt, now: now)
    }
}
