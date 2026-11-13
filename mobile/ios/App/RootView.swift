import SwiftUI
import UIKit

struct RootView: View {
    enum Tab: Hashable { case home, history, stats, settings }

    let runtime: PhoneRuntime
    let incomingURL: URL?
    @Environment(\.scenePhase) private var phase
    @AppStorage("keepHistoryDays") private var keepDays = 0
    @State private var activeScreen: UIScreen?
    @State private var tab = Tab.home
    @State private var dictationOpen = false
    @State private var serverOpen = false

    var body: some View {
        TabView(selection: $tab) {
            SwiftUI.Tab("Home", systemImage: "house", value: Tab.home) {
                NavigationStack {
                    HomeScreen(
                        onboarding: runtime.onboarding,
                        models: runtime.models,
                        devices: runtime.devices,
                        server: runtime.server,
                        history: runtime.history,
                        showStats: { tab = .stats },
                        showHistory: { tab = .history }
                    )
                    .routes(runtime, serve: { serve($0) }, showServer: { serverOpen = true })
                }
            }
            SwiftUI.Tab("History", systemImage: "clock", value: Tab.history) {
                NavigationStack {
                    HistoryScreen(history: runtime.history, models: runtime.models)
                }
            }
            SwiftUI.Tab("Stats", systemImage: "chart.bar", value: Tab.stats) {
                NavigationStack { StatsScreen(history: runtime.history) }
            }
            SwiftUI.Tab("Settings", systemImage: "gearshape", value: Tab.settings) {
                NavigationStack {
                    SettingsScreen(model: runtime.settings, onboarding: runtime.onboarding, models: runtime.models, devices: runtime.devices, history: runtime.history)
                        .routes(runtime, serve: { serve($0) }, showServer: { serverOpen = true })
                }
            }
        }
        .background(SceneScreenReader { value in activeScreen = value; updateScene() })
        .onAppear { runtime.server.setOpen(true); updateScene() }
        .onChange(of: phase) { _, _ in updateScene() }
        .task(id: incomingURL) {
            guard let url = incomingURL else { return }
            runtime.server.stop()
            runtime.devices.scannerOpen = false
            serverOpen = false
            dictationOpen = true
            await runtime.dictation.open(url: url)
        }
        .fullScreenCover(isPresented: $serverOpen) {
            ServerScreen(
                server: runtime.server,
                models: runtime.models,
                devices: runtime.devices,
                settings: runtime.settings,
                close: { serverOpen = false }
            )
        }
        .fullScreenCover(isPresented: $dictationOpen, onDismiss: { runtime.dictation.abort() }) {
            DictationScreen(model: runtime.dictation, close: { dictationOpen = false })
        }
    }

    private func serve(_ on: Bool) {
        if on {
            runtime.server.start()
        } else {
            runtime.server.stop()
            serverOpen = false
        }
    }

    private func updateScene() {
        runtime.server.setActive(phase == .active, screen: activeScreen)
        if phase == .active {
            runtime.onboarding.refresh()
            Task { await runtime.history.prune(keepingDays: keepDays) }
        }
        runtime.dictation.setActive(phase == .active)
        Task { await runtime.settings.refresh() }
    }
}

private extension View {
    /// The screens a stack can push: models, devices, licenses, and a dictation's detail.
    func routes(_ runtime: PhoneRuntime, serve: @escaping @MainActor @Sendable (Bool) -> Void, showServer: @escaping () -> Void) -> some View {
        navigationDestination(for: Route.self) { route in
            switch route {
            case .models: ModelsScreen(model: runtime.models, settings: runtime.settings)
            case .devices: DevicesScreen(model: runtime.devices, serve: serve, showServer: showServer)
            case .licenses: LicensesScreen()
            }
        }
        .navigationDestination(for: HistoryEntry.self) { HistoryDetail(entry: $0, history: runtime.history, models: runtime.models) }
    }
}

/// The opening page, shown while the phone core loads or when it could not open.
struct LaunchView: View {
    let loading: Bool
    let notice: String
    let retry: () -> Void

    var body: some View {
        VStack(spacing: 20) {
            Image(systemName: "waveform").font(.system(size: 56, weight: .medium)).foregroundStyle(.tint).accessibilityHidden(true)
            Text("Hushpen").font(.largeTitle.bold())
            if loading {
                ProgressView()
                    .accessibilityLabel("Opening Hushpen")
            } else {
                if !notice.isEmpty {
                    Text(notice)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                        .accessibilityIdentifier("app.notice")
                }
                Button("Try Again", action: retry)
                    .buttonStyle(.glassProminent)
                    .controlSize(.large)
            }
        }
        .padding(32)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

private struct SceneScreenReader: UIViewRepresentable {
    let changed: (UIScreen?) -> Void
    func makeUIView(context: Context) -> SceneScreenView {
        let view = SceneScreenView()
        view.changed = changed
        return view
    }
    func updateUIView(_ uiView: SceneScreenView, context: Context) {}
}

private final class SceneScreenView: UIView {
    var changed: ((UIScreen?) -> Void)?
    override func didMoveToWindow() {
        super.didMoveToWindow()
        let screen = window?.windowScene?.screen
        DispatchQueue.main.async { [weak self] in self?.changed?(screen) }
    }
}
