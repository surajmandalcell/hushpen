import Darwin
import Foundation
import notify

/// The live microphone level, from the app to the keyboard. A Darwin notification carries a 64-bit
/// state, which is enough for one number and needs no file or Full Access.
enum FlowLevel {
    static let name = "com.mandalsuraj.hushpen.flow.level"

    /// A reading in decibels (about -50 quiet, 0 loud) as 0...1.
    static func normalized(decibels: Float) -> Double {
        let value = Double((decibels + 50) / 50)
        return pow(min(max(value, 0), 1), 1.5)
    }

    static func encode(_ level: Double) -> UInt64 { UInt64((min(max(level, 0), 1) * 1000).rounded()) }

    static func decode(_ state: UInt64) -> Double { min(Double(state), 1000) / 1000 }

    final class Publisher {
        private var token: Int32 = -1

        init() { notify_register_check(FlowLevel.name, &token) }

        deinit { if token != -1 { notify_cancel(token) } }

        func set(_ level: Double) {
            guard token != -1 else { return }
            notify_set_state(token, FlowLevel.encode(level))
            notify_post(FlowLevel.name)
        }
    }

    final class Observer {
        private var token: Int32 = -1

        init(changed: @escaping @Sendable (Double) -> Void) throws {
            let status = notify_register_dispatch(FlowLevel.name, &token, .main) { token in
                var state: UInt64 = 0
                notify_get_state(token, &state)
                changed(FlowLevel.decode(state))
            }
            guard status == NOTIFY_STATUS_OK else { throw FlowSessionError.unavailable }
        }

        deinit { if token != -1 { notify_cancel(token) } }
    }
}
