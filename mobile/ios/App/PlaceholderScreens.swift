import SwiftUI
import WebKit

struct PlaceholderOnboarding: View {
    let onboarding: OnboardingModel
    let models: ModelsModel

    var body: some View {
        List {
            Text("Microphone: \(onboarding.microphone)").accessibilityIdentifier("onboarding.microphone")
            Button("Allow microphone") { Task { await onboarding.requestMicrophone() } }
            Button("Open Settings") { onboarding.openSettings() }
            Text("Keyboard used: \(onboarding.keyboardUsed ? "Yes" : "No")")
            Text("In Settings, open General, Keyboard, Keyboards, Add New Keyboard, then choose Hushpen.")
            Text("Allow Full Access to share dictation text with the app. Then select Hushpen with the globe key.")
            Text("Speech model ready: \(models.speechReady ? "Yes" : "No")")
            NavigationLink("Choose a speech model") { PlaceholderModels(model: models) }
            Text(onboarding.microphoneGranted && onboarding.keyboardUsed && models.speechReady ? "Ready to dictate from the keyboard." : "Allow the microphone, use the keyboard, and download a speech model.")
        }
        .navigationTitle("Onboarding")
        .task { onboarding.refresh(); await models.refresh() }
    }
}

struct PlaceholderModels: View {
    let model: ModelsModel

    var body: some View {
        List {
            Text("Selected: \(model.selectedModel)").accessibilityIdentifier("models.selected")
            if model.models.isEmpty { Text("No models are available.") }
            ForEach(model.models, id: \.id) { item in
                Section(item.name) {
                    Text("Status: \(String(describing: item.status))")
                    Text("Bytes: \(item.bytes)")
                    if item.kind == .speech {
                        Button("Download \(item.name)") { Task { await model.download(item.id) } }
                            .disabled(model.downloading != nil)
                        Button("Verify \(item.name)") { Task { await model.verify(item.id) } }
                            .disabled(model.busy || model.downloading != nil)
                        Button("Use \(item.name)") { Task { await model.select(item.id) } }
                            .disabled(item.status != .ready || model.busy || model.downloading != nil)
                    }
                }
            }
            if let downloading = model.downloading {
                Text("Downloading: \(downloading)")
                Text(model.verifying ? "Verifying" : "\(model.progress) of \(model.total) bytes")
                Button("Cancel download") { model.cancelDownload() }
            }
            if !model.notice.isEmpty { Text(model.notice).accessibilityIdentifier("models.notice") }
            Button("Refresh") { Task { await model.refresh() } }
        }
        .navigationTitle("Models")
        .task { await model.refresh() }
    }
}

struct PlaceholderServer: View {
    let model: ServerModel

    var body: some View {
        List {
            Text(model.running ? "Running" : model.busy ? "Starting" : "Stopped").accessibilityIdentifier("server.state")
            Text(model.port.map(String.init) ?? "None").accessibilityIdentifier("server.port")
            Text("Selected speech model: \(model.selectedModel)").accessibilityIdentifier("server.selectedModel")
            Text("Plug in to keep serving")
            Text("Keep Hushpen in front. Server mode stops when you leave the app.")
            Text("Unload rule: While Server mode is open")
            Toggle("Server mode", isOn: Binding(get: { model.running || model.busy }, set: { $0 ? model.start() : model.stop() }))
                .accessibilityIdentifier("server.toggle")
            if !model.notice.isEmpty { Text(model.notice).accessibilityIdentifier("server.notice") }
        }
        .navigationTitle("Server mode")
        .task { await model.refresh() }
    }
}

struct PlaceholderDevices: View {
    @Bindable var model: DevicesModel
    let openServer: () -> Void

    var body: some View {
        List {
            Text("Server mode: \(model.server.running ? "Running" : "Stopped")")
            Text("Start Server mode before pairing a desktop.")
            Button("Open Server mode", action: openServer)
            if model.clients.isEmpty { Text("No desktops are paired.") }
            ForEach(model.clients, id: \.self) { client in
                Section(client) {
                    Button("Forget desktop") { Task { await model.forget(client) } }.disabled(model.busy)
                }
            }
            Button("Scan desktop QR") { model.scannerOpen = true }
                .accessibilityIdentifier("devices.scan")
            TextField("Desktop pairing code", text: $model.scannedOffer)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .accessibilityIdentifier("devices.offer")
            Button("Pair desktop") { Task { await model.pair() } }
                .disabled(model.busy || model.scannedOffer.isEmpty || !model.server.running)
                .accessibilityIdentifier("devices.pair")
            if !model.notice.isEmpty { Text(model.notice).accessibilityIdentifier("devices.notice") }
            Button("Refresh") { Task { await model.refresh() } }
        }
        .navigationTitle("Devices")
        .sheet(isPresented: $model.scannerOpen) {
            PlaceholderScanner { value in
                model.scannedOffer = value
                model.scannerOpen = false
            }
        }
        .task { await model.refresh() }
    }
}

