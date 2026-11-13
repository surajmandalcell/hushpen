@preconcurrency import ActivityKit
import SwiftUI
import WidgetKit

@main
struct HushpenActivityBundle: WidgetBundle {
    var body: some Widget { DictationLiveActivity() }
}

struct DictationLiveActivity: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: DictationActivityAttributes.self) { context in
            LockScreenView(attributes: context.attributes, phase: context.state.phase)
                .padding(16)
        } dynamicIsland: { context in
            let phase = context.state.phase
            return DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    PhaseGlyph().font(.title2).padding(.leading, 6)
                }
                DynamicIslandExpandedRegion(.trailing) {
                    PhaseClock(attributes: context.attributes, phase: phase)
                        .font(.title3.weight(.semibold))
                        .frame(width: 64, alignment: .trailing)
                        .padding(.trailing, 6)
                }
                DynamicIslandExpandedRegion(.center) {
                    Text(PhaseText.title(phase)).font(.headline)
                }
                DynamicIslandExpandedRegion(.bottom) {
                    Text(PhaseText.detail(phase))
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                        .padding(.top, 2)
                }
            } compactLeading: {
                PhaseGlyph()
            } compactTrailing: {
                PhaseClock(attributes: context.attributes, phase: phase)
                    .font(.footnote.weight(.semibold))
                    .frame(width: 44, alignment: .trailing)
            } minimal: {
                PhaseGlyph()
            }
            .keylineTint(PhaseText.blue)
        }
    }
}

private enum PhaseText {
    static let blue = Color(red: 0.039, green: 0.518, blue: 1)
    static let orange = Color(red: 1, green: 0.624, blue: 0.039)

    static func title(_ phase: String) -> String {
        switch phase {
        case "transcribing": "Transcribing"
        case "cleaning": "Cleaning up"
        default: "Listening"
        }
    }

    static func detail(_ phase: String) -> String {
        switch phase {
        case "transcribing", "cleaning": "Hushpen is turning your speech into text."
        default: "Tap the check mark in the keyboard to insert."
        }
    }
}

private struct PhaseGlyph: View {
    var body: some View {
        Image(systemName: "waveform")
            .foregroundStyle(PhaseText.blue)
            .accessibilityHidden(true)
    }
}

/// The elapsed time while listening. The system draws it, so the app never wakes to update it.
/// The range ends at 59:59: an open end makes the text reserve room for many digits, and the
/// compact island then clips "0:04" to "0:0".
private struct PhaseClock: View {
    let attributes: DictationActivityAttributes
    let phase: String

    var body: some View {
        // No cross-fade between the two: the fade left half of the old time ("0:0") under the dots.
        Group {
            if phase == "listening" {
                Text(timerInterval: attributes.startedAt...attributes.startedAt.addingTimeInterval(59 * 60 + 59), pauseTime: nil, countsDown: false, showsHours: false)
                    .monospacedDigit()
                    .foregroundStyle(PhaseText.orange)
                    .multilineTextAlignment(.trailing)
                    .transition(.identity)
            } else {
                Image(systemName: "ellipsis")
                    .foregroundStyle(.secondary)
                    .accessibilityLabel("Working")
                    .transition(.identity)
            }
        }
        .transaction { $0.animation = nil }
    }
}

private struct LockScreenView: View {
    let attributes: DictationActivityAttributes
    let phase: String

    var body: some View {
        HStack(spacing: 14) {
            PhaseGlyph()
                .font(.title2)
                .frame(width: 44, height: 44)
                .background(PhaseText.blue.opacity(0.18), in: Circle())
            VStack(alignment: .leading, spacing: 2) {
                Text(PhaseText.title(phase)).font(.headline)
                Text(PhaseText.detail(phase)).font(.subheadline).foregroundStyle(.secondary)
            }
            Spacer(minLength: 8)
            // A timer text reserves room for many digits; a fixed slot keeps the wording from wrapping.
            PhaseClock(attributes: attributes, phase: phase)
                .font(.title3.weight(.semibold))
                .frame(width: 64, alignment: .trailing)
        }
    }
}
