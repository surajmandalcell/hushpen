import SwiftUI
import WebKit

struct SettingsScreen: View {
    let model: SettingsModel
    let onboarding: OnboardingModel
    let models: ModelsModel
    let devices: DevicesModel
    let history: HistoryModel
    @AppStorage("typingSpeed") private var typingSpeed = 40
    @AppStorage("keepHistoryDays") private var keepDays = 0
    @AppStorage(KeyboardSetting.leftHandedKey, store: KeyboardSetting.defaults) private var leftHanded = false
    @State private var clearing = false

    private var version: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
    }

    private var speechName: String {
        let speech = models.models.filter { $0.kind == .speech }
        return (speech.first { $0.id == models.selectedModel } ?? speech.first { $0.recommended })?.name ?? ""
    }

    var body: some View {
        List {
            if !model.notice.isEmpty {
                Section { NoticeRow(text: model.notice) }
            }
            Section {
                permission("Microphone", "mic", onboarding.microphone, granted: onboarding.microphoneGranted, id: "onboarding.microphone") {
                    if onboarding.microphone == "Not requested" { Task { await onboarding.requestMicrophone() } } else { onboarding.openSettings() }
                }
                permission("Keyboard", "keyboard", onboarding.keyboardAdded ? "On" : "Off", granted: onboarding.keyboardAdded, id: "onboarding.keyboard", onboarding.openSettings)
                permission("Full Access", "lock.open", fullAccess, granted: onboarding.fullAccess == true, id: "onboarding.fullAccess", onboarding.openSettings)
                Button { onboarding.openSettings() } label: {
                    Label("Open Settings", systemImage: "gearshape")
                }
                Toggle(isOn: $leftHanded) { RowTitle(title: "Left-handed keyboard", symbol: "hand.point.left") }
                    .accessibilityIdentifier("settings.leftHanded")
            } header: {
                Text("Keyboard")
            } footer: {
                Text("Allow Full Access to share dictation text with the app. The left-handed keyboard puts dictation on the left and Undo and Redo on the right.")
            }
            Section {
                NavigationLink(value: Route.models) {
                    Reading(title: "Models", symbol: "shippingbox", value: speechName)
                }
                .accessibilityIdentifier("settings.models")
                NavigationLink(value: Route.devices) {
                    Reading(title: "Devices", symbol: "laptopcomputer", value: devices.clients.isEmpty ? "" : "\(devices.clients.count)")
                }
                .accessibilityIdentifier("settings.devices")
            }
            Section {
                Toggle(isOn: Binding(get: { model.cleanupEnabled }, set: { enabled in Task { await model.setCleanup(enabled) } })) {
                    Label {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Use Text Cleanup")
                            if !model.systemAvailable {
                                Text("Uses your downloaded cleanup model when the system model is unavailable.")
                                    .font(.subheadline).foregroundStyle(.secondary)
                            }
                        }
                    } icon: {
                        Image(systemName: "sparkles").foregroundStyle(.tint)
                    }
                }
                .disabled(model.busy)
                .accessibilityIdentifier("settings.cleanup")
                Reading(title: "System Model", symbol: "cpu", value: model.systemAvailable ? "Available" : "Unavailable")
                    .accessibilityIdentifier("settings.systemModel")
            } header: {
                Text("Cleanup")
            } footer: {
                Text("The system model is first when available. Otherwise, Hushpen uses the selected downloaded model. If neither is ready, rules clean the text. Cleanup is off until you turn it on.")
            }
            Section {
                Picker(selection: $keepDays) {
                    Text("Forever").tag(0)
                    Text("1 Year").tag(365)
                    Text("3 Months").tag(90)
                    Text("1 Month").tag(30)
                } label: { RowTitle(title: "Keep History", symbol: "clock") }
                .accessibilityIdentifier("settings.keepHistory")
                Button("Clear History\u{2026}", role: .destructive) { clearing = true }
                    .accessibilityIdentifier("settings.clearHistory")
            } header: {
                Text("History")
            } footer: {
                Text("History stays on this \(deviceKind) and is never sent anywhere.")
            }
            Section {
                Stepper(value: $typingSpeed, in: 10...200, step: 5) {
                    LabeledContent {
                        Text("\(typingSpeed)").monospacedDigit()
                    } label: {
                        Label {
                            VStack(alignment: .leading, spacing: 2) {
                                Text("Typing speed")
                                Text("Words a minute, for time saved").font(.subheadline).foregroundStyle(.secondary)
                            }
                        } icon: {
                            Image(systemName: "timer").foregroundStyle(.tint)
                        }
                    }
                }
                .accessibilityIdentifier("settings.typingSpeed")
            } header: {
                Text("Stats")
            }
            Section {
                NavigationLink(value: Route.licenses) {
                    RowTitle(title: "Open-source licenses", symbol: "doc.text")
                }
                if !version.isEmpty { Reading(title: "Version", symbol: "info.circle", value: version) }
            } header: {
                Text("About")
            } footer: {
                Text("Hushpen is licensed under AGPL-3.0-or-later.")
            }
        }
        .navigationTitle("Settings")
        .task { onboarding.refresh(); await model.refresh(); await models.refresh(); await devices.refresh() }
        .onChange(of: keepDays) { _, days in Task { await history.prune(keepingDays: days) } }
        .confirmationDialog("Clear all history?", isPresented: $clearing, titleVisibility: .visible) {
            Button("Clear History", role: .destructive) { Task { await history.clear() } }
        } message: { Text("Every dictation is removed from this \(deviceKind). This cannot be undone.") }
    }

    private var fullAccess: String {
        switch onboarding.fullAccess {
        case true: "Allowed"
        case false: "Off"
        case nil: onboarding.keyboardAdded ? "Not seen yet" : "Off"
        }
    }

    /// A permission as a row: the state at the end, and a tap that asks or opens Settings while it is not granted.
    @ViewBuilder private func permission(_ title: String, _ symbol: String, _ value: String, granted: Bool, id: String, _ action: @escaping () -> Void) -> some View {
        if granted {
            LabeledContent {
                HStack { Text(value).foregroundStyle(.secondary); ReadyMark(label: "\(title) \(value)") }
            } label: { RowTitle(title: title, symbol: symbol) }
                .accessibilityIdentifier(id)
        } else {
            Button(action: action) {
                LabeledContent {
                    Text(value == "Not requested" ? "Allow" : value).foregroundStyle(.tint)
                } label: { RowTitle(title: title, symbol: symbol) }
            }
            .foregroundStyle(.primary)
            .accessibilityIdentifier(id)
        }
    }
}

