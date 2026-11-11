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
