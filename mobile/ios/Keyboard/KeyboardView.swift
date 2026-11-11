import SwiftUI
import UIKit

private extension Color {
    init(light: UInt32, dark: UInt32) {
        func color(_ value: UInt32) -> UIColor {
            UIColor(red: CGFloat((value >> 16) & 0xFF) / 255, green: CGFloat((value >> 8) & 0xFF) / 255, blue: CGFloat(value & 0xFF) / 255, alpha: 1)
        }
        self.init(uiColor: UIColor { $0.userInterfaceStyle == .dark ? color(dark) : color(light) })
    }

    init(hex: UInt32) { self.init(light: hex, dark: hex) }
}

private enum Palette {
    static let letter = Color(light: 0xFFFFFF, dark: 0x5C5C60)
    static let function = Color(light: 0xC6CAD2, dark: 0x3D3D41)
    static let hud = Color(hex: 0x1F1F1F)
    static let hudEdge = Color(hex: 0x353535)
    static let hudButton = Color(hex: 0x3A3A3C)
}

/// The keyboard: a voice bar above Gboard-style keys. The mic key starts a flow session in the app.
struct KeyboardView: View {
    @Bindable var model: KeyboardModel

    var body: some View {
        VStack(spacing: 0) {
            VoiceBar(model: model).frame(height: 48)
            Keys(model: model).zIndex(1)
        }
    }
}

// MARK: - Voice bar

private struct VoiceBar: View {
    @Bindable var model: KeyboardModel

    var body: some View {
        HStack(spacing: 8) {
            leading.frame(width: 44)
            center
                .frame(maxWidth: .infinity)
                .accessibilityElement(children: .contain)
                .accessibilityIdentifier("keyboard.status")
            trailing.frame(width: 44)
        }
        .padding(.horizontal, 6)
    }

    @ViewBuilder private var leading: some View {
        if model.canInsert && !model.canStop {
            BarButton(symbol: "xmark", label: "Cancel dictation", id: "keyboard.cancel", action: model.cancel)
        }
    }

    @ViewBuilder private var trailing: some View {
        if !model.canStop && !model.recording {
            BarButton(symbol: "arrow.uturn.backward", label: "Undo", id: "keyboard.undo", action: model.undo)
                .disabled(!model.canUndo)
                .opacity(model.canUndo ? 1 : 0.35)
        }
    }

    @ViewBuilder private var center: some View {
        if model.canStop {
            ListeningPill(model: model)
        } else if model.recording {
            WorkingPill()
        } else if model.canInsert {
            Button(action: model.insertResult) {
                Label("Insert dictation", systemImage: "text.insert")
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(.white)
                    .padding(.horizontal, 16)
                    .frame(height: 36)
                    .background(Color.accentColor, in: Capsule())
                    .contentShape(Capsule().inset(by: -4))
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("keyboard.insert")
        } else {
            Text(message)
                .font(.system(size: 14))
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .lineLimit(2)
                .minimumScaleFactor(0.85)
        }
    }

    private var message: String {
        model.status == "Ready" ? "Tap the mic to dictate" : model.status
    }
}

private struct BarButton: View {
    let symbol: String
    let label: String
    let id: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(.primary)
                .frame(width: 36, height: 36)
                .glassEffect(.regular.interactive(), in: .circle)
                .frame(width: 44, height: 44)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
        .accessibilityIdentifier(id)
    }
}

/// The desktop flow bar, in the same dark capsule: cancel, the wave, and finish.
private struct ListeningPill: View {
    @Bindable var model: KeyboardModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        HStack(spacing: 10) {
            round("xmark", fill: Palette.hudButton, ink: .white, label: "Cancel dictation", id: "keyboard.cancel", action: model.cancel)
            HStack(spacing: 6) {
                Image(systemName: "waveform")
                    .symbolEffect(.variableColor.iterative.dimInactiveLayers.nonReversing, options: .repeating, isActive: !reduceMotion)
                    .foregroundStyle(Color.accentColor)
                Text("Listening in Hushpen")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(.white)
                    .lineLimit(1)
            }
            .accessibilityElement(children: .combine)
            round("checkmark", fill: .white, ink: Palette.hud, label: "Finish dictation", id: "keyboard.finish", action: model.stop)
        }
        .padding(4)
        .background(Palette.hud, in: Capsule())
        .overlay(Capsule().strokeBorder(Palette.hudEdge, lineWidth: 0.5))
    }

    private func round(_ symbol: String, fill: Color, ink: Color, label: String, id: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 13, weight: .bold))
                .foregroundStyle(ink)
                .frame(width: 28, height: 28)
                .background(fill, in: Circle())
                .contentShape(Circle().inset(by: -8))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
        .accessibilityIdentifier(id)
    }
}

