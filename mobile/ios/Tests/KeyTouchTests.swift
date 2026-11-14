import CoreGraphics
import Foundation
import XCTest
@testable import Hushpen

@MainActor
final class KeyTouchTests: XCTestCase {
    private let h = KeyID.character("h")
    private let i = KeyID.character("i")

    /// Three cells side by side: h, i, delete.
    private func tracker() -> (KeyTracker, typed: Box) {
        let box = Box()
        let tracker = KeyTracker()
        tracker.frames = [
            h: CGRect(x: 0, y: 0, width: 40, height: 54),
            i: CGRect(x: 40, y: 0, width: 40, height: 54),
            .backspace: CGRect(x: 80, y: 0, width: 60, height: 54)
        ]
        tracker.onDown = { box.downs += 1 }
        tracker.onType = { box.typed.append($0) }
        return (tracker, box)
    }

    final class Box {
        var typed: [KeyID] = []
        var downs = 0
    }

    func testLetterShowsCalloutAtTouchDownAndTypesAtTouchUp() {
        let (tracker, box) = tracker()
        tracker.down(1, at: CGPoint(x: 10, y: 10))
        XCTAssertEqual(tracker.pressed, [h])
        XCTAssertTrue(h.showsCallout)
        XCTAssertEqual(box.downs, 1)
        XCTAssertEqual(box.typed, [], "a letter must wait for touch-up")
        tracker.up(1, at: CGPoint(x: 10, y: 10))
        XCTAssertEqual(box.typed, [h])
        XCTAssertTrue(tracker.pressed.isEmpty)
    }

    func testLetterTypesTheKeyUnderTheFingerAtTouchUp() {
        let (tracker, box) = tracker()
        tracker.down(1, at: CGPoint(x: 38, y: 10))
        tracker.move(1, to: CGPoint(x: 45, y: 10))
        XCTAssertEqual(tracker.pressed, [i])
        tracker.up(1, at: CGPoint(x: 45, y: 10))
        XCTAssertEqual(box.typed, [i])
    }

    func testFingerOutsideEveryCellMeansTheNearestKey() {
        let (tracker, box) = tracker()
        tracker.down(1, at: CGPoint(x: 10, y: 10))
        tracker.up(1, at: CGPoint(x: 70, y: -30))
        XCTAssertEqual(box.typed, [i])
        XCTAssertNil(tracker.key(at: CGPoint(x: 70, y: -30)))
    }

    func testPeriodAndEmojiKeysTypeAtTouchUpWithoutCallout() {
        let (tracker, box) = tracker()
        tracker.frames[.period] = CGRect(x: 140, y: 0, width: 40, height: 54)
        tracker.frames[.emoji] = CGRect(x: 180, y: 0, width: 40, height: 54)
        for (point, key) in [(CGPoint(x: 150, y: 10), KeyID.period), (CGPoint(x: 190, y: 10), .emoji)] {
            tracker.down(1, at: point)
            XCTAssertEqual(tracker.pressed, [key])
            XCTAssertFalse(key.showsCallout)
            XCTAssertFalse(key.actsOnTouchDown)
            XCTAssertEqual(box.typed.last == key, false, "waits for touch-up")
            tracker.up(1, at: point)
        }
        XCTAssertEqual(box.typed, [.period, .emoji])
    }

    func testCancelledTouchTypesNothing() {
        let (tracker, box) = tracker()
        tracker.down(1, at: CGPoint(x: 10, y: 10))
        tracker.cancel(1)
        XCTAssertEqual(box.typed, [])
        XCTAssertTrue(tracker.pressed.isEmpty)
    }

    func testTwoFingersTypeTwoKeys() {
        let (tracker, box) = tracker()
        tracker.down(1, at: CGPoint(x: 10, y: 10))
        tracker.down(2, at: CGPoint(x: 50, y: 10))
        XCTAssertEqual(tracker.pressed, [h, i])
        tracker.up(2, at: CGPoint(x: 50, y: 10))
        tracker.up(1, at: CGPoint(x: 10, y: 10))
        XCTAssertEqual(box.typed, [i, h])
    }

    func testDeleteTypesAtTouchDownOnceAndNotAtTouchUp() {
        let (tracker, box) = tracker()
        tracker.down(1, at: CGPoint(x: 100, y: 10))
        XCTAssertEqual(box.typed, [.backspace])
        XCTAssertFalse(KeyID.backspace.showsCallout)
        tracker.up(1, at: CGPoint(x: 100, y: 10))
        XCTAssertEqual(box.typed, [.backspace])
    }

    func testDeleteRepeatsWhileHeldAndStopsAtTouchUp() async throws {
        let (tracker, box) = tracker()
        tracker.down(1, at: CGPoint(x: 100, y: 10))
        try await Task.sleep(for: .seconds(BackspaceRepeat.startDelay - 0.2))
        XCTAssertEqual(box.typed.count, 1, "no repeat before the start delay")
        try await Task.sleep(for: .seconds(0.6))
        XCTAssertGreaterThan(box.typed.count, 2, "repeats after the start delay")
        tracker.up(1, at: CGPoint(x: 100, y: 10))
        let stopped = box.typed.count
        try await Task.sleep(for: .seconds(0.4))
        XCTAssertEqual(box.typed.count, stopped)
    }

    func testBackspaceRepeatStartsAtHalfASecondAndAccelerates() {
        XCTAssertEqual(BackspaceRepeat.startDelay, 0.5)
        let steps = (0..<60).map(BackspaceRepeat.interval(afterRepeats:))
        XCTAssertEqual(steps.first, 0.1)
        XCTAssertLessThan(steps.last!, steps.first!)
        XCTAssertEqual(steps, steps.sorted(by: >), "the interval never grows")
    }

    /// The keys must press and release at once. The Keys section of the view has no animation, fade, or scale.
    func testKeyViewsHaveNoAnimationModifiers() throws {
        let file = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("Keyboard/KeyboardView.swift")
        let source = try String(contentsOf: file, encoding: .utf8)
        let section = try XCTUnwrap(source.components(separatedBy: "// MARK: - Keys").last)
        for banned in [".animation(", "withAnimation", ".opacity(", ".scaleEffect(", ".transition(", "Animation", ".blur(", ".withAnimation"] {
            XCTAssertFalse(section.contains(banned), "Keys section uses \(banned)")
        }
        XCTAssertTrue(section.contains("$0.animation = nil"), "the key area must strip inherited animations")
    }
}