struct LicensesScreen: View {
    var body: some View {
        Group {
            if let url = Bundle.main.url(forResource: "THIRD-PARTY-NOTICES", withExtension: "html") {
                LicenseNotice(url: url).ignoresSafeArea(edges: .bottom)
            } else {
                ContentUnavailableView("The license notice could not open.", systemImage: "doc.questionmark")
            }
        }
        .navigationTitle("Open-source licenses")
        .navigationBarTitleDisplayMode(.inline)
    }
}

private struct LicenseNotice: UIViewRepresentable {
    let url: URL

    func makeUIView(context: Context) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        configuration.defaultWebpagePreferences.allowsContentJavaScript = false
        let web = WKWebView(frame: .zero, configuration: configuration)
        web.navigationDelegate = context.coordinator
        // The notice declares light and dark colors; the page behind it follows the system.
        web.isOpaque = false
        web.backgroundColor = .systemBackground
        web.scrollView.backgroundColor = .systemBackground
        let rules = "[{\"trigger\":{\"url-filter\":\"^https?://\"},\"action\":{\"type\":\"block\"}}]"
        WKContentRuleListStore.default().compileContentRuleList(forIdentifier: "HushpenOfflineNotices", encodedContentRuleList: rules) { list, error in
            guard let list, error == nil, let page = Self.fitted(url) else {
                web.loadHTMLString("<p>The license notice could not open.</p>", baseURL: nil)
                return
            }
            configuration.userContentController.add(list)
            web.loadHTMLString(page, baseURL: url.deletingLastPathComponent())
        }
        return web
    }

    /// The notice has no viewport, so a phone would draw it at desktop width and shrink it.
    private static func fitted(_ url: URL) -> String? {
        guard let html = try? String(contentsOf: url, encoding: .utf8), let head = html.range(of: "<head>") else { return nil }
        let fit = """
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <style>
        body { font: 15px/1.45 -apple-system, system-ui; margin: 0; padding: 4px 20px 40px; overflow-wrap: anywhere; -webkit-text-size-adjust: 100%; }
        h1 { font-size: 24px; line-height: 1.2; } h2 { font-size: 19px; } h3 { font-size: 16px; }
        pre { font: 11px/1.4 ui-monospace, monospace; white-space: pre-wrap; }
        </style>
        """
        return html.replacingCharacters(in: head.upperBound..<head.upperBound, with: fit)
    }

    func updateUIView(_ uiView: WKWebView, context: Context) {}
    func makeCoordinator() -> Coordinator { Coordinator() }

    final class Coordinator: NSObject, WKNavigationDelegate {
        func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction) async -> WKNavigationActionPolicy {
            navigationAction.request.url?.isFileURL == true ? .allow : .cancel
        }
    }
}
