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

/// The system keyboard colors, and the gray tint of the strip's round glass buttons.
private enum Palette {
    static let letter = Color(light: rgb(0xFFFFFF), dark: rgb(0x6B6B6E))
    static let function = Color(light: rgb(0xABB0BC), dark: rgb(0x444447))
    static let keyShadow = Color(light: rgb(0x000000, alpha: 0.3), dark: rgb(0x000000, alpha: 0.45))
    static let calloutShadow = Color(light: rgb(0x000000, alpha: 0.28), dark: rgb(0x000000, alpha: 0.5))
    /// A letter key under the finger, as the system keyboard darkens it in Light and lifts it in Dark.
    static let letterPressed = Color(light: rgb(0xE5E5E5), dark: rgb(0x8E8E91))
    /// The ink of a control that cannot act now. A color, not a fade.
    static let dimInk = Color(uiColor: .tertiaryLabel)

    static let buttonTint = Color(light: rgb(0x3C3C43, alpha: 0.12), dark: rgb(0xFFFFFF, alpha: 0.16))
    static let blue = Color(uiColor: .systemBlue)
}

/// The system keyboard look with our dictation strip. A fixed 44 pt strip above the keys holds the
/// dictation hint and its states, so nothing moves when they change.
struct KeyboardView: View {
    @Bindable var model: KeyboardModel

    var body: some View {
        VStack(spacing: 0) {
            Band(model: model).frame(height: 44)
            // The callout of a top-row key rises into the strip, so the keys draw above it.
            Keys(model: model).zIndex(1)
        }
    }
}

// MARK: - Strip

private extension KeyboardPill {
    /// What identifies a look, so a new text of the same notice swaps the strip too.
    var key: String {
        switch self {
        case .idle: "idle"
        case .listening: "listening"
        case .transcribing: "transcribing"
        case .inserted: "inserted"
        case let .pasteFailed(copied): "pasteFailed.\(copied)"
        case let .notice(text): "notice.\(text)"
        }
    }

    var label: String {
        switch self {
        case .idle: "Ready"
        case .listening: "Listening"
        case .transcribing: "Transcribing"
        case .inserted: "Inserted"
        case .pasteFailed(copied: true): "Paste failed. Copied to clipboard."
        case .pasteFailed(copied: false): "Paste failed."
        case let .notice(text): text
        }
    }
}

/// Holds the strip of the current state. A new state fades the old strip out in 0.15 s while the
/// parts of the new one come in one by one (see `Part`).
private struct Band: View {
    @Bindable var model: KeyboardModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let pill = model.pill
        ZStack {
            Strip(pill: pill, model: model)
                .id(pill.key)
                .transition(reduceMotion ? .identity : .asymmetric(insertion: .identity, removal: .opacity.animation(.easeOut(duration: 0.15))))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .animation(reduceMotion ? nil : .easeOut(duration: 0.15), value: pill.key)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(pill.label)
        .accessibilityIdentifier("keyboard.status")
    }
}

/// Left part, middle part, right part. Cancel, Finish, Undo, and Copy are 28 pt round buttons that
/// stand outside the middle, with no shape around the set.
private struct Strip: View {
    let pill: KeyboardPill
    let model: KeyboardModel

    var body: some View {
        ZStack {
            Part(order: 1) { middle }
                .padding(.horizontal, 56)
            // Left-handed mirrors the whole strip: what stands on the right goes to the left.
            HStack(spacing: 0) {
                Part(order: 0) { if model.leftHanded { right } else { left } }
                Spacer(minLength: 0)
                Part(order: 2) { if model.leftHanded { left } else { right } }
            }
        }
    }

    /// At rest the left holds Undo and Redo of the keyboard's own edits; the hint is on the right.
    @ViewBuilder private var left: some View {
        switch pill {
        case .idle: edits
        case .listening:
            round("xmark", label: "Cancel dictation", id: "keyboard.cancel", action: model.cancel)
        default: EmptyView()
        }
    }

