import CoreGraphics
import Foundation
import Observation

/// A key of the Hushpen keyboard that reads touches. The globe keeps its own control.
enum KeyID: Hashable, Sendable {
    case character(String), shift, backspace, symbols, emoji, space, period, enter

    /// Letter and symbol keys show the key callout; the other keys only change shade.
    var showsCallout: Bool {
        if case .character = self { true } else { false }
    }

    /// Delete acts when the finger lands and repeats while it stays, like the system keyboard.
    var actsOnTouchDown: Bool { self == .backspace }
}

/// The system keyboard's delete timing: the first repeat comes after half a second, then the rate climbs.
enum BackspaceRepeat {
    static let startDelay: TimeInterval = 0.5

    static func interval(afterRepeats count: Int) -> TimeInterval {
        switch count {
        case ..<10: 0.10
        case ..<25: 0.06
        default: 0.04
        }
    }
}

/// Follows each finger on the keys. A letter shows its callout at touch-down and types at
/// touch-up, on the key under the finger then. Delete types at touch-down and repeats.
@MainActor
@Observable
final class KeyTracker {
    struct Press: Equatable {
        let origin: KeyID
        var current: KeyID
    }

    private(set) var presses: [Int: Press] = [:]
    /// Key cells in the key area's coordinates; set by the keyboard view as it lays out.
    @ObservationIgnored var frames: [KeyID: CGRect] = [:]
    /// Feedback for the finger landing: the click and the haptic.
    @ObservationIgnored var onDown: () -> Void = {}
    @ObservationIgnored var onType: (KeyID) -> Void = { _ in }
    @ObservationIgnored private var repeats: [Int: Task<Void, Never>] = [:]

    var pressed: Set<KeyID> { Set(presses.values.map(\.current)) }

    /// The cell that holds the point; touches in no cell pass through to the globe and the emoji grid.
    func key(at point: CGPoint) -> KeyID? {
        frames.first { $0.value.contains(point) }?.key
    }

    /// A finger can wander off the cells, so the key it means is the closest one.
    func nearestKey(to point: CGPoint) -> KeyID? {
        if let inside = key(at: point) { return inside }
        return frames.min { distance(from: point, to: $0.value) < distance(from: point, to: $1.value) }?.key
    }

    func down(_ touch: Int, at point: CGPoint) {
        guard presses[touch] == nil, let key = nearestKey(to: point) else { return }
        presses[touch] = Press(origin: key, current: key)
        onDown()
        guard key.actsOnTouchDown else { return }
        onType(key)
        startRepeating(touch)
    }

    func move(_ touch: Int, to point: CGPoint) {
        guard var press = presses[touch], let key = nearestKey(to: point), key != press.current else { return }
        press.current = key
        presses[touch] = press
        if key != .backspace { stopRepeating(touch) }
    }

    func up(_ touch: Int, at point: CGPoint) {
        move(touch, to: point)
        guard let press = presses[touch] else { return }
        finish(touch)
        if !press.origin.actsOnTouchDown, !press.current.actsOnTouchDown { onType(press.current) }
    }

    /// A cancelled touch (a call, a system gesture) types nothing.
    func cancel(_ touch: Int) { finish(touch) }

    func cancelAll() {
        for touch in Array(presses.keys) { finish(touch) }
    }

    private func finish(_ touch: Int) {
        stopRepeating(touch)
        presses[touch] = nil
    }

    private func startRepeating(_ touch: Int) {
        repeats[touch] = Task { [weak self] in
            try? await Task.sleep(for: .seconds(BackspaceRepeat.startDelay))
            var count = 0
            while !Task.isCancelled {
                guard let self, self.presses[touch]?.current == .backspace else { return }
                self.onType(.backspace)
                count += 1
                try? await Task.sleep(for: .seconds(BackspaceRepeat.interval(afterRepeats: count)))
            }
        }
    }

    private func stopRepeating(_ touch: Int) {
        repeats.removeValue(forKey: touch)?.cancel()
    }

    private func distance(from point: CGPoint, to rect: CGRect) -> CGFloat {
        let dx = max(rect.minX - point.x, 0, point.x - rect.maxX)
        let dy = max(rect.minY - point.y, 0, point.y - rect.maxY)
        return hypot(dx, dy)
    }
}
