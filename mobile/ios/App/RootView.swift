import SwiftUI
import UIKit

struct RootView: View {
    let runtime: PhoneRuntime
    let incomingURL: URL?
    @Environment(\.scenePhase) private var phase
    @State private var activeScreen: UIScreen?
    @State private var dictationOpen = false
    @State private var serverOpen = false

    var body: some View {
        NavigationStack {
            HomeScreen(
                onboarding: runtime.onboarding,
                models: runtime.models,
                devices: runtime.devices,
                server: runtime.server,
                serve: { serve($0, showCard: true) }
            )
            .navigationDestination(for: Route.self) { route in
                switch route {
                case .models: ModelsScreen(model: runtime.models, settings: runtime.settings)
                case .devices: DevicesScreen(model: runtime.devices, serve: { serve($0, showCard: false) })
                case .settings: SettingsScreen(model: runtime.settings, onboarding: runtime.onboarding)
                case .licenses: LicensesScreen()
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

    private func serve(_ on: Bool, showCard: Bool) {
        if on {
            serverOpen = showCard
            runtime.server.start()
        } else {
            runtime.server.stop()
            serverOpen = false
        }
    }

    private func updateScene() {
        runtime.server.setActive(phase == .active, screen: activeScreen)
        runtime.onboarding.refresh()
        runtime.dictation.setActive(phase == .active)
        Task { await runtime.settings.refresh() }
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