    @ViewBuilder private var middle: some View {
        switch pill {
        case .listening:
            ListeningMiddle(model: model)
        case .transcribing:
            HStack(spacing: 8) {
                Curve(level: Self.transcribingLevel).accessibilityHidden(true)
                text("Transcribing")
            }
        case .inserted:
            HStack(spacing: 6) {
                Image(systemName: "checkmark.circle.fill")
                    .font(.system(size: 17))
                    .foregroundStyle(Palette.blue)
                    .accessibilityHidden(true)
                text("Inserted")
            }
        case .pasteFailed(copied: true):
            text("Paste failed", sub: "Copied to clipboard")
        case .pasteFailed(copied: false):
            text("Paste failed")
        case let .notice(message):
            text(message)
        case .idle:
            EmptyView()
        }
    }

    @ViewBuilder private var right: some View {
        switch pill {
        case .idle: hint
        case .listening:
            round("checkmark", filled: true, label: "Finish dictation", id: "keyboard.finish", action: model.stop)
        case .inserted:
            round("arrow.uturn.backward", label: "Undo", id: "keyboard.undo", action: model.undo)
        case .pasteFailed(copied: true):
            round("xmark", label: "Dismiss", id: "keyboard.dismiss", action: model.dismiss)
        case .pasteFailed(copied: false):
            round("doc.on.doc", label: "Copy", id: "keyboard.copy", action: model.copy)
        default: EmptyView()
        }
    }

    /// Undo and Redo of the keyboard's own edits. The icons stand 14 pt apart; each hit area reaches
    /// 8 pt past its 28 pt box (44 pt), so the two areas overlap and the later one wins the overlap.
    private var edits: some View {
        HStack(spacing: Self.editGap - (Self.editBox - Self.editIcon)) {
            editButton("arrow.uturn.backward", label: "Undo typing", id: "keyboard.edit.undo", enabled: model.canUndoEdit, action: model.undoEdit)
            editButton("arrow.uturn.forward", label: "Redo typing", id: "keyboard.edit.redo", enabled: model.canRedoEdit, action: model.redoEdit)
        }
        .padding(.horizontal, 12)
    }

    private static let editIcon: CGFloat = 18
    private static let editGap: CGFloat = 14
    private static let editBox: CGFloat = 28
    /// Wave level while the app transcribes: low and steady.
    private static let transcribingLevel = 0.2

    /// Dimmed when there is nothing to do.
    private func editButton(_ symbol: String, label: String, id: String, enabled: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: Self.editIcon))
                .foregroundStyle(enabled ? Color.secondary : Palette.dimInk)
                .frame(width: Self.editBox, height: Self.editBox)
                // A plain button over a bare glyph takes no touch outside the strokes; this paints the area.
                .background(Color.black.opacity(0.001), in: Circle())
                .contentShape(Circle().inset(by: -8))
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
        .accessibilityLabel(label)
        .accessibilityIdentifier(id)
    }

    /// At rest: the whole hint is one tap target as tall as the strip. It starts dictation.
    @ViewBuilder private var hint: some View {
        if let url = model.url, model.fullAccess {
            Link(destination: url) { hintFace(ink: Color.secondary) }
                .buttonStyle(HintStyle(onPress: model.keyFeedback))
                .accessibilityLabel("Dictate")
                .accessibilityIdentifier("keyboard.mic")
        } else {
            hintFace(ink: Palette.dimInk)
                .accessibilityLabel("Dictation unavailable")
                .accessibilityIdentifier("keyboard.mic")
        }
    }

    private func hintFace(ink: Color) -> some View {
        HStack(spacing: 6) {
            Image(systemName: "mic.fill").font(.system(size: 17))
            Text("Tap to dictate").font(.system(size: 13))
        }
        .foregroundStyle(ink)
        .padding(.horizontal, 12)
        .frame(height: 44)
        .contentShape(Rectangle())
    }

    private func text(_ value: String, sub: String? = nil) -> some View {
        VStack(spacing: 0) {
            Text(value).font(.system(size: 13))
            if let sub { Text(sub).font(.system(size: 11)) }
        }
        .foregroundStyle(.secondary)
        .multilineTextAlignment(.center)
        .lineLimit(2)
        .minimumScaleFactor(0.8)
    }

    /// A 28 pt round button: gray glass, or the filled blue one for the main action.
    @ViewBuilder private func round(_ symbol: String, filled: Bool = false, label: String, id: String, action: @escaping () -> Void) -> some View {
        let face = Image(systemName: symbol)
            .font(.system(size: 13, weight: .bold))
            .foregroundStyle(filled ? Color.white : Color.primary)
            .frame(width: 28, height: 28)
        Button(action: action) {
            if filled {
                face.background(Palette.blue, in: Circle()).contentShape(Circle().inset(by: -8))
            } else {
                face.glassEffect(.regular.tint(Palette.buttonTint), in: Circle()).contentShape(Circle().inset(by: -8))
            }
        }
        .buttonStyle(.plain)
        .padding(.horizontal, 12)
        .accessibilityLabel(label)
        .accessibilityIdentifier(id)
    }
}

