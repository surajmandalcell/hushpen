import Darwin
import Foundation
import notify

/// The live microphone level, from the app to the keyboard. The level goes in a file in the App Group
/// container, which only the app and the keyboard can read. The Darwin notification carries no data:
/// any app on the phone can read its state, and that would show when and how loud the user speaks.
enum FlowLevel {
    static let name = "com.mandalsuraj.hushpen.flow.level"
    /// The level has `steps + 1` values, which is enough for the curve and shows little.
    static let steps = 7

    private static var file: URL? {
        FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: FlowSessionStore.groupIdentifier)?
            .appendingPathComponent("flow.level")
    }

    /// A reading in decibels (about -50 quiet, 0 loud) as 0...1.
    static func normalized(decibels: Float) -> Double {
        let value = Double((decibels + 50) / 50)
        return pow(min(max(value, 0), 1), 1.5)
    }

    static func encode(_ level: Double) -> UInt8 { UInt8((min(max(level, 0), 1) * Double(steps)).rounded()) }

    static func decode(_ step: UInt8) -> Double { Double(min(Int(step), steps)) / Double(steps) }

    final class Publisher {
        private let file = FlowLevel.file

        func set(_ level: Double) {
            guard let file else { return }
            try? Data([FlowLevel.encode(level)]).write(to: file)
            notify_post(FlowLevel.name)
        }

        deinit { if let file { try? FileManager.default.removeItem(at: file) } }
    }

    final class Observer {
        private var token: Int32 = -1

        init(changed: @escaping @Sendable (Double) -> Void) throws {
            let file = FlowLevel.file
            let status = notify_register_dispatch(FlowLevel.name, &token, .main) { _ in
                let step = file.flatMap { try? Data(contentsOf: $0).first } ?? 0
                changed(FlowLevel.decode(step))
            }
            guard status == NOTIFY_STATUS_OK else { throw FlowSessionError.unavailable }
        }

        deinit { if token != -1 { notify_cancel(token) } }
    }
}
