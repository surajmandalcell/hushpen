import SwiftUI

/// A list title with an accent symbol, like a row in the system Settings app.
struct RowTitle: View {
    let title: String
    let symbol: String

    var body: some View {
        Label {
            Text(title)
        } icon: {
            Image(systemName: symbol).foregroundStyle(.tint)
        }
    }
}

/// A title and a quiet reading on the right, for rows that only state a fact.
struct Reading: View {
    let title: String
    let symbol: String?
    let value: String

    var body: some View {
        LabeledContent {
            Text(value).foregroundStyle(.secondary)
        } label: {
            if let symbol { RowTitle(title: title, symbol: symbol) } else { Text(title) }
        }
    }
}

struct ReadyMark: View {
    var label = "Ready"

    var body: some View {
        Image(systemName: "checkmark.circle.fill")
            .foregroundStyle(.green)
            .accessibilityLabel(label)
    }
}

/// One line of help or trouble at the top of a list.
struct NoticeRow: View {
    let text: String

    var body: some View {
        Label {
            Text(text).font(.subheadline)
        } icon: {
            Image(systemName: "exclamationmark.circle.fill").foregroundStyle(.orange)
        }
    }
}

/// The speech wave. It moves only while `active`, and never with Reduce Motion.
struct WaveGlyph: View {
    enum Style { case listening, working, still }

    let style: Style
    @ScaledMetric private var size: CGFloat
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    init(size: CGFloat, style: Style) {
        _size = ScaledMetric(wrappedValue: size)
        self.style = style
    }

    var body: some View {
        Image(systemName: "waveform")
            .font(.system(size: size, weight: .medium))
            .symbolEffect(.variableColor.iterative.dimInactiveLayers.nonReversing, options: .repeating, isActive: style == .listening && !reduceMotion)
            .symbolEffect(.pulse, options: .repeating, isActive: style == .working && !reduceMotion)
            .accessibilityHidden(true)
    }
}

func sizeLabel(_ bytes: UInt64) -> String {
    let value = Double(bytes)
    return value >= 1e9 ? String(format: "%.1f GB", value / 1e9) : "\(Int((value / 1e6).rounded())) MB"
}

/// Rust names a paired desktop only by its public key, shown as lowercase hex.
func desktopName(_ id: String) -> String { "Desktop \(id.prefix(8))" }

func speechPurpose(_ model: Model) -> String {
    switch model.name.lowercased() {
    case let name where name.hasPrefix("parakeet"): "English and 24 European languages"
    case let name where name.hasPrefix("whisper"): "Other languages and slower phones"
    default: "Speech to text"
    }
}

/// A white card on the grouped background, the way the phone screens group content.
struct CardBackground: ViewModifier {
    var padding: CGFloat = 16

    func body(content: Content) -> some View {
        content
            .padding(padding)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color(.secondarySystemGroupedBackground), in: RoundedRectangle(cornerRadius: 24, style: .continuous))
    }
}

extension View {
    func card(padding: CGFloat = 16) -> some View { modifier(CardBackground(padding: padding)) }
}

/// A card title: an accent symbol and a name, with an optional quiet reading at the end.
struct CardHeader: View {
    let symbol: String
    let title: String
    var trailing: String?

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: symbol).imageScale(.small).accessibilityHidden(true)
            Text(title).fontWeight(.medium)
            Spacer(minLength: 8)
            if let trailing {
                Text(trailing).font(.subheadline).foregroundStyle(.secondary)
                Image(systemName: "chevron.right").font(.footnote.weight(.semibold)).foregroundStyle(.secondary).accessibilityHidden(true)
            }
        }
        .font(.headline)
        .foregroundStyle(.tint)
    }
}

/// The big number of a stat card with its small unit, such as `20` `min`.
struct BigNumber: View {
    let value: String
    var unit: String?

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 5) {
            Text(value).font(.system(.largeTitle, weight: .bold)).monospacedDigit()
            if let unit { Text(unit).font(.title3).foregroundStyle(.secondary) }
        }
        .accessibilityElement(children: .combine)
    }
}

/// A button with a 28 pt glyph and a 44 pt touch area, for the trailing end of a card line.
struct GlyphButton: View {
    let symbol: String
    let label: String
    var size: CGFloat = 28
    var color: Color = .accentColor
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol).font(.system(size: size, weight: .regular)).foregroundStyle(color)
                .frame(width: 44, height: 44)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
    }
}

func wordCount(_ words: UInt32) -> String { words == 1 ? "1 word" : "\(words) words" }

/// "7 s" up to a minute, then whole minutes, so a history line never grows.
func spokenLabel(_ ms: UInt64) -> String {
    let seconds = Int((Double(ms) / 1000).rounded())
    return seconds < 60 ? "\(seconds) s" : "\(Int((Double(seconds) / 60).rounded())) min"
}

func tookLabel(_ ms: UInt64) -> String { String(format: "%.1f s", Double(ms) / 1000) }

/// A model id from the history as a person reads it, or the id itself when the catalog lacks it.
func languageName(_ code: String) -> String {
    code.isEmpty ? "Unknown" : (Locale.current.localizedString(forLanguageCode: code) ?? code).capitalized
}
