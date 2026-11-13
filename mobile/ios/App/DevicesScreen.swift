import SwiftUI
import UIKit

/// The Power row of Devices and Server mode: a title, "Charging" or "On battery", then the battery icon and percent.
struct PowerRow: View {
    let power: PowerModel

    var body: some View {
        LabeledContent {
            BatteryReading(power: power)
        } label: {
            Label {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Power")
                    Text(power.status).font(.subheadline).foregroundStyle(.secondary)
                }
            } icon: {
                Image(systemName: "bolt.fill").foregroundStyle(.tint)
            }
        }
        .accessibilityIdentifier("power.row")
    }
}

struct DevicesScreen: View {
    @Bindable var model: DevicesModel
    let serve: @MainActor @Sendable (Bool) -> Void
    let showServer: () -> Void
    @State private var forgetting: PairedDesktop?
    @State private var power = PowerModel()

    private var server: ServerModel { model.server }

    var body: some View {
        List {
            let notices = [model.notice, server.notice].filter { !$0.isEmpty }
            if !notices.isEmpty {
                Section {
                    ForEach(notices, id: \.self) { NoticeRow(text: $0, ok: $0.hasSuffix(" paired.")) }
                        .accessibilityIdentifier("devices.notice")
                }
            }
            Section {
                Toggle(isOn: Binding(get: { server.running || server.busy }, set: serve)) {
                    Label {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Server Mode")
                            Text(server.running ? "Serving on port \(server.port.map(String.init) ?? "")" : server.busy ? "Starting" : "Off")
                                .font(.subheadline).foregroundStyle(.secondary)
                        }
                    } icon: {
                        Image(systemName: "dot.radiowaves.left.and.right").foregroundStyle(.tint)
                    }
                }
                .accessibilityIdentifier("devices.server")
                PowerRow(power: power)
                if server.running {
                    Button(action: showServer) { Label("Show Server Screen", systemImage: "moon.stars") }
                        .accessibilityIdentifier("devices.screen")
                }
            } footer: {
                Text("Your computer can use this iPhone\u{2019}s models while Hushpen is open. Server mode stops when you leave the app.")
            }
            Section("Paired Desktops") {
                if model.clients.isEmpty {
                    Label {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("No paired desktops")
                            Text(server.running
                                ? "On your computer, open Add a Device and pick this iPhone."
                                : "Turn on Server Mode, then pair from your computer.")
                                .font(.subheadline).foregroundStyle(.secondary)
                        }
                    } icon: {
                        Image(systemName: "laptopcomputer").foregroundStyle(.tint)
                    }
                }
                ForEach(model.clients, id: \.id) { desktop in
                    Reading(title: desktop.name, symbol: "laptopcomputer", value: "Paired")
                        .swipeActions(edge: .trailing) {
                            Button("Forget", role: .destructive) { forgetting = desktop }
                                .disabled(model.busy)
                        }
                }
            }
            Section {
                Button { model.scannerOpen = true } label: {
                    Label("Pair a Desktop", systemImage: "qrcode.viewfinder")
                }
                .disabled(!server.running)
                .accessibilityIdentifier("devices.scan")
            } footer: {
                Text(server.running ? "Not listed on your computer? Scan its code or type the 8-digit code." : "Turn on Server Mode to pair.")
            }
        }
        .navigationTitle("Devices")
        .toolbarVisibility(.hidden, for: .tabBar)
        .refreshable { await model.refresh() }
        .task { await model.refresh() }
        .onAppear { power.start(); model.watch() }
        .onDisappear { power.stop(); model.unwatch() }
        .onReceive(NotificationCenter.default.publisher(for: UIDevice.batteryStateDidChangeNotification)) { _ in power.read() }
        .onReceive(NotificationCenter.default.publisher(for: UIDevice.batteryLevelDidChangeNotification)) { _ in power.read() }
        .sheet(isPresented: $model.scannerOpen) { PairSheet(model: model) }
        .confirmationDialog(
            forgetting.map { "Forget \($0.name)?" } ?? "",
            isPresented: Binding(get: { forgetting != nil }, set: { if !$0 { forgetting = nil } }),
            titleVisibility: .visible,
            presenting: forgetting
        ) { desktop in
            Button("Forget", role: .destructive) { Task { await model.forget(desktop.id) } }
        } message: { _ in
            Text("This desktop can no longer use this iPhone's models. You can pair it again later.")
        }
    }
}
