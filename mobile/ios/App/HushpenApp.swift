import Observation
import SwiftUI
import UIKit

@Observable @MainActor
final class PhoneAppModel {
    private(set) var runtime: PhoneRuntime?
    private(set) var loading = false
    var notice = ""
    var incomingURL: URL?

    func load() async {
        guard runtime == nil, !loading else { return }
        loading = true
        notice = ""
        defer { loading = false }
        do { runtime = try PhoneRuntime(repository: await PhoneRepository.open()) }
        catch { notice = phoneError(error) }
    }
}

@MainActor
final class PhoneRuntime {
    let repository: PhoneRepository
    let onboarding = OnboardingModel()
    let models: ModelsModel
    let settings: SettingsModel
    let server: ServerModel
    let devices: DevicesModel
    let dictation: DictationModel

    init(repository: PhoneRepository) throws {
        self.repository = repository
        models = ModelsModel(repository: repository)
        settings = SettingsModel(repository: repository)
        server = ServerModel(repository: repository)
        devices = DevicesModel(repository: repository, server: server)
        dictation = try DictationModel(repository: repository, store: FlowSessionStore())
    }
}

@main
struct HushpenApp: App {
    @State private var app = PhoneAppModel()

    var body: some Scene {
        WindowGroup {
            Group {
                if let runtime = app.runtime {
                    PlaceholderApp(runtime: runtime, incomingURL: app.incomingURL)
                } else {
                    List {
                        Text(app.loading ? "Opening Hushpen" : "Hushpen")
                        if !app.notice.isEmpty { Text(app.notice).accessibilityIdentifier("app.notice") }
                        Button("Try again") { Task { await app.load() } }.disabled(app.loading)
                    }
                    .task { await app.load() }
                }
            }
            .onOpenURL { app.incomingURL = $0 }
        }
    }
}

struct PlaceholderApp: View {
    let runtime: PhoneRuntime
    let incomingURL: URL?
    @Environment(\.scenePhase) private var phase
    @State private var screen = "onboarding"
    @State private var activeScreen: UIScreen?
    @State private var dictationOpen = false

    var body: some View {
        TabView(selection: $screen) {
            Tab(value: "onboarding") {
                NavigationStack { PlaceholderOnboarding(onboarding: runtime.onboarding, models: runtime.models) }
            } label: { Text("Onboarding").accessibilityIdentifier("screen.onboarding") }
            Tab(value: "models") {
                NavigationStack { PlaceholderModels(model: runtime.models) }
            } label: { Text("Models").accessibilityIdentifier("screen.models") }
            Tab(value: "server") {
                NavigationStack { PlaceholderServer(model: runtime.server) }
            } label: { Text("Server").accessibilityIdentifier("screen.server") }
            Tab(value: "devices") {
                NavigationStack { PlaceholderDevices(model: runtime.devices, openServer: { screen = "server" }) }
            } label: { Text("Devices").accessibilityIdentifier("screen.devices") }
            Tab(value: "settings") {
                NavigationStack { PlaceholderSettings(model: runtime.settings, onboarding: runtime.onboarding) }
            } label: { Text("Settings").accessibilityIdentifier("screen.settings") }
        }
        .background(SceneScreenReader { value in activeScreen = value; updateScene() })
        .onAppear { runtime.server.setOpen(true); updateScene() }
        .onChange(of: phase) { _, _ in updateScene() }
        .task(id: incomingURL) {
            guard let url = incomingURL else { return }
            runtime.server.stop()
            dictationOpen = true
            await runtime.dictation.open(url: url)
        }
        .sheet(isPresented: $dictationOpen, onDismiss: { runtime.dictation.abort() }) {
            NavigationStack { PlaceholderDictation(model: runtime.dictation) }
        }
    }

    private func updateScene() {
        runtime.server.setActive(phase == .active, screen: activeScreen)
        runtime.onboarding.refresh()
        runtime.dictation.setActive(phase == .active)
        Task { await runtime.settings.refresh() }
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