/// One part of a strip: it fades in and rises 4 pt over 0.18 s, ease-out, 0.05 s after the part before it.
/// With Reduce Motion it shows at once and stays still.
private struct Part<Content: View>: View {
    let order: Int
    @ViewBuilder let content: Content
    @State private var shown = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        content
            .opacity(shown ? 1 : 0)
            .offset(y: shown ? 0 : 4)
            .onAppear {
                if reduceMotion {
                    shown = true
                } else {
                    withAnimation(.easeOut(duration: 0.18).delay(0.05 * Double(order))) { shown = true }
                }
            }
    }
}

/// The middle of the strip while it listens. Kept apart so another look can replace it.
private struct ListeningMiddle: View {
    /// Space between the time and the curve.
    static let spacing: CGFloat = 20

    let model: KeyboardModel

    var body: some View {
        HStack(spacing: Self.spacing) {
            Elapsed(since: model.listeningSince)
            Curve(level: model.level).accessibilityHidden(true)
        }
    }
}

/// The time of the take, as 0:12, in digits of one width so the strip does not shift.
private struct Elapsed: View {
    let since: Date

    var body: some View {
        TimelineView(.periodic(from: since, by: 1)) { context in
            let seconds = max(0, Int(context.date.timeIntervalSince(since)))
            Text(String(format: "%d:%02d", seconds / 60, seconds % 60))
                .font(.system(size: 13).monospacedDigit())
                .foregroundStyle(.secondary)
        }
    }
}

/// Eases the shown level toward the latest reading, so a reading every 80 ms looks smooth.
private final class LevelSmoother {
    private var value = 0.0
    private var last: TimeInterval?

    func step(toward target: Double, at time: TimeInterval) -> Double {
        let elapsed = last.map { min(max(time - $0, 0), 0.1) } ?? 0.1
        last = time
        value += (target - value) * (1 - exp(-elapsed * 12))
        return value
    }
}

/// The press look of the hint: the system gives no click for a link, so the keyboard plays it.
private struct HintStyle: ButtonStyle {
    let onPress: () -> Void

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .onChange(of: configuration.isPressed) { _, down in if down { onPress() } }
    }
}

/// One blue line, 2 pt with round caps: a sine that travels left to right. Its height follows `level`
/// (0...1) and tapers (amplitude x sin(pi x)): flat at both ends, tallest in the middle.
/// With Reduce Motion the curve stands still.
private struct Curve: View {
    static let width: CGFloat = 96
    static let height: CGFloat = 28
    static let cycles = 2.5
    /// Cycles a second.
    static let speed = 0.8

    let level: Double
    @State private var smoother = LevelSmoother()
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        TimelineView(.animation(paused: reduceMotion)) { context in
            let time = reduceMotion ? 0 : context.date.timeIntervalSinceReferenceDate
            let shown = reduceMotion ? level : smoother.step(toward: level, at: time)
            let amount = 0.1 + 0.9 * shown
            Canvas { graphics, size in
                var path = Path()
                let steps = Int(size.width / 2)
                for step in 0...steps {
                    let x = Double(step) / Double(steps)
                    let wave = sin(2 * .pi * (Self.cycles * x - Self.speed * time))
                    let y = size.height / 2 - (size.height / 2 - 1) * amount * sin(.pi * x) * wave
                    let point = CGPoint(x: x * size.width, y: y)
                    if step == 0 { path.move(to: point) } else { path.addLine(to: point) }
                }
                graphics.stroke(path, with: .foreground, style: StrokeStyle(lineWidth: 2, lineCap: .round, lineJoin: .round))
            }
            .foregroundStyle(Palette.blue)
            .frame(width: Self.width, height: Self.height)
        }
    }
}

