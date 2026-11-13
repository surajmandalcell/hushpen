import SwiftUI
import UIKit

private extension Color {
    init(light: UIColor, dark: UIColor) {
        self.init(uiColor: UIColor { $0.userInterfaceStyle == .dark ? dark : light })
    }
}

private func rgb(_ value: UInt32, alpha: CGFloat = 1) -> UIColor {
    UIColor(
        red: CGFloat((value >> 16) & 0xFF) / 255, green: CGFloat((value >> 8) & 0xFF) / 255,
        blue: CGFloat(value & 0xFF) / 255, alpha: alpha
    )
}

/// The system keyboard colors, and a pill for each appearance: white on Light, the desktop flow bar's
/// dark HUD on Dark.
private enum Palette {
    static let letter = Color(light: rgb(0xFFFFFF), dark: rgb(0x6B6B6E))
    static let function = Color(light: rgb(0xABB0BC), dark: rgb(0x444447))
    static let keyShadow = Color(light: rgb(0x000000, alpha: 0.3), dark: rgb(0x000000, alpha: 0.45))

    static let pill = Color(light: rgb(0xFFFFFF), dark: rgb(0x1F1F1F))
    static let pillEdge = Color(light: rgb(0x000000, alpha: 0.08), dark: rgb(0x353535))
    static let pillShadow = Color(light: rgb(0x000000, alpha: 0.16), dark: rgb(0x000000, alpha: 0.25))
    static let pillText = Color(light: rgb(0x1C1C1E), dark: rgb(0xFFFFFF))
    static let pillButton = Color(light: rgb(0x3C3C43, alpha: 0.12), dark: rgb(0x3A3A3C))
    static let pillMain = Color(light: rgb(0x1C1C1E), dark: rgb(0xFFFFFF))
    static let pillMainText = Color(light: rgb(0xFFFFFF), dark: rgb(0x1F1F1F))
    static let pillOk = Color(light: rgb(0x34C759, alpha: 0.16), dark: rgb(0x30D158, alpha: 0.18))
    static let pillOkText = Color(light: rgb(0x248A3D), dark: rgb(0x30D158))
    static let pillWarn = Color(light: rgb(0xFF9500, alpha: 0.16), dark: rgb(0xFFC266, alpha: 0.18))
    static let pillWarnText = Color(light: rgb(0xB45309), dark: rgb(0xFFC266))
}

/// The system keyboard look with our mic key. A fixed 44 pt band above the keys holds the pill, so
/// nothing moves when it appears.
struct KeyboardView: View {
    @Bindable var model: KeyboardModel

    var body: some View {
        VStack(spacing: 0) {
            Band(model: model).frame(height: 44)
            Keys(model: model)
        }
    }
}

// MARK: - Pill

private struct Band: View {
    @Bindable var model: KeyboardModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let pill = model.pill
        ZStack {
            ZStack { content(pill).transition(.opacity) }
                .frame(width: width(pill), height: 36)
                .background(Palette.pill, in: Capsule())
                .overlay(Capsule().strokeBorder(Palette.pillEdge, lineWidth: 0.5))
                .shadow(color: Palette.pillShadow, radius: 6, y: 4)
                .opacity(pill == .idle ? 0 : 1)
                .scaleEffect(pill == .idle ? 0.96 : 1)
                .allowsHitTesting(pill != .idle)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .animation(reduceMotion ? nil : .easeOut(duration: 0.2), value: pill)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(label(pill))
        .accessibilityIdentifier("keyboard.status")
    }

    private func width(_ pill: KeyboardPill) -> CGFloat? {
        switch pill {
        case .idle, .listening, .inserted: 154
        case .transcribing: 148
        case .pasteFailed(copied: true): 296
        case .pasteFailed(copied: false): 206
        case .notice: nil
        }
    }

    private func label(_ pill: KeyboardPill) -> String {
        switch pill {
        case .idle: "Ready"
        case .listening: "Listening"
        case .transcribing: "Transcribing"
        case .inserted: "Inserted"
        case .pasteFailed(copied: true): "Paste failed. Copied to clipboard."
        case .pasteFailed(copied: false): "Paste failed."
        case let .notice(text): text
        }
    }

