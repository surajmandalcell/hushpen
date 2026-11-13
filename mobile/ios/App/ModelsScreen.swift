import SwiftUI

struct ModelsScreen: View {
    let model: ModelsModel
    let settings: SettingsModel

    private var speech: [Model] { model.models.filter { $0.kind == .speech } }
    private var cleanup: [Model] { model.models.filter { $0.kind == .systemCleanup || $0.kind == .cleanup } }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 8) {
                if !model.notice.isEmpty { NoticeRow(text: model.notice).card().accessibilityIdentifier("models.notice") }
                header("Speech to text")
                if speech.isEmpty { Text("No models are available.").foregroundStyle(.secondary).card() }
                ForEach(speech, id: \.id) { ModelCard(entry: $0, model: model, settings: settings) }
                Text("Tap a downloaded model to use it for the keyboard and paired desktops.")
                    .font(.footnote).foregroundStyle(.secondary).padding(.horizontal, 4)
                if !cleanup.isEmpty {
                    header("Text cleanup").padding(.top, 14)
                    ForEach(cleanup, id: \.id) { ModelCard(entry: $0, model: model, settings: settings) }
                    Text("The system model is first when available. Otherwise, Hushpen uses your selected downloaded cleanup model. Turn cleanup on in Settings.")
                        .font(.footnote).foregroundStyle(.secondary).padding(.horizontal, 4)
                }
            }
            .padding(.horizontal, 16)
            .padding(.bottom, 16)
        }
        .background(Color(.systemGroupedBackground))
        .navigationTitle("Models")
        .navigationBarTitleDisplayMode(.large)
        .toolbarVisibility(.hidden, for: .tabBar)
        .refreshable { await model.refresh() }
        .task { await model.refresh() }
    }

    private func header(_ title: String) -> some View {
        Text(title).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary)
            .padding(.horizontal, 4).accessibilityAddTraits(.isHeader)
    }
}

/// One model. The trailing button is 28 pt with a 44 pt touch area, centered on the name line.
private struct ModelCard: View {
    let entry: Model
    let model: ModelsModel
    let settings: SettingsModel

    private var info: ModelDetails? { model.details[entry.id] }
    private var system: Bool { entry.kind == .systemCleanup }
    private var active: Bool { model.downloading == entry.id }
    private var ready: Bool { entry.status == .ready }
    private var inUse: Bool { ready && entry.id == (entry.kind == .cleanup ? model.selectedCleanupModel : model.selectedModel) }
    private var idle: Bool { !model.busy && model.downloading == nil }
    private var fraction: Double { model.total > 0 ? Double(model.progress) / Double(model.total) : 0 }
    private var systemReady: Bool { settings.cleanupEnabled && settings.systemAvailable }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(alignment: .center, spacing: 8) {
                Text(entry.name).font(.headline)
                    .accessibilityIdentifier("\(entry.kind == .cleanup ? "models.cleanup" : "models").\(inUse ? "selected" : "row")")
                Spacer(minLength: 8)
                trailing.padding(.vertical, -11)
            }
            Text(blurb).font(.subheadline).foregroundStyle(.secondary)
            footer.padding(.top, 8)
        }
        .card()
        .contentShape(Rectangle())
        .onTapGesture { if ready && !inUse && idle && !system { Task { await model.select(entry.id) } } }
        .accessibilityElement(children: .contain)
    }

    private var blurb: String {
        if system { return "Built into this iPhone" }
        return info?.description ?? (entry.kind == .cleanup ? "Text cleanup" : speechPurpose(entry))
    }

    @ViewBuilder private var footer: some View {
        if active {
            VStack(alignment: .leading, spacing: 6) {
                Text(model.verifying ? "Checking the file" : "Downloading \(sizeLabel(model.progress)) of \(sizeLabel(model.total))")
                    .font(.footnote).foregroundStyle(.secondary)
            }
        } else if system {
            if !systemReady {
                Text(settings.cleanupEnabled ? "Unavailable. Use a downloaded cleanup model below." : "Off. Turn it on in Settings.")
                    .font(.footnote).foregroundStyle(.secondary)
            }
        } else if entry.status == .damaged {
            Text("\(sizeLabel(entry.bytes)) \u{B7} Damaged. Download it again.").font(.footnote).foregroundStyle(.red)
        } else if entry.status == .needsVerification {
            Text("\(sizeLabel(entry.bytes)) \u{B7} Not checked").font(.footnote).foregroundStyle(.secondary)
        } else {
            HStack(spacing: 14) {
                if let info {
                    Meter(title: "Accuracy", level: Int(info.accuracy))
                    Meter(title: "Speed", level: Int(info.speed))
                }
                Spacer(minLength: 4)
                Text(sizeLabel(entry.bytes)).font(.footnote).foregroundStyle(.secondary)
            }
        }
    }

    @ViewBuilder private var trailing: some View {
        if system {
            if systemReady { Image(systemName: "checkmark.circle.fill").font(.system(size: 28)).foregroundStyle(.tint).frame(width: 44, height: 44).accessibilityLabel("On") }
        } else if active {
            Button { model.cancelDownload() } label: {
                ProgressRing(fraction: model.verifying || model.total == 0 ? nil : fraction).frame(width: 44, height: 44).contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Cancel download")
        } else if ready {
            GlyphButton(symbol: "checkmark.circle.fill", label: inUse ? "In use" : "Use \(entry.name)", color: inUse ? .accentColor : Color(.tertiaryLabel)) {
                if !inUse { Task { await model.select(entry.id) } }
            }
            .disabled(!idle)
            .accessibilityIdentifier(inUse ? "models.inuse.\(entry.id)" : "models.select.\(entry.id)")
        } else if entry.status == .needsVerification {
            GlyphButton(symbol: "checkmark.shield", label: "Check \(entry.name)") { Task { await model.verify(entry.id) } }
                .disabled(!idle)
                .accessibilityIdentifier("models.verify.\(entry.id)")
        } else {
            GlyphButton(symbol: "arrow.down.circle", label: "Download \(entry.name)", color: entry.status == .damaged ? .red : .accentColor) { Task { await model.download(entry.id) } }
                .disabled(model.downloading != nil)
                .accessibilityIdentifier("models.download.\(entry.id)")
        }
    }
}

/// Five dots after a word: how many are filled is the model's rating from 1 to 5.
private struct Meter: View {
    let title: String
    let level: Int

    var body: some View {
        HStack(spacing: 6) {
            Text(title).font(.footnote).foregroundStyle(.secondary)
            HStack(spacing: 3) {
                ForEach(1...5, id: \.self) { Circle().fill($0 <= level ? Color.secondary : Color(.quaternaryLabel)).frame(width: 5, height: 5) }
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("\(title) \(level) of 5")
    }
}

/// A 28 pt ring that fills as the file arrives, with a stop square for "cancel". No fraction spins.
private struct ProgressRing: View {
    let fraction: Double?
    @State private var spin = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ZStack {
            Circle().stroke(Color.accentColor.opacity(0.25), lineWidth: 3)
            Circle().trim(from: 0, to: fraction ?? 0.28).stroke(Color.accentColor, style: StrokeStyle(lineWidth: 3, lineCap: .round))
                .rotationEffect(.degrees(spin ? 270 : -90))
            RoundedRectangle(cornerRadius: 2).fill(Color.accentColor).frame(width: 9, height: 9)
        }
        .frame(width: 28, height: 28)
        .onAppear { if fraction == nil && !reduceMotion { withAnimation(.linear(duration: 1).repeatForever(autoreverses: false)) { spin = true } } }
    }
}
