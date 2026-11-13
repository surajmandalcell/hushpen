import SwiftUI

/// The hand-off demo: a finger swipes right along the bottom edge and the previous app's card slides
/// back in. One 2 s loop. With Reduce Motion it stands still on a frame that shows the direction.
struct SwipeDemo: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        TimelineView(.animation(paused: reduceMotion)) { context in
            let phase = reduceMotion ? 0.3 : context.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: 2) / 2
            Stage(frame: SwipeFrame(phase: phase), time: context.date.timeIntervalSinceReferenceDate, still: reduceMotion)
        }
        .accessibilityHidden(true)
    }
}

/// Where each part is at one point of the loop. A card's `shift` is in card widths: 0 centered, -1 to the left.
private struct SwipeFrame {
    var appShift = 0.0
    var appOpacity = 1.0
    var previousShift = -1.0
    var previousOpacity = 0.55
    var finger = 0.0
    var fingerOpacity = 0.0
    var fingerScale = 1.2

    private static let ease = UnitCurve.bezier(startControlPoint: .init(x: 0.4, y: 0), endControlPoint: .init(x: 0.2, y: 1))

    /// Eases between the stops, like a CSS keyframe list with one timing function.
    private static func track(_ phase: Double, _ stops: [(Double, Double)]) -> Double {
        for (from, to) in zip(stops, stops.dropFirst()) where phase < to.0 {
            let local = max(0, (phase - from.0) / (to.0 - from.0))
            return from.1 + (to.1 - from.1) * ease.value(at: local)
        }
        return stops.last?.1 ?? 0
    }

    init(phase: Double) {
        appShift = Self.track(phase, [(0, 0), (0.08, 0), (0.44, 1), (0.78, 1), (0.79, 0), (1, 0)])
        appOpacity = Self.track(phase, [(0, 1), (0.08, 1), (0.44, 0.55), (0.66, 0.55), (0.78, 0), (0.79, 0), (0.92, 1), (1, 1)])
        previousShift = Self.track(phase, [(0, -1), (0.08, -1), (0.44, 0), (0.78, 0), (0.79, -1), (1, -1)])
        previousOpacity = Self.track(phase, [(0, 0.55), (0.08, 0.55), (0.44, 1), (0.66, 1), (0.78, 0), (0.79, 0), (0.92, 0.55), (1, 0.55)])
        finger = Self.track(phase, [(0, 0), (0.06, 0), (0.10, -24), (0.44, 96), (1, 96)])
        fingerOpacity = Self.track(phase, [(0, 0), (0.06, 1), (0.44, 1), (0.54, 0), (1, 0)])
        fingerScale = Self.track(phase, [(0, 1.2), (0.06, 1), (0.44, 1), (0.54, 1.2), (1, 1.2)])
    }
}

private struct Stage: View {
    let frame: SwipeFrame
    let time: Double
    let still: Bool

    var body: some View {
        GeometryReader { proxy in
            // The cards, a row of chevrons, and the finger lane stack to 98 pt plus the card height.
            let scale = max(0.3, min(1, (proxy.size.height - 98) / 330))
            let travel = 176 * scale * 0.955
            VStack(spacing: 12) {
                ZStack {
                    Card(app: true)
                        .scaleEffect(scale * (1 - 0.16 * frame.appShift))
                        .offset(x: frame.appShift * travel)
                        .opacity(frame.appOpacity)
                    Card(app: false)
                        .scaleEffect(scale * (1 - 0.16 * -frame.previousShift))
                        .offset(x: frame.previousShift * travel)
                        .opacity(frame.previousOpacity)
                }
                .frame(height: 330 * scale)
                Chevrons(time: time, still: still).frame(height: 28)
                Circle()
                    .fill(Color.secondary.opacity(0.35))
                    .overlay(Circle().strokeBorder(Color.secondary.opacity(0.35), lineWidth: 1))
                    .frame(width: 46, height: 46)
                    .scaleEffect(frame.fingerScale)
                    .offset(x: frame.finger)
                    .opacity(frame.fingerOpacity)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}

private struct Card: View {
    let app: Bool

    var body: some View {
        VStack(spacing: 14) {
            if app {
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(.tint)
                    .frame(width: 54, height: 54)
                    .overlay { Image(systemName: "waveform").font(.system(size: 26, weight: .medium)).foregroundStyle(.white) }
                    .padding(.top, 20)
                line(112).padding(.top, 6)
                line(78)
            } else {
                HStack(spacing: 2) {
                    Image(systemName: "chevron.left").font(.system(size: 18, weight: .semibold)).foregroundStyle(.tint)
                    line(46)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.leading, 14)
                .padding(.top, 20)
                line(112)
                line(78)
                line(112)
                line(78)
            }
            Spacer(minLength: 0)
        }
        .frame(width: 176, height: 330)
        .background(Color(.secondarySystemGroupedBackground), in: RoundedRectangle(cornerRadius: 28, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 28, style: .continuous).strokeBorder(Color.primary.opacity(0.07), lineWidth: 0.5))
        .shadow(color: .black.opacity(0.16), radius: 15, y: 12)
    }

    private func line(_ width: CGFloat) -> some View {
        Capsule().fill(Color.secondary.opacity(0.2)).frame(width: width, height: 8)
    }
}

private struct Chevrons: View {
    let time: Double
    let still: Bool

    var body: some View {
        HStack(spacing: 2) {
            ForEach(0..<3, id: \.self) { index in
                let pulse = 0.5 - 0.5 * cos(2 * .pi * (time - Double(index) * 0.1) / 0.8)
                Image(systemName: "chevron.right")
                    .font(.system(size: 24, weight: .semibold))
                    .foregroundStyle(.tint)
                    .opacity(still ? 0.6 : 0.2 + 0.8 * pulse)
            }
        }
    }
}

/// A quiet ring around the system's back link in the status bar, and an arrow that nudges toward it.
/// iOS draws the link and its app name and the app cannot read either, so the ring is sized for a
/// typical name. On a Dynamic Island iPhone the link is a small line under the clock, at the left.
/// On a notch iPhone it replaces the clock in the left ear.
struct BackLinkHint: View {
    let top: CGFloat
    let width: CGFloat
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var ring: CGRect {
        if top >= 54 { return CGRect(x: 6, y: top * 0.62 - 10, width: 100, height: 20) }
        let ear = min(114, width * 0.29)
        return CGRect(x: (ear - 96) / 2, y: max(top / 2, 12) - 12, width: 96, height: 24)
    }

    var body: some View {
        TimelineView(.animation(paused: reduceMotion)) { context in
            let time = context.date.timeIntervalSinceReferenceDate
            let glow = reduceMotion ? 0.5 : 0.15 + 0.45 * (0.5 - 0.5 * cos(.pi * time))
            let nudge = reduceMotion ? 3 : 3 + 3 * cos(2 * .pi * time / 1.6)
            ZStack(alignment: .topLeading) {
                Capsule()
                    .strokeBorder(Color.accentColor, lineWidth: 1.5)
                    .opacity(glow)
                    .frame(width: ring.width, height: ring.height)
                    .position(x: ring.midX, y: ring.midY)
                Image(systemName: "arrow.up.left")
                    .font(.system(size: 28, weight: .medium))
                    .foregroundStyle(.tint)
                    .opacity(0.7)
                    .position(x: ring.midX + 14 + nudge, y: ring.maxY + 16 + nudge)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
        .accessibilityHidden(true)
    }
}
