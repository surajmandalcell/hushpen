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
                    RootView(runtime: runtime, incomingURL: app.incomingURL)
                } else {
                    LaunchView(loading: app.loading, notice: app.notice) { Task { await app.load() } }
                        .task { await app.load() }
                }
            }
            .onOpenURL { app.incomingURL = $0 }
        }
    }
}