struct PlaceholderScanner: View {
    @State private var model = ScannerModel()
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var phase
    let scanned: (String) -> Void

    var body: some View {
        VStack {
            Text("Camera: \(model.permission)")
            CameraPreview(session: model.session)
            Text(model.starting ? "Starting" : model.scanning ? "Scanning" : "Stopped").accessibilityIdentifier("scanner.state")
            if !model.notice.isEmpty { Text(model.notice) }
            Button("Open Settings") { model.openSettings() }
            Button("Try again") { Task { await model.start() } }.disabled(model.starting || model.scanning)
            Button("Cancel") { model.stop(); dismiss() }
        }
        .task { await model.start() }
        .onChange(of: model.value) { _, value in if !value.isEmpty { scanned(value) } }
        .onDisappear { model.stop() }
        .onChange(of: phase) { _, value in if value == .background { model.stop() } }
    }
}

struct PlaceholderSettings: View {
    let model: SettingsModel
    let onboarding: OnboardingModel

    var body: some View {
        List {
            Text("Microphone: \(onboarding.microphone)")
            Text("Keyboard used: \(onboarding.keyboardUsed ? "Yes" : "No")")
            Button("Open Settings") { onboarding.openSettings() }
            Text("System model: \(model.systemAvailable ? "Available" : "Unavailable")")
                .accessibilityIdentifier("settings.systemModel")
            Toggle("System cleanup", isOn: Binding(get: { model.cleanupEnabled }, set: { enabled in Task { await model.setCleanup(enabled) } }))
                .accessibilityIdentifier("settings.cleanup")
            Text("Cleanup uses the on-device Apple system model when it is available. Otherwise, Hushpen keeps rule-cleaned text.")
            NavigationLink("Open-source licenses") { PlaceholderLicenses() }
            if !model.notice.isEmpty { Text(model.notice) }
        }
        .navigationTitle("Settings")
        .task { onboarding.refresh(); await model.refresh() }
    }
}

struct PlaceholderDictation: View {
    let model: DictationModel

    var body: some View {
        List {
            Text(model.state).accessibilityIdentifier("dictation.state")
            Text(model.sessionID).accessibilityIdentifier("dictation.session")
            Text("The microphone records in Hushpen. Return to the previous app after the text is ready.")
            Button("Finish") { model.finish() }.disabled(model.state != "listening")
                .accessibilityIdentifier("dictation.finish")
            Button("Cancel") { model.cancel() }.disabled(!model.busy)
                .accessibilityIdentifier("dictation.cancel")
            Button("Recover") { model.recover() }.disabled(!model.recoverable)
                .accessibilityIdentifier("dictation.recover")
            Text(model.result).accessibilityIdentifier("dictation.result")
            if !model.notice.isEmpty { Text(model.notice).accessibilityIdentifier("dictation.notice") }
        }
        .navigationTitle("Dictation")
    }
}

struct PlaceholderLicenses: View {
    var body: some View {
        Group {
            if let url = Bundle.main.url(forResource: "THIRD-PARTY-NOTICES", withExtension: "html") {
                LicenseNotice(url: url)
            } else {
                Text("The license notice could not open.")
            }
        }
        .navigationTitle("Open-source licenses")
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
        let rules = "[{\"trigger\":{\"url-filter\":\"^https?://\"},\"action\":{\"type\":\"block\"}}]"
        WKContentRuleListStore.default().compileContentRuleList(forIdentifier: "HushpenOfflineNotices", encodedContentRuleList: rules) { list, error in
            guard let list, error == nil else {
                web.loadHTMLString("<p>The license notice could not open.</p>", baseURL: nil)
                return
            }
            configuration.userContentController.add(list)
            web.loadFileURL(url, allowingReadAccessTo: url)
        }
        return web
    }
    func updateUIView(_ uiView: WKWebView, context: Context) {}
    func makeCoordinator() -> Coordinator { Coordinator() }

    final class Coordinator: NSObject, WKNavigationDelegate {
        func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction) async -> WKNavigationActionPolicy {
            navigationAction.request.url?.isFileURL == true ? .allow : .cancel
        }
    }
}
