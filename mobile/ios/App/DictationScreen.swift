import SwiftUI

/// The page the keyboard opens to record. The user then goes back to the app they were typing in.
struct DictationScreen: View {
    let model: DictationModel
    let close: () -> Void

    /// The one line on the hand-off page. Change the words here.
    private static let handOffText = "You're all set. Head back to keep typing."

    var body: some View {
        VStack(spacing: 0) {
            if model.state == "listening" {
                handOff
            } else {
                ScrollView {
                    VStack(spacing: 18) {
                        hero.padding(.top, 56)
                        Text(title)
                            .font(.title2.weight(.semibold))
                            .accessibilityIdentifier("dictation.state")
                        subtitle
                        if !model.result.isEmpty { result.padding(.top, 6) }
                        notice
                    }
                    .padding(.horizontal, 24)
                    .frame(maxWidth: 520)
                    .frame(maxWidth: .infinity)
                }
                .scrollBounceBehavior(.basedOnSize)
            }
            // The hand-off page only shows how to swipe back; the keyboard pill holds Cancel and Done.
            if model.state != "listening" { controls.padding(.vertical, 20) }
        }
        .background(Color(.systemBackground))
        .overlay {
            if model.state == "listening" {
                // The insets read zero once a view ignores the safe area, so read them one level up.
                GeometryReader { proxy in
                    BackLinkHint(top: proxy.safeAreaInsets.top, width: proxy.size.width)
                        .ignoresSafeArea()
                }
                .allowsHitTesting(false)
            }
        }
    }

    /// The page the keyboard opens to: how to swipe back. Nothing here says or shows that it listens.
    private var handOff: some View {
        VStack(spacing: 0) {
            Text(Self.handOffText)
                .font(.system(size: 17, weight: .semibold))
                .lineLimit(1)
                .minimumScaleFactor(0.85)
                .padding(.top, 40)
                .padding(.horizontal, 24)
                // The state is for VoiceOver and the UI tests only.
                .accessibilityLabel("Listening. \(Self.handOffText)")
                .accessibilityIdentifier("dictation.state")
            SwipeDemo()
                .frame(maxWidth: 420, maxHeight: .infinity)
                .padding(.vertical, 20)
            notice.padding(.horizontal, 24)
        }
    }

    @ViewBuilder private var notice: some View {
        if !model.notice.isEmpty {
            Text(model.notice)
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .accessibilityIdentifier("dictation.notice")
        }
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
        case "transcribing": Text("Turning your speech into text on this \(deviceKind).").subtitleStyle()
        case "cleaning": Text("Polishing the text.").subtitleStyle()
        case "done": Text("Your text is ready.").subtitleStyle()
        case "cancelled": Text("Recover to transcribe what you said.").subtitleStyle()
        default: Text("Start again from the Hushpen keyboard.").subtitleStyle()
        }
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
