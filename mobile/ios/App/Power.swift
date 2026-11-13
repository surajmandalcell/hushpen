import Observation
import SwiftUI
import UIKit

/// The phone battery for the Power row: how full it is and whether it charges.
@Observable @MainActor
final class PowerModel {
    private(set) var level: Double?
    private(set) var charging = false

    func start() {
        UIDevice.current.isBatteryMonitoringEnabled = true
        read()
    }

    func stop() { UIDevice.current.isBatteryMonitoringEnabled = false }

    func read() {
        #if DEBUG
        // A simulator has no battery; a test or screenshot run sets one: "64,charging" or "38".
        if let fake = ProcessInfo.processInfo.environment["HUSHPEN_TEST_BATTERY"] {
            let parts = fake.split(separator: ",")
            level = parts.first.flatMap { Double($0) }.map { $0 / 100 }
            charging = parts.contains("charging")
            return
        }
        #endif
        let device = UIDevice.current
        level = device.batteryLevel >= 0 ? Double(device.batteryLevel) : nil
        charging = device.batteryState == .charging || device.batteryState == .full
    }

    var percent: String { level.map { "\(Int(($0 * 100).rounded()))%" } ?? "Unknown" }
    var status: String { charging ? "Charging" : "On battery" }
}

/// The battery icon followed by the percent at its right, as the Power row ends.
struct BatteryReading: View {
    let power: PowerModel

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: charging ? "battery.100percent.bolt" : "battery.100percent", variableValue: power.level ?? 1)
                .symbolRenderingMode(.palette)
                .foregroundStyle(charging ? Color.green : Color.primary, .secondary)
                .accessibilityHidden(true)
            Text(power.percent).monospacedDigit().foregroundStyle(.secondary)
        }
        .accessibilityElement(children: .combine)
    }

    private var charging: Bool { power.charging }
}
