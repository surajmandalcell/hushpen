import SwiftUI

/// The page the keyboard opens to record. The user then goes back to the app they were typing in.
struct DictationScreen: View {
    let model: DictationModel
    let close: () -> Void
    @State private var startedAt: Date?

    private static let orange = Color(red: 1, green: 0.624, blue: 0.039)

    var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                VStack(spacing: 18) {
                    hero.padding(.top, 56)
                    Text(title)
                        .font(.title2.weight(.semibold))
                        .accessibilityIdentifier("dictation.state")
                    subtitle
                    if model.state == "listening" { goBack.padding(.top, 14) }
                    if !model.result.isEmpty { result.padding(.top, 6) }
                    if !model.notice.isEmpty {
                        Text(model.notice)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                            .multilineTextAlignment(.center)
                            .accessibilityIdentifier("dictation.notice")
                    }
                }
                .padding(.horizontal, 24)
                .frame(maxWidth: 520)
                .frame(maxWidth: .infinity)
            }
            .scrollBounceBehavior(.basedOnSize)
            controls.padding(.vertical, 20)
        }
        .background(Color(.systemBackground))
        .onChange(of: model.state) { _, state in if state == "listening" { startedAt = .now } }
        .onAppear { if model.state == "listening" && startedAt == nil { startedAt = .now } }
    }

    private var title: String {
        switch model.state {
        case "listening": "Listening"
        case "transcribing": "Transcribing"
        case "cleaning": "Cleaning up"
        case "done": "Done"
        case "cancelled": "Cancelled"
        default: "Not listening"
        }
    }

    @ViewBuilder private var hero: some View {
        switch model.state {
        case "listening": WaveGlyph(size: 96, style: .listening).foregroundStyle(.tint)
        case "transcribing", "cleaning": WaveGlyph(size: 96, style: .working).foregroundStyle(.tint)
        case "done":
            Image(systemName: "checkmark.circle.fill").font(.system(size: 80)).foregroundStyle(.green).accessibilityHidden(true)
        case "cancelled":
            Image(systemName: "xmark.circle").font(.system(size: 80)).foregroundStyle(.secondary).accessibilityHidden(true)
        default:
            Image(systemName: model.notice.isEmpty ? "waveform" : "exclamationmark.triangle.fill")
                .font(.system(size: 80))
                .foregroundStyle(model.notice.isEmpty ? Color.secondary : Color.orange)
                .accessibilityHidden(true)
        }
    }

    @ViewBuilder private var subtitle: some View {
        switch model.state {
        case "listening":
            Text(timerInterval: (startedAt ?? .now)...Date.distantFuture, pauseTime: nil, countsDown: false, showsHours: false)
                .font(.title3.monospacedDigit())
                .foregroundStyle(Self.orange)
        case "transcribing": Text("Turning your speech into text on this iPhone.").subtitleStyle()
        case "cleaning": Text("Polishing the text.").subtitleStyle()
        case "done": Text("Your text is ready.").subtitleStyle()
        case "cancelled": Text("Recover to transcribe what you said.").subtitleStyle()
        default: Text("Start again from the Hushpen keyboard.").subtitleStyle()
        }
    }

    private var goBack: some View {
        HStack(alignment: .top, spacing: 14) {
            Image(systemName: "arrow.left").font(.title3).foregroundStyle(.tint).padding(.top, 1)
            VStack(alignment: .leading, spacing: 3) {
                Text("Go back and keep talking").font(.headline)
                Text("Hushpen listens in the background and shows in the Dynamic Island. Tap the check mark in the keyboard to insert.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                Text("Swipe right on the bar at the bottom to go back.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .padding(.top, 4)
            }
        }
        .padding(18)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color(.secondarySystemFill), in: RoundedRectangle(cornerRadius: 26, style: .continuous))
    }

    private var result: some View {
        Text(model.result)
            .font(.body)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(16)
            .background(Color(.secondarySystemFill), in: RoundedRectangle(cornerRadius: 20, style: .continuous))
            .textSelection(.enabled)
            .accessibilityIdentifier("dictation.result")
    }

    @ViewBuilder private var controls: some View {
        HStack(alignment: .top, spacing: 44) {
            if model.busy {
                action("Cancel", symbol: "xmark", id: "dictation.cancel") { model.cancel() }
                if model.state == "listening" {
                    action("Done", symbol: "checkmark", id: "dictation.finish", prominent: true) { model.finish() }
                }
            } else if model.recoverable {
                action("Close", symbol: "xmark", id: "dictation.close", action: close)
                action("Recover", symbol: "arrow.uturn.backward", id: "dictation.recover", prominent: true) { model.recover() }
            } else {
                action("Close", symbol: "xmark", id: "dictation.close", prominent: model.state == "done", action: close)
            }
        }
    }

    private func action(_ title: String, symbol: String, id: String, prominent: Bool = false, action: @escaping () -> Void) -> some View {
        let label = Image(systemName: symbol)
            .font(.title.weight(.semibold))
            .frame(width: 64, height: 64)
        return VStack(spacing: 8) {
            Group {
                if prominent {
                    Button(action: action) { label }.buttonStyle(.glassProminent)
                } else {
                    Button(action: action) { label }.buttonStyle(.glass)
                }
            }
            .buttonBorderShape(.circle)
            .accessibilityLabel(title)
            .accessibilityIdentifier(id)
            Text(title).font(.footnote).foregroundStyle(.secondary).accessibilityHidden(true)
        }
    }
}

private extension Text {
    func subtitleStyle() -> some View {
        font(.body).foregroundStyle(.secondary).multilineTextAlignment(.center)
    }
}