    @ViewBuilder private func content(_ pill: KeyboardPill) -> some View {
        switch pill {
        case .idle:
            Color.clear
        case .listening:
            HStack(spacing: 0) {
                round("xmark", fill: Palette.pillButton, ink: Palette.pillText, label: "Cancel dictation", id: "keyboard.cancel", action: model.cancel)
                Spacer(minLength: 0)
                Wave(bars: 11).accessibilityHidden(true)
                Spacer(minLength: 0)
                round("checkmark", fill: Palette.pillMain, ink: Palette.pillMainText, label: "Finish dictation", id: "keyboard.finish", action: model.stop)
            }
            .padding(4)
        case .transcribing:
            HStack(spacing: 8) {
                Wave(bars: 7, ripple: true).accessibilityHidden(true)
                text("Transcribing")
            }
        case .inserted:
            HStack(spacing: 0) {
                mark("checkmark", fill: Palette.pillOk, ink: Palette.pillOkText)
                text("Inserted").frame(maxWidth: .infinity)
                round("arrow.uturn.backward", fill: Palette.pillButton, ink: Palette.pillText, label: "Undo", id: "keyboard.undo", action: model.undo)
            }
            .padding(4)
        case .pasteFailed(copied: true):
            HStack(spacing: 0) {
                mark("exclamationmark", fill: Palette.pillWarn, ink: Palette.pillWarnText)
                text("Paste failed. Copied to clipboard.").frame(maxWidth: .infinity)
                round("xmark", fill: Palette.pillButton, ink: Palette.pillText, label: "Dismiss", id: "keyboard.dismiss", action: model.dismiss)
            }
            .padding(4)
        case .pasteFailed(copied: false):
            HStack(spacing: 8) {
                mark("exclamationmark", fill: Palette.pillWarn, ink: Palette.pillWarnText)
                text("Paste failed.").frame(maxWidth: .infinity, alignment: .leading)
                Button(action: model.copy) {
                    Text("Copy")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(Palette.pillMainText)
                        .padding(.horizontal, 12)
                        .frame(height: 28)
                        .background(Palette.pillMain, in: Capsule())
                        .contentShape(Capsule().inset(by: -4))
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("keyboard.copy")
            }
            .padding(4)
        case let .notice(message):
            HStack(spacing: 8) {
                mark("exclamationmark", fill: Palette.pillWarn, ink: Palette.pillWarnText)
                text(message)
            }
            .padding(.leading, 4)
            .padding(.trailing, 14)
        }
    }

    private func text(_ value: String) -> some View {
        Text(value)
            .font(.system(size: 13, weight: .medium))
            .foregroundStyle(Palette.pillText)
            .lineLimit(1)
            .minimumScaleFactor(0.8)
    }

    private func mark(_ symbol: String, fill: Color, ink: Color) -> some View {
        Image(systemName: symbol)
            .font(.system(size: 13, weight: .bold))
            .foregroundStyle(ink)
            .frame(width: 28, height: 28)
            .background(fill, in: Circle())
            .accessibilityHidden(true)
    }

    private func round(_ symbol: String, fill: Color, ink: Color, label: String, id: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 13, weight: .bold))
                .foregroundStyle(ink)
                .frame(width: 28, height: 28)
                .background(fill, in: Circle())
                .contentShape(Circle().inset(by: -4))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
        .accessibilityIdentifier(id)
    }
}

/// Rounded bars that rise and fall: steady while listening, a faster ripple while transcribing.
private struct Wave: View {
    let bars: Int
    var ripple = false
    @State private var high = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private static let heights: [CGFloat] = [0.35, 0.6, 0.85, 1, 0.7, 0.95, 0.55, 0.75, 0.4, 0.65, 0.9]

    var body: some View {
        HStack(spacing: 3) {
            ForEach(0..<bars, id: \.self) { index in
                let peak = ripple ? 0.8 : Self.heights[index % Self.heights.count]
                let rest = ripple ? 0.25 : peak * 0.3
                Capsule()
                    .frame(width: 2.5, height: 22)
                    .scaleEffect(y: reduceMotion || high ? peak : rest)
                    .animation(
                        reduceMotion ? nil : .easeInOut(duration: ripple ? 0.55 : 0.5).repeatForever(autoreverses: true)
                            .delay(ripple ? Double(index) * 0.09 : (Double(index) * 0.13).truncatingRemainder(dividingBy: 1)),
                        value: high
                    )
            }
        }
        .frame(height: 22)
        .foregroundStyle(Palette.pillText)
        .onAppear { high = true }
    }
}

// MARK: - Keys

private struct Keys: View {
    @Bindable var model: KeyboardModel

    private static let letters = ["qwertyuiop", "asdfghjkl", "zxcvbnm"]
    private static let symbols = ["1234567890", "-/:;()$&@\"", ".,?!'"]

    var body: some View {
        GeometryReader { proxy in
            let unit = proxy.size.width / 10
            let rows = model.symbols ? Self.symbols : Self.letters
            VStack(spacing: 0) {
                row(rows[0])
                row(rows[1]).padding(.horizontal, model.symbols ? 0 : unit / 2)
                thirdRow(rows[2], unit: unit)
                bottomRow(unit: unit)
            }
        }
        .frame(height: 216)
    }

    private func row(_ characters: String) -> some View {
        HStack(spacing: 0) {
            ForEach(Array(characters).map(String.init), id: \.self) { character in letter(character) }
        }
    }

    private func letter(_ key: String) -> some View {
        Button { model.insert(key) } label: {
            Text(model.shift ? key.uppercased() : key).font(.system(size: 24))
        }
        .buttonStyle(KeyStyle(kind: .letter, preview: model.shift ? key.uppercased() : key))
        .accessibilityIdentifier("keyboard.key.\(key)")
        .accessibilityAddTraits(.isKeyboardKey)
    }