/// After finish is tapped the app transcribes; the pill keeps the bar from going blank.
private struct WorkingPill: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: "waveform")
                .symbolEffect(.pulse, options: .repeating, isActive: !reduceMotion)
                .foregroundStyle(Color.accentColor)
            Text("Finishing in Hushpen")
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(.white)
                .lineLimit(1)
        }
        .padding(.horizontal, 14)
        .frame(height: 36)
        .background(Palette.hud, in: Capsule())
        .overlay(Capsule().strokeBorder(Palette.hudEdge, lineWidth: 0.5))
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
                    Image(systemName: model.shift ? "shift.fill" : "shift").font(.system(size: 18, weight: .medium))
                }
                .buttonStyle(KeyStyle(kind: model.shift ? .letter : .function))
                .frame(width: unit * 1.2)
                .accessibilityLabel("Shift")
                .accessibilityIdentifier("keyboard.shift")
                Color.clear.frame(width: unit * 0.25)
            }
            ForEach(Array(characters).map(String.init), id: \.self) { character in letter(character) }
            Color.clear.frame(width: unit * 0.25)
            Button(action: model.backspace) {
                Image(systemName: "delete.left").font(.system(size: 18, weight: .medium))
            }
            .buttonStyle(KeyStyle(kind: .function))
            .frame(width: unit * 1.2)
            .accessibilityLabel("Delete")
            .accessibilityIdentifier("keyboard.backspace")
        }
    }

    private func bottomRow(unit: CGFloat) -> some View {
        HStack(spacing: 0) {
            Button { model.symbols.toggle() } label: {
                Text(model.symbols ? "ABC" : "123").font(.system(size: 16, weight: .medium))
            }
            .buttonStyle(KeyStyle(kind: .function))
            .frame(width: unit * 1.25)
            .accessibilityIdentifier("keyboard.symbols")
            if model.needsGlobe {
                Button(action: model.nextKeyboard) {
                    Image(systemName: "globe").font(.system(size: 18, weight: .medium))
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
                Text("return").font(.system(size: 16, weight: .medium))
            }
            .buttonStyle(KeyStyle(kind: .function))
            .frame(width: unit * 1.9)
            .accessibilityIdentifier("keyboard.return")
        }
    }

    @ViewBuilder private var mic: some View {
        if model.canStop {
            Button(action: model.stop) {
                Image(systemName: "stop.fill").font(.system(size: 18, weight: .medium))
            }
            .buttonStyle(KeyStyle(kind: .accent))
            .accessibilityLabel("Finish dictation")
            .accessibilityIdentifier("keyboard.mic")
        } else if let url = model.url, model.fullAccess {
            Link(destination: url) {
                Image(systemName: "mic.fill").font(.system(size: 19, weight: .medium))
                    .frame(maxWidth: .infinity, minHeight: 45, maxHeight: 45)
                    .foregroundStyle(.white)
                    .background(Color.accentColor, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                    .shadow(color: .black.opacity(0.22), radius: 0, y: 1)
                    .padding(.horizontal, 3)
                    .padding(.vertical, 4.5)
                    .contentShape(Rectangle())
            }
            .accessibilityLabel("Dictate")
            .accessibilityIdentifier("keyboard.mic")
        } else {
            Image(systemName: "mic.fill").font(.system(size: 19, weight: .medium))
                .frame(maxWidth: .infinity, minHeight: 45, maxHeight: 45)
                .foregroundStyle(.secondary)
                .background(Palette.function, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                .padding(.horizontal, 3)
                .padding(.vertical, 4.5)
                .opacity(0.7)
                .accessibilityLabel("Dictation unavailable")
        }
    }
}

private struct KeyStyle: ButtonStyle {
    enum Kind { case letter, function, accent }

    let kind: Kind
    var preview: String?

    func makeBody(configuration: Configuration) -> some View {
        let pressed = configuration.isPressed
        configuration.label
            .foregroundStyle(kind == .accent ? Color.white : Color.primary)
            .frame(maxWidth: .infinity, minHeight: 45, maxHeight: 45)
            .background(fill(pressed: pressed), in: RoundedRectangle(cornerRadius: 9, style: .continuous))
            .shadow(color: .black.opacity(0.22), radius: 0, y: 1)
            .padding(.horizontal, 3)
            .padding(.vertical, 4.5)
            .contentShape(Rectangle())
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

    private func fill(pressed: Bool) -> Color {
        switch kind {
        case .letter: pressed ? Palette.function : Palette.letter
        case .function: pressed ? Palette.letter : Palette.function
        case .accent: Color.accentColor.opacity(pressed ? 0.75 : 1)
        }
    }
}