// MARK: - Keys
// Keys press and release at once: no fade, scale, or animation. The keys tests scan this section.

private enum KeyKind { case letter, function }

private struct KeyFrames: PreferenceKey {
    static let defaultValue: [KeyID: CGRect] = [:]

    static func reduce(value: inout [KeyID: CGRect], nextValue: () -> [KeyID: CGRect]) {
        value.merge(nextValue()) { $1 }
    }
}

private struct Keys: View {
    @Bindable var model: KeyboardModel

    private static let letters = ["qwertyuiop", "asdfghjkl", "zxcvbnm"]
    private static let symbols = ["1234567890", "-/:;()$&@\"", ".,?!'"]

    var body: some View {
        GeometryReader { proxy in
            let unit = proxy.size.width / 10
            let rows = model.symbols ? Self.symbols : Self.letters
            if model.emoji {
                VStack(spacing: 0) {
                    EmojiGrid(model: model).frame(height: 162)
                    emojiBottomRow(unit: unit)
                }
            } else {
                VStack(spacing: 0) {
                    row(rows[0])
                    row(rows[1]).padding(.horizontal, model.symbols ? 0 : unit / 2)
                    thirdRow(rows[2], unit: unit)
                    bottomRow(unit: unit)
                }
            }
        }
        .frame(height: 216)
        .coordinateSpace(name: "keys")
        .onPreferenceChange(KeyFrames.self) { frames in
            MainActor.assumeIsolated { model.keys.frames = frames }
        }
        .overlay { KeyTouchLayer(tracker: model.keys).accessibilityHidden(true) }
        .overlay { callouts }
        .transaction { $0.animation = nil }
    }

    /// One key: its face, its cell for touches, and its accessibility element. The touch layer reads
    /// the cell frames; a VoiceOver activation types the key the same way.
    private func key<Content: View>(
        _ id: KeyID, kind: KeyKind, name: String, identifier: String, width: CGFloat? = nil,
        @ViewBuilder content: () -> Content
    ) -> some View {
        return content()
            .modifier(KeyFace(kind: kind, pressed: model.keys.pressed.contains(id)))
            .frame(width: width)
            .background {
                GeometryReader { proxy in
                    Color.clear.preference(key: KeyFrames.self, value: [id: proxy.frame(in: .named("keys"))])
                }
            }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(name)
            .accessibilityIdentifier(identifier)
            .accessibilityAddTraits([.isButton, .isKeyboardKey])
            .accessibilityAction {
                model.keyFeedback()
                model.press(id)
            }
    }

    private func row(_ characters: String) -> some View {
        HStack(spacing: 0) {
            ForEach(Array(characters).map(String.init), id: \.self) { character in letter(character) }
        }
    }

    private func letter(_ character: String) -> some View {
        key(.character(character), kind: .letter, name: character, identifier: "keyboard.key.\(character)") {
            Text(model.shift ? character.uppercased() : character).font(.system(size: 24))
        }
    }

    private func thirdRow(_ characters: String, unit: CGFloat) -> some View {
        HStack(spacing: 0) {
            if model.symbols {
                Color.clear.frame(width: unit * 1.45)
            } else {
                key(.shift, kind: model.shift ? .letter : .function, name: "Shift", identifier: "keyboard.shift", width: unit * 1.23) {
                    Image(systemName: model.shift ? "shift.fill" : "shift").font(.system(size: 20))
                }
                Color.clear.frame(width: unit * 0.27)
            }
            ForEach(Array(characters).map(String.init), id: \.self) { character in letter(character) }
            Color.clear.frame(width: unit * 0.27)
            backspace(unit: unit)
        }
    }

