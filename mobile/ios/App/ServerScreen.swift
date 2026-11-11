import SwiftUI
import UIKit

/// The dim, always-black screen a phone shows on a desk while it serves desktops.
struct ServerScreen: View {
    let server: ServerModel
    let models: ModelsModel
    let devices: DevicesModel
    let settings: SettingsModel
    let close: () -> Void
    @State private var plugged = false

    private static let card = Color(red: 0.078, green: 0.078, blue: 0.086)
    private static let label = Color.white.opacity(0.48)
    private static let value = Color.white.opacity(0.8)
    private static let serving = Color(red: 0.188, green: 0.82, blue: 0.345)
    private static let charge = Color(red: 1, green: 0.702, blue: 0.251)
    private static let problem = Color(red: 1, green: 0.612, blue: 0.561)

    private var status: (text: String, color: Color) {
        if server.running { return ("Serving", Self.serving) }
        if server.busy { return ("Starting", Self.label) }
        if !server.notice.isEmpty { return ("Stopped", Self.problem) }
        return ("Off", Self.label)
    }

    private var speechModel: String {
        models.models.first { $0.id == server.selectedModel }?.name ?? "None"
    }

    private var cleanup: String {
        settings.cleanupEnabled && settings.systemAvailable ? "System model when open" : "Rules"
    }

    private var desktops: String {
        devices.clients.isEmpty ? "None yet" : devices.clients.map(desktopName).joined(separator: ", ")
    }

    var body: some View {
        GeometryReader { proxy in
            ScrollView {
                VStack(spacing: 0) {
                    Color.clear.frame(height: proxy.size.height * 0.1)
                    card
                    Spacer(minLength: 28)
                    Text("Server mode stops when you leave Hushpen.")
                        .font(.subheadline)
                        .foregroundStyle(.white.opacity(0.4))
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, 12)
                    Button(server.running || server.busy ? "Stop Server Mode" : "Close") {
                        if server.running || server.busy { server.stop() }
                        close()
                    }
                    .buttonStyle(.glass)
                    .controlSize(.extraLarge)
                    .foregroundStyle(.white.opacity(0.85))
                    .padding(.vertical, 28)
                    .accessibilityIdentifier("server.stop")
                }
                .frame(maxWidth: 520, minHeight: proxy.size.height)
                .frame(maxWidth: .infinity)
                .padding(.horizontal, 18)
            }
            .scrollBounceBehavior(.basedOnSize)
        }
        .background(Color.black.ignoresSafeArea())
        .preferredColorScheme(.dark)
        .task { await server.refresh(); await models.refresh(); await devices.refresh(); await settings.refresh() }
        .onAppear {
            UIDevice.current.isBatteryMonitoringEnabled = true
            plugged = Self.isPlugged
        }
        .onDisappear { UIDevice.current.isBatteryMonitoringEnabled = false }
        .onReceive(NotificationCenter.default.publisher(for: UIDevice.batteryStateDidChangeNotification)) { _ in
            plugged = Self.isPlugged
        }
    }

    private static var isPlugged: Bool {
        let state = UIDevice.current.batteryState
        return state == .charging || state == .full
    }

    private var card: some View {
        VStack(spacing: 0) {
            HStack(spacing: 7) {
                Text("Server mode")
                    .font(.title3.weight(.semibold))
                    .foregroundStyle(.white.opacity(0.82))
                    .accessibilityAddTraits(.isHeader)
                Spacer(minLength: 8)
                Circle().fill(status.color).frame(width: 8, height: 8).accessibilityHidden(true)
                Text(status.text)
                    .font(.subheadline.weight(.medium))
                    .foregroundStyle(status.color)
                    .accessibilityIdentifier("server.state")
            }
            .padding(.horizontal, 20)
            .padding(.top, 20)
            .padding(.bottom, 14)
            stat("Port", server.port.map(String.init) ?? (server.busy ? "Starting" : "None"), id: "server.port")
            stat("Speech model", speechModel, id: "server.selectedModel")
            stat("Cleanup", cleanup)
            stat("Desktops", desktops)
            stat("Unload", "While Server mode is open")
            if !server.notice.isEmpty {
                note("exclamationmark.circle.fill", server.notice, color: Self.problem)
                    .accessibilityIdentifier("server.notice")
            }
            if plugged {
                note("bolt.fill", "Charging", color: Self.serving.opacity(0.8))
            } else {
                note("bolt.fill", "Plug in to keep serving", color: Self.charge)
            }
        }
        .background(Self.card, in: RoundedRectangle(cornerRadius: 28, style: .continuous))
    }

    private func stat(_ title: String, _ value: String, id: String? = nil) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 12) {
            Text(title).foregroundStyle(Self.label)
            Spacer(minLength: 8)
            Text(value)
                .foregroundStyle(Self.value)
                .multilineTextAlignment(.trailing)
                .lineLimit(2)
                .accessibilityIdentifier(id ?? "")
        }
        .font(.subheadline)
        .padding(.horizontal, 20)
        .frame(minHeight: 50)
        .overlay(alignment: .top) { Rectangle().fill(.white.opacity(0.07)).frame(height: 1) }
    }

    private func note(_ symbol: String, _ text: String, color: Color) -> some View {
        Label(text, systemImage: symbol)
            .font(.subheadline)
            .foregroundStyle(color)
            .labelStyle(NoteLabelStyle())
            .padding(.horizontal, 20)
            .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
            .overlay(alignment: .top) { Rectangle().fill(.white.opacity(0.07)).frame(height: 1) }
    }
}

private struct NoteLabelStyle: LabelStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            configuration.icon.font(.footnote)
            configuration.title
        }
    }
}
