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
    @State private var forgetting: String?
    @State private var power = PowerModel()
    @State private var enteringLink = false
    @FocusState private var linkFocused: Bool

    private var server: ServerModel { model.server }

    var body: some View {
        List {
            let notices = [model.notice, server.notice].filter { !$0.isEmpty }
            if !notices.isEmpty {
                Section {
                    ForEach(notices, id: \.self) { NoticeRow(text: $0) }
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
            if enteringLink || !model.scannedOffer.isEmpty {
                Section {
                    HStack(spacing: 8) {
                        TextField("hushpen-pair://v1/...", text: $model.scannedOffer, axis: .vertical)
                            .lineLimit(1...4)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .keyboardType(.URL)
                            .focused($linkFocused)
                            .accessibilityIdentifier("devices.offer")
                        PasteButton(payloadType: String.self) { strings in
                            if let first = strings.first { model.scannedOffer = first }
                        }
                        .labelStyle(.iconOnly)
                        .buttonBorderShape(.capsule)
                    }
                    Button("Pair Desktop") { Task { await model.pair() } }
                        .disabled(model.busy || model.scannedOffer.isEmpty || !server.running)
                        .accessibilityIdentifier("devices.pair")
                } header: {
                    Text("Pair a Desktop")
                } footer: {
                    Text(server.running
                        ? "Copy the link on your computer: Devices, Add device, Phone."
                        : "Turn on Server Mode, then pair.")
                }
            }
            Section("Paired Desktops") {
                if model.clients.isEmpty {
                    Label {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("No paired desktops")
                            Text(server.running
                                ? "Choose Pair a Desktop, then scan the code on your computer."
                                : "Turn on Server Mode, then scan the code on your computer.")
                                .font(.subheadline).foregroundStyle(.secondary)
                        }
                    } icon: {
                        Image(systemName: "laptopcomputer").foregroundStyle(.tint)
                    }
                }
                ForEach(model.clients, id: \.self) { id in
                    Reading(title: desktopName(id), symbol: "laptopcomputer", value: "Paired")
                        .swipeActions(edge: .trailing) {
                            Button("Forget", role: .destructive) { forgetting = id }
                                .disabled(model.busy)
                        }
                }
            }
            Section {
                Button { model.scannerOpen = true } label: {
                    Label("Pair a Desktop", systemImage: "qrcode.viewfinder")
                }
                .accessibilityIdentifier("devices.scan")
            }
        }
        .navigationTitle("Devices")
        .toolbarVisibility(.hidden, for: .tabBar)
        .refreshable { await model.refresh() }
        .task { await model.refresh() }
        .onAppear { power.start() }
        .onDisappear { power.stop() }
        .onReceive(NotificationCenter.default.publisher(for: UIDevice.batteryStateDidChangeNotification)) { _ in power.read() }
        .onReceive(NotificationCenter.default.publisher(for: UIDevice.batteryLevelDidChangeNotification)) { _ in power.read() }
        .sheet(isPresented: $model.scannerOpen) {
            ScannerSheet(
                scanned: { value in
                    model.scannedOffer = value
                    model.scannerOpen = false
                },
                pasteLink: {
                    model.scannerOpen = false
                    enteringLink = true
                }
            )
        }
        .onChange(of: enteringLink) { _, entering in if entering { linkFocused = true } }
        .confirmationDialog(
            forgetting.map { "Forget \(desktopName($0))?" } ?? "",
            isPresented: Binding(get: { forgetting != nil }, set: { if !$0 { forgetting = nil } }),
            titleVisibility: .visible,
            presenting: forgetting
        ) { id in
            Button("Forget", role: .destructive) { Task { await model.forget(id) } }
        } message: { _ in
            Text("This desktop can no longer use this iPhone's models. You can pair it again later.")
        }
    }
}