    private func backspace(unit: CGFloat) -> some View {
        key(.backspace, kind: .function, name: "Delete", identifier: "keyboard.backspace", width: unit * 1.23) {
            Image(systemName: "delete.left").font(.system(size: 20))
        }
    }

    /// The globe shows only when iOS asks for one: a tap switches keyboard, a hold lists them.
    @ViewBuilder private func globe(unit: CGFloat) -> some View {
        if model.needsGlobe {
            Image(systemName: "globe").font(.system(size: 20))
                .modifier(KeyFace(kind: .function))
                .overlay { GlobeTouch(controller: model.controller) }
                .frame(width: unit * 1.1)
        }
    }

    private func bottomRow(unit: CGFloat) -> some View {
        HStack(spacing: 0) {
            key(.symbols, kind: .function, name: model.symbols ? "ABC" : "123", identifier: "keyboard.symbols", width: unit * 1.25) {
                Text(model.symbols ? "ABC" : "123").font(.system(size: 16))
            }
            globe(unit: unit)
            key(.emoji, kind: .function, name: "Emojis", identifier: "keyboard.emoji", width: unit * 1.1) {
                Image(systemName: "face.smiling").font(.system(size: 20))
            }
            key(.space, kind: .letter, name: "space", identifier: "keyboard.space") {
                Text("space").font(.system(size: 16))
            }
            key(.period, kind: .letter, name: "period", identifier: "keyboard.period", width: unit) {
                Text(".").font(.system(size: 24))
            }
            key(.enter, kind: .function, name: "return", identifier: "keyboard.return", width: unit * 2.1) {
                Text("return").font(.system(size: 16))
            }
        }
    }

    /// The emoji panel keeps ABC (back to the letters), the globe, and delete under the grid.
    private func emojiBottomRow(unit: CGFloat) -> some View {
        HStack(spacing: 0) {
            key(.emoji, kind: .function, name: "ABC", identifier: "keyboard.emoji", width: unit * 1.25) {
                Text("ABC").font(.system(size: 16))
            }
            globe(unit: unit)
            Spacer(minLength: 0)
            backspace(unit: unit)
        }
    }

