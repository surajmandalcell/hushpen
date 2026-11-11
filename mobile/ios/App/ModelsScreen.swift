import SwiftUI

struct ModelsScreen: View {
    let model: ModelsModel
    let settings: SettingsModel

    private var speech: [Model] { model.models.filter { $0.kind == .speech } }
    private var cleanup: [Model] { model.models.filter { $0.kind == .systemCleanup } }

    var body: some View {
        List {
            if !model.notice.isEmpty {
                Section { NoticeRow(text: model.notice).accessibilityIdentifier("models.notice") }
            }
            Section {
                if speech.isEmpty { Text("No models are available.").foregroundStyle(.secondary) }
                ForEach(speech, id: \.id) { SpeechRow(entry: $0, model: model) }
            } header: {
                Text("Speech to Text")
            } footer: {
                Text("Tap a downloaded model to use it for the keyboard and paired desktops.")
            }
            if !cleanup.isEmpty {
                Section {
                    ForEach(cleanup, id: \.id) { entry in
                        Label {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(entry.name)
                                Text(cleanupStatus).font(.subheadline).foregroundStyle(.secondary)
                            }
                        } icon: {
                            Image(systemName: "sparkles").foregroundStyle(.tint)
                        }
                    }
                } header: {
                    Text("Text cleanup")
                } footer: {
                    Text("Built into this iPhone. Nothing to download.")
                }
            }
        }
        .navigationTitle("Models")
        .navigationBarTitleDisplayMode(.large)
        .refreshable { await model.refresh() }
        .task { await model.refresh() }
    }

    private var cleanupStatus: String {
        if !settings.cleanupEnabled { return "Off. Turn it on in Settings." }
        return settings.systemAvailable ? "Ready. Works while Hushpen is open." : "Not available on this iPhone. Rule cleanup still works."
    }
}

private struct SpeechRow: View {
    let entry: Model
    let model: ModelsModel

    private var active: Bool { model.downloading == entry.id }
    private var ready: Bool { entry.status == .ready }
    private var inUse: Bool { ready && entry.id == model.selectedModel }
    private var idle: Bool { !model.busy && model.downloading == nil }
    private var fraction: Double { model.total > 0 ? Double(model.progress) / Double(model.total) : 0 }

    var body: some View {
        Label {
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(entry.name).accessibilityIdentifier(inUse ? "models.selected" : "models.row")
                    Text(speechPurpose(entry)).font(.subheadline).foregroundStyle(.secondary)
                    Text(status).font(.footnote).foregroundStyle(entry.status == .damaged && !active ? Color.red : Color.secondary)
                    if active {
                        Group {
                            if model.verifying || model.total == 0 { ProgressView() } else { ProgressView(value: fraction) }
                        }
                        .progressViewStyle(.linear)
                        .padding(.top, 6)
                    }
                }
                Spacer(minLength: 0)
                trailing
            }
        } icon: {
            Image(systemName: "waveform").foregroundStyle(.tint)
        }
        .contentShape(Rectangle())
        .onTapGesture { if ready && !inUse && idle { Task { await model.select(entry.id) } } }
    }

    private var status: String {
        if active { return model.verifying ? "Checking the file" : "Downloading \(sizeLabel(model.progress)) of \(sizeLabel(model.total))" }
        var parts = [sizeLabel(entry.bytes)]
        if entry.recommended { parts.append("Recommended") }
        switch entry.status {
        case .ready: parts.append(inUse ? "In use" : "Downloaded")
        case .needsVerification: parts.append("Not checked")
        case .damaged: parts.append("Damaged")
        default: break
        }
        return parts.joined(separator: " \u{B7} ")
    }

    @ViewBuilder private var trailing: some View {
        if active {
            Button { model.cancelDownload() } label: {
                Image(systemName: "xmark.circle.fill").font(.title2).foregroundStyle(.secondary)
            }
            .buttonStyle(.borderless)
            .accessibilityLabel("Cancel download")
        } else if inUse {
            Image(systemName: "checkmark.circle.fill").font(.title3).foregroundStyle(.tint).accessibilityLabel("In use")
        } else if ready {
            Button("Use") { Task { await model.select(entry.id) } }
                .buttonStyle(.borderless)
                .disabled(!idle)
        } else if entry.status == .needsVerification {
            Button("Verify") { Task { await model.verify(entry.id) } }
                .buttonStyle(.borderless)
                .disabled(!idle)
        } else {
            Button { Task { await model.download(entry.id) } } label: {
                Image(systemName: "arrow.down.circle").font(.title2)
            }
            .buttonStyle(.borderless)
            .disabled(model.downloading != nil)
            .accessibilityLabel("Download \(entry.name)")
        }
    }
}
