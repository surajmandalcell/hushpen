import SwiftUI

enum Route: Hashable {
    case models, devices, settings, licenses
}

/// The one list the app opens on: setup status, Server mode, then the other screens.
struct HomeScreen: View {
    let onboarding: OnboardingModel
    let models: ModelsModel
    let devices: DevicesModel
    let server: ServerModel
    let serve: @MainActor @Sendable (Bool) -> Void

    private var speech: [Model] { models.models.filter { $0.kind == .speech } }
    private var current: Model? { speech.first { $0.id == models.selectedModel } ?? speech.first { $0.recommended } }

    var body: some View {
        List {
            let notices = [models.notice, devices.notice, server.notice].filter { !$0.isEmpty }
            if !notices.isEmpty {
                Section { ForEach(notices, id: \.self) { NoticeRow(text: $0) } }
            }
            Section {
                keyboard
                microphone
                speechModel
            } header: {
                Text("Setup")
            } footer: {
                if !onboarding.keyboardUsed {
                    Text("To add the keyboard, open Settings, then General, Keyboard, Keyboards, Add New Keyboard, and choose Hushpen. Turn on Allow Full Access so dictation can return text.")
                }
            }
            Section {
                Toggle(isOn: Binding(get: { server.running || server.busy }, set: serve)) {
                    RowTitle(title: "Server Mode", symbol: "dot.radiowaves.left.and.right")
                }
                .accessibilityIdentifier("server.toggle")
            } footer: {
                Text(server.running
                    ? "Serving on port \(server.port.map(String.init) ?? ""). Keep Hushpen open and plugged in."
                    : "Keep Hushpen open and plugged in. Your computer can then use this iPhone's models.")
            }
            Section {
                NavigationLink(value: Route.models) {
                    Reading(title: "Models", symbol: "shippingbox", value: modelsSummary)
                }
                .accessibilityIdentifier("home.models")
                NavigationLink(value: Route.devices) {
                    Reading(title: "Devices", symbol: "laptopcomputer", value: devices.clients.isEmpty ? "" : "\(devices.clients.count)")
                }
                .accessibilityIdentifier("home.devices")
                NavigationLink(value: Route.settings) {
                    RowTitle(title: "Settings", symbol: "gearshape")
                }
                .accessibilityIdentifier("home.settings")
            }
        }
        .navigationTitle("Hushpen")
        .task {
            onboarding.refresh()
            await models.refresh()
            await devices.refresh()
        }
    }

    private var modelsSummary: String {
        speech.isEmpty ? "" : speech.count == 1 ? "1 speech model" : "\(speech.count) speech models"
    }

    @ViewBuilder private var keyboard: some View {
        let used = onboarding.keyboardUsed
        if used {
            LabeledContent {
                HStack { Text("Used with Full Access").foregroundStyle(.secondary); ReadyMark() }
            } label: { RowTitle(title: "Keyboard", symbol: "keyboard") }
        } else {
            Button(action: onboarding.openSettings) {
                LabeledContent {
                    Text("Set Up").foregroundStyle(.tint)
                } label: { RowTitle(title: "Keyboard", symbol: "keyboard") }
            }
            .foregroundStyle(.primary)
        }
    }

    @ViewBuilder private var microphone: some View {
        if onboarding.microphoneGranted {
            LabeledContent {
                HStack { Text("Allowed").foregroundStyle(.secondary); ReadyMark() }
            } label: { RowTitle(title: "Microphone", symbol: "mic") }
                .accessibilityIdentifier("onboarding.microphone")
        } else {
            let asking = onboarding.microphone == "Not requested"
            Button {
                if asking { Task { await onboarding.requestMicrophone() } } else { onboarding.openSettings() }
            } label: {
                LabeledContent {
                    Text(asking ? "Allow" : "Open Settings").foregroundStyle(.tint)
                } label: { RowTitle(title: "Microphone", symbol: "mic") }
            }
            .foregroundStyle(.primary)
            .accessibilityIdentifier("onboarding.microphone")
        }
    }

    @ViewBuilder private var speechModel: some View {
        NavigationLink(value: Route.models) {
            HStack {
                Label {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Speech Model")
                        Text(speechLine).font(.subheadline).foregroundStyle(.secondary)
                    }
                } icon: {
                    Image(systemName: "waveform").foregroundStyle(.tint)
                }
                if current?.status == .ready && models.downloading != current?.id {
                    Spacer(minLength: 0)
                    ReadyMark()
                }
            }
        }
        .accessibilityIdentifier("home.speech")
    }

    private var speechLine: String {
        guard let current else { return "Loading models" }
        if models.downloading == current.id {
            return "\(current.name) \u{B7} " + (models.verifying ? "Checking" : "Downloading \(percent)%")
        }
        switch current.status {
        case .ready: return "\(current.name) \u{B7} Ready"
        case .needsVerification: return "\(current.name) \u{B7} Not checked"
        case .damaged: return "\(current.name) \u{B7} Damaged"
        default: return "\(current.name) \u{B7} Not downloaded"
        }
    }

    private var percent: Int {
        models.total > 0 ? Int(Double(models.progress) / Double(models.total) * 100) : 0
    }
}