    /// The key callouts above the fingers that are down on a letter or symbol.
    private var callouts: some View {
        GeometryReader { proxy in
            ForEach(Array(model.keys.pressed.filter(\.showsCallout)), id: \.self) { id in
                if case let .character(character) = id, let cell = model.keys.frames[id] {
                    Callout(text: model.shift ? character.uppercased() : character, face: KeyFace.face(of: cell), width: proxy.size.width)
                }
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// Common emoji in a scrolling grid. A tap types the emoji at the cursor.
private struct EmojiGrid: View {
    let model: KeyboardModel

    private static let columns = Array(repeating: GridItem(.flexible(), spacing: 0), count: 8)
    private static let all: [String] = {
        let text = "😀😃😄😁😆😅😂🤣😊😇🙂😉😍🥰😘😋😛😜🤪😎🤩🥳😏😒😞😔😟😕🙁😣😖😫😩🥺😢😭😤😠😡🤬🤯😳🥵🥶😱😨😰😥😓🤗🤔🤭🤫😶😐😑😬🙄😯😴🤤😷🤒🤕🤢🤮🥴😵🤠🤓🧐👍👎👌🤞🤟🤘👈👉👆👇👋🤚✋👏🙌🤝🙏💪❤🧡💛💚💙💜🖤🤍💔💕💖💯🔥✨⭐🎉🎊🎁🏆🍕🍔🍟🌮🍎🍌🍓🍺☕🌞🌙🌈🐶🐱🐭🐻🐼🦁🐸🐵🐔🦄🌹🌻🚗🏠📱💻🎵⚽🏀"
        var seen = Set<Character>()
        return text.filter { seen.insert($0).inserted }.map(String.init)
    }()

    var body: some View {
        ScrollView {
            LazyVGrid(columns: Self.columns, spacing: 0) {
                ForEach(Self.all, id: \.self) { emoji in
                    Button {
                        model.keyFeedback()
                        model.insert(emoji)
                    } label: {
                        Text(emoji).font(.system(size: 30)).frame(maxWidth: .infinity, minHeight: 44)
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("keyboard.emoji.\(emoji)")
                }
            }
            .padding(.horizontal, 4)
        }
        .scrollIndicators(.hidden)
        .scrollEdgeEffectHidden(true, for: .all)
    }
}

/// Clear UIKit view over the keys. It sees every finger at touch-down and touch-up, which a SwiftUI
/// button reports late, and it follows several fingers at once. Points outside the key cells (the
/// globe and the emoji grid) fall through to the controls underneath.
private struct KeyTouchLayer: UIViewRepresentable {
    let tracker: KeyTracker

    func makeUIView(context: Context) -> KeyTouchView { KeyTouchView(tracker: tracker) }

    func updateUIView(_ view: KeyTouchView, context: Context) {}
}

private final class KeyTouchView: UIView {
    private let tracker: KeyTracker

    init(tracker: KeyTracker) {
        self.tracker = tracker
        super.init(frame: .zero)
        isMultipleTouchEnabled = true
        backgroundColor = .clear
        isAccessibilityElement = false
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    override func point(inside point: CGPoint, with event: UIEvent?) -> Bool {
        tracker.key(at: point) != nil
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches { tracker.down(token(touch), at: touch.location(in: self)) }
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches { tracker.move(token(touch), to: touch.location(in: self)) }
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches { tracker.up(token(touch), at: touch.location(in: self)) }
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches { tracker.cancel(token(touch)) }
    }

    private func token(_ touch: UITouch) -> Int { ObjectIdentifier(touch).hashValue }
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
            button.addAction(UIAction { [weak controller] _ in controller?.keyFeedback() }, for: .touchDown)
        }
        return button
    }

    func updateUIView(_ view: UIButton, context: Context) {}
}

/// A key: 43 pt tall in a 54 pt row, 8 pt corners, a one point edge below. Pressed swaps the shade,
/// the way the system keyboard darkens a light key and lightens a dark one.
private struct KeyFace: ViewModifier {
    static let sideInset: CGFloat = 3
    static let verticalInset: CGFloat = 5.5

    let kind: KeyKind
    var pressed = false
    var ink = Color.primary

    /// The painted key inside its touch cell.
    static func face(of cell: CGRect) -> CGRect {
        cell.insetBy(dx: sideInset, dy: verticalInset)
    }

    func body(content: Content) -> some View {
        content
            .foregroundStyle(ink)
            .frame(maxWidth: .infinity, minHeight: 43, maxHeight: 43)
            .background(fill, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            .shadow(color: Palette.keyShadow, radius: 0, y: 1)
            .padding(.horizontal, Self.sideInset)
            .padding(.vertical, Self.verticalInset)
            .contentShape(Rectangle())
    }

    private var fill: Color {
        switch kind {
        case .letter: pressed ? Palette.letterPressed : Palette.letter
        case .function: pressed ? Palette.letter : Palette.function
        }
    }
}

// MARK: - Callout

/// The system key callout (iOS 26 and later): a rounded square with the glyph, a few points above
/// the key, shown from touch-down to touch-up. The system draws it taller (63 pt) over a key row
/// with the whole screen above it; here it must fit the 44 pt strip above the top row, so every
/// row gets the same 46 pt bubble. A key at the screen edge keeps the bubble inside the keyboard.
private struct Callout: View {
    static let gap: CGFloat = 3
    static let height: CGFloat = 46

    let text: String
    let face: CGRect
    let width: CGFloat

    var body: some View {
        let bubble = face.width * 1.6
        let left = min(max(face.midX - bubble / 2, 2), width - bubble - 2)
        RoundedRectangle(cornerRadius: 14, style: .continuous)
            .fill(Palette.letter)
            .shadow(color: Palette.calloutShadow, radius: 4, y: 1)
            .overlay { Text(text).font(.system(size: 30)).foregroundStyle(Color.primary) }
            .frame(width: bubble, height: Self.height)
            .position(x: left + bubble / 2, y: face.minY - Self.gap - Self.height / 2)
    }
}