    private func thirdRow(_ characters: String, unit: CGFloat) -> some View {
        HStack(spacing: 0) {
            if model.symbols {
                Color.clear.frame(width: unit * 1.45)
            } else {
                Button { model.shift.toggle() } label: {
                    Image(systemName: model.shift ? "shift.fill" : "shift").font(.system(size: 20))
                }
                .buttonStyle(KeyStyle(kind: model.shift ? .letter : .function))
                .frame(width: unit * 1.23)
                .accessibilityLabel("Shift")
                .accessibilityIdentifier("keyboard.shift")
                Color.clear.frame(width: unit * 0.27)
            }
            ForEach(Array(characters).map(String.init), id: \.self) { character in letter(character) }
            Color.clear.frame(width: unit * 0.27)
            Button(action: model.backspace) {
                Image(systemName: "delete.left").font(.system(size: 20))
            }
            .buttonStyle(KeyStyle(kind: .function))
            .frame(width: unit * 1.23)
            .accessibilityLabel("Delete")
            .accessibilityIdentifier("keyboard.backspace")
        }
    }

    private func bottomRow(unit: CGFloat) -> some View {
        HStack(spacing: 0) {
            Button { model.symbols.toggle() } label: {
                Text(model.symbols ? "ABC" : "123").font(.system(size: 16))
            }
            .buttonStyle(KeyStyle(kind: .function))
            .frame(width: unit * 1.25)
            .accessibilityIdentifier("keyboard.symbols")
            if model.needsGlobe {
                Button(action: model.nextKeyboard) {
                    Image(systemName: "globe").font(.system(size: 20))
                }
                .buttonStyle(KeyStyle(kind: .function))
                .frame(width: unit * 1.1)
                .accessibilityLabel("Next keyboard")
                .accessibilityIdentifier("keyboard.next")
            }
            Button { model.insert(" ") } label: {
                Text("space").font(.system(size: 16))
            }
            .buttonStyle(KeyStyle(kind: .letter))
            .accessibilityIdentifier("keyboard.space")
            mic.frame(width: unit * 1.25)
            Button { model.insert("\n") } label: {
                Text("return").font(.system(size: 16))
            }
            .buttonStyle(KeyStyle(kind: .function))
            .frame(width: unit * 2.1)
            .accessibilityIdentifier("keyboard.return")
        }
    }

    /// Only the mic dims while the app listens or transcribes; every other key keeps working.
    @ViewBuilder private var mic: some View {
        let glyph = Image(systemName: "mic").font(.system(size: 20))
        switch model.pill {
        case .listening:
            Button(action: model.stop) { glyph }
                .buttonStyle(KeyStyle(kind: .function))
                .opacity(0.4)
                .accessibilityLabel("Finish dictation")
                .accessibilityIdentifier("keyboard.mic")
        case .transcribing:
            glyph
                .modifier(KeyFace(kind: .function))
                .opacity(0.4)
                .accessibilityLabel("Transcribing")
                .accessibilityIdentifier("keyboard.mic")
        default:
            if let url = model.url, model.fullAccess {
                Link(destination: url) { glyph }
                    .buttonStyle(KeyStyle(kind: .function))
                    .accessibilityLabel("Dictate")
                    .accessibilityIdentifier("keyboard.mic")
            } else {
                glyph
                    .modifier(KeyFace(kind: .function))
                    .opacity(0.4)
                    .accessibilityLabel("Dictation unavailable")
                    .accessibilityIdentifier("keyboard.mic")
            }
        }
    }
}

private struct KeyStyle: ButtonStyle {
    enum Kind { case letter, function }

    let kind: Kind
    var preview: String?

    func makeBody(configuration: Configuration) -> some View {
        let pressed = configuration.isPressed
        configuration.label
            .modifier(KeyFace(kind: kind, pressed: pressed))
            .overlay(alignment: .top) {
                if pressed, let preview {
                    Text(preview)
                        .font(.system(size: 34))
                        .foregroundStyle(.primary)
                        .frame(width: 56, height: 54)
                        .background(Palette.letter, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                        .shadow(color: .black.opacity(0.25), radius: 5, y: 2)
                        .offset(y: -52)
                        .allowsHitTesting(false)
                }
            }
    }
}

/// A key: 43 pt tall in a 54 pt row, 8 pt corners, a one point edge below.
private struct KeyFace: ViewModifier {
    let kind: KeyStyle.Kind
    var pressed = false

    func body(content: Content) -> some View {
        content
            .foregroundStyle(Color.primary)
            .frame(maxWidth: .infinity, minHeight: 43, maxHeight: 43)
            .background(fill, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            .shadow(color: Palette.keyShadow, radius: 0, y: 1)
            .padding(.horizontal, 3)
            .padding(.vertical, 5.5)
            .contentShape(Rectangle())
    }

    private var fill: Color {
        switch kind {
        case .letter: pressed ? Palette.function : Palette.letter
        case .function: pressed ? Palette.letter : Palette.function
        }
    }
}
