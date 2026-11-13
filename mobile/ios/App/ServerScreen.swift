import SwiftUI
import UIKit

/// The dark screen a phone shows on a desk while it serves desktops: what it serves, and its power.
struct ServerScreen: View {
    let server: ServerModel
    let models: ModelsModel
    let devices: DevicesModel
    let settings: SettingsModel
    let close: () -> Void
    @State private var power = PowerModel()

    private var status: (text: String, color: Color) {
        if server.running { return ("Serving", .primary) }
        if server.busy { return ("Starting", .secondary) }
        if !server.notice.isEmpty { return ("Stopped", Color(red: 1, green: 0.612, blue: 0.561)) }
        return ("Off", .secondary)
    }

    private var speechModel: String {
        models.models.first { $0.id == server.selectedModel }?.name ?? "None"
    }

    private var cleanup: String {
        guard settings.cleanupEnabled else { return "Rules" }
        if settings.systemAvailable { return "System model" }
        return models.models.first { $0.kind == .cleanup && $0.id == models.selectedCleanupModel && $0.status == .ready }?.name ?? "Rules"
    }

    private var line: String {
        switch devices.clients.count {
        case 0: "No desktop is paired yet."
        case 1: "\(devices.clients[0].name) can use this \(deviceKind)."
        default: "\(devices.clients.count) desktops can use this \(deviceKind)."
        }
    }

    var body: some View {
        List {
            Section {
                VStack(spacing: 2) {
                    Image(systemName: "dot.radiowaves.left.and.right").font(.system(size: 44)).foregroundStyle(.secondary).accessibilityHidden(true)
                    Text(status.text).font(.title.bold()).foregroundStyle(status.color).padding(.top, 8)
                        .accessibilityAddTraits(.isHeader).accessibilityIdentifier("server.state")
                    Text(line).font(.subheadline).foregroundStyle(.secondary).multilineTextAlignment(.center)
                }
                .frame(maxWidth: .infinity)
                .listRowBackground(Color.clear)
                .listRowInsets(EdgeInsets(top: 24, leading: 0, bottom: 8, trailing: 0))
            }
            if !server.notice.isEmpty {
                Section { NoticeRow(text: server.notice).accessibilityIdentifier("server.notice") }
            }
            Section {
                LabeledContent("Speech model") { Text(speechModel).accessibilityIdentifier("server.selectedModel") }
                LabeledContent("Cleanup", value: cleanup)
                LabeledContent("Port") { Text(server.port.map(String.init) ?? (server.busy ? "Starting" : "None")).accessibilityIdentifier("server.port") }
                LabeledContent("Served", value: server.served == 1 ? "1 job" : "\(server.served) jobs")
            }
            if power.known {
                Section {
                    PowerRow(power: power)
                } footer: {
                    if let charging = power.charging {
                        Text(charging ? "This \(deviceKind) can keep serving." : "Server Mode keeps running on battery. Plug in for long sessions.")
                    }
                }
            }
        }
        .scrollContentBackground(.hidden)
        .background(Color.black.ignoresSafeArea())
        .safeAreaInset(edge: .bottom) {
            VStack(spacing: 12) {
                Text("Server mode stops when you leave Hushpen.")
                    .font(.footnote).foregroundStyle(.secondary).multilineTextAlignment(.center)
                Button(role: server.running || server.busy ? .destructive : nil) {
                    if server.running || server.busy { server.stop() }
                    close()
                } label: {
                    Text(server.running || server.busy ? "Stop Server Mode" : "Close").font(.headline).frame(maxWidth: .infinity, minHeight: 44)
                }
                .buttonStyle(.glass)
                .tint(server.running || server.busy ? .red : nil)
                .accessibilityIdentifier("server.stop")
            }
            .padding(.horizontal, 20).padding(.bottom, 8)
        }
        .nearbyPairing(devices)
        .preferredColorScheme(.dark)
        .task { await server.refresh(); await models.refresh(); await devices.refresh(); await settings.refresh() }
        .onAppear { power.start(); devices.watch() }
        .onDisappear { power.stop(); devices.unwatch() }
        .onReceive(NotificationCenter.default.publisher(for: UIDevice.batteryStateDidChangeNotification)) { _ in power.read() }
        .onReceive(NotificationCenter.default.publisher(for: UIDevice.batteryLevelDidChangeNotification)) { _ in power.read() }
    }
}
