import SwiftUI
import WebKit

struct SettingsScreen: View {
    let model: SettingsModel
    let onboarding: OnboardingModel

    private var version: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
    }

    var body: some View {
        List {
            if !model.notice.isEmpty {
                Section { NoticeRow(text: model.notice) }
            }
            Section {
                Toggle(isOn: Binding(get: { model.cleanupEnabled }, set: { enabled in Task { await model.setCleanup(enabled) } })) {
                    Label {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Use System Cleanup")
                            if !model.systemAvailable {
                                Text("No system model on this iPhone. Rule cleanup still works.")
                                    .font(.subheadline).foregroundStyle(.secondary)
                            }
                        }
                    } icon: {
                        Image(systemName: "sparkles").foregroundStyle(.tint)
                    }
                }
                .accessibilityIdentifier("settings.cleanup")
                Reading(title: "System Model", symbol: "cpu", value: model.systemAvailable ? "Available" : "Unavailable")
                    .accessibilityIdentifier("settings.systemModel")
            } header: {
                Text("Cleanup")
            } footer: {
                Text("The system model works only while Hushpen is open. At other times, rules clean the text. Cleanup is off until you turn it on.")
            }
            Section {
                Reading(title: "Microphone", symbol: "mic", value: onboarding.microphone)
                Reading(title: "Keyboard", symbol: "keyboard", value: onboarding.keyboardStatus)
                Button { onboarding.openSettings() } label: {
                    Label("Open Settings", systemImage: "gearshape")
                }
            } header: {
                Text("Keyboard")
            } footer: {
                Text("In Settings, open General, Keyboard, Keyboards, Add New Keyboard, then choose Hushpen. Allow Full Access to share dictation text with the app. Then switch to Hushpen with the globe key.")
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
        .task { onboarding.refresh(); await model.refresh() }
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
