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

/// The system keyboard colors, and the ink of the glass pill, which follows Light and Dark.
private enum Palette {
    static let letter = Color(light: rgb(0xFFFFFF), dark: rgb(0x6B6B6E))
    static let function = Color(light: rgb(0xABB0BC), dark: rgb(0x444447))
    static let keyShadow = Color(light: rgb(0x000000, alpha: 0.3), dark: rgb(0x000000, alpha: 0.45))

    static let pillText = Color(light: rgb(0x1C1C1E), dark: rgb(0xFFFFFF))
    static let pillSub = Color(light: rgb(0x3C3C43, alpha: 0.6), dark: rgb(0xEBEBF5, alpha: 0.6))
    static let pillButton = Color(light: rgb(0x3C3C43, alpha: 0.12), dark: rgb(0xFFFFFF, alpha: 0.16))
    static let pillMain = Color(light: rgb(0x1C1C1E), dark: rgb(0xFFFFFF))
    static let pillMainText = Color(light: rgb(0xFFFFFF), dark: rgb(0x1F1F1F))
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
                .glassEffect(.regular, in: Capsule())
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
        case .idle, .listening: 154
        default: nil
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
                round("xmark", label: "Cancel dictation", id: "keyboard.cancel", action: model.cancel)
                Spacer(minLength: 0)
                Wave().accessibilityHidden(true)
                Spacer(minLength: 0)
                round("checkmark", main: true, label: "Finish dictation", id: "keyboard.finish", action: model.stop)
            }
            .padding(4)
        case .transcribing:
            HStack(spacing: 8) {
                Wave(settles: true).accessibilityHidden(true)
                text("Transcribing")
            }
            .padding(.horizontal, 16)
        case .inserted:
            HStack(spacing: 8) {
                mark("checkmark.circle.fill")
                text("Inserted")
                round("arrow.uturn.backward", label: "Undo", id: "keyboard.undo", action: model.undo)
            }
            .padding(4)
        case .pasteFailed(copied: true):
            HStack(spacing: 8) {
                mark("exclamationmark.circle.fill")
                text("Paste failed", sub: "Copied to clipboard")
                round("xmark", label: "Dismiss", id: "keyboard.dismiss", action: model.dismiss)
            }
            .padding(4)
        case .pasteFailed(copied: false):
            HStack(spacing: 8) {
                mark("exclamationmark.circle.fill")
                text("Paste failed")
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
                mark("exclamationmark.circle.fill")
                text(message)
            }
            .padding(.leading, 4)
            .padding(.trailing, 14)
        }
    }

    private func text(_ value: String, sub: String? = nil) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(value).font(.system(size: 13, weight: .semibold)).foregroundStyle(Palette.pillText)
            if let sub { Text(sub).font(.system(size: 11)).foregroundStyle(Palette.pillSub) }
        }
        .lineLimit(1)
        .minimumScaleFactor(0.8)
    }

    /// The System banner's status icon: a gray filled circle, never a status color.
    private func mark(_ symbol: String) -> some View {
        Image(systemName: symbol)
            .font(.system(size: 28))
            .foregroundStyle(Palette.pillSub)
            .frame(width: 28, height: 28)
            .accessibilityHidden(true)
    }

    /// A tinted round button; `main` is the dark (Light) or white (Dark) one for the primary action.
    private func round(_ symbol: String, main: Bool = false, label: String, id: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 13, weight: .bold))
                .foregroundStyle(main ? Palette.pillMainText : Palette.pillText)
                .frame(width: 28, height: 28)
                .background(main ? Palette.pillMain : Palette.pillButton, in: Circle())
                .contentShape(Circle().inset(by: -4))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
        .accessibilityIdentifier(id)
    }
}

/// Rounded bars that rise and fall: steady while listening, a faster ripple while transcribing.
private struct Wave: View {
    /// Transcribing starts as the flowing wave and turns once to the ripple after 2 s.
    var settles = false
    @State private var start = Date()
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        TimelineView(.animation(paused: reduceMotion)) { context in
            let t = reduceMotion ? 390 : context.date.timeIntervalSince(start) * 1000
            let turn = settles ? min(max((t - 2000) / 420, 0), 1) : 0
            HStack(spacing: 2) {
                ForEach(0..<11, id: \.self) { index in
                    let x = Double(index) / 10
                    // One taper for both looks, so the outer bars always stand lower than the middle ones.
                    let taper = 0.15 + 0.85 * pow(sin(.pi * x), 1.5)
                    let flow = 0.5 + 0.5 * sin(2 * .pi * (1.1 * x - t / 1300))
                    let ripple = 0.5 + 0.5 * cos(2 * .pi * (0.9 * abs(2 * x - 1) - t / 1800))
                    let level = (1 - turn) * flow + turn * ripple
                    Capsule()
                        .frame(width: 2, height: 3 + 13 * taper * level)
                        .opacity(0.5 + 0.5 * level)
                }
            }
            .frame(height: 28)
        }
        .foregroundStyle(Palette.pillText)
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
                Image(systemName: "globe").font(.system(size: 20))
                    .modifier(KeyFace(kind: .function))
                    .overlay { GlobeTouch(controller: model.controller) }
                    .frame(width: unit * 1.1)
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

/// A clear UIKit button over the globe key: iOS needs the touch events to switch on a tap and list the keyboards on a hold.
private struct GlobeTouch: UIViewRepresentable {
    let controller: KeyboardController?

    func makeUIView(context: Context) -> UIButton {
        let button = UIButton(type: .custom)
        button.accessibilityLabel = "Next keyboard"
        button.accessibilityIdentifier = "keyboard.next"
        if let controller {
            button.addTarget(controller, action: #selector(UIInputViewController.handleInputModeList(from:with:)), for: .allTouchEvents)
        }
        return button
    }

    func updateUIView(_ view: UIButton, context: Context) {}
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
