import SwiftUI

/// Pair a Desktop: the camera reads the computer's QR; Type Code takes the 8-digit code.
struct PairSheet: View {
    let model: DevicesModel
    @State private var scanner = ScannerModel()
    @State private var typing = false
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var phase

    private var denied: Bool { scanner.permission == "Denied" || scanner.permission == "Restricted" }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 18) {
                    camera
                    Text("Point the camera at the code in Hushpen on your computer.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                    HStack(spacing: 12) {
                        if denied {
                            Button { scanner.openSettings() } label: { Label("Open Settings", systemImage: "gearshape") }
                        } else if !scanner.starting && !scanner.scanning {
                            Button { Task { await scanner.start() } } label: { Label("Try Again", systemImage: "arrow.clockwise") }
                        }
                        Button { typing = true } label: { Label("Type Code", systemImage: "number") }
                            .accessibilityIdentifier("scanner.type")
                    }
                    .buttonStyle(.glass)
                    .controlSize(.large)
                }
                .padding(.horizontal, 20)
                .padding(.bottom, 20)
            }
            .navigationTitle("Pair a Desktop")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(role: .close) { scanner.stop(); dismiss() }
                }
            }
            .navigationDestination(isPresented: $typing) { TypeCodeView(model: model, done: { scanner.stop(); dismiss() }) }
        }
        .presentationDetents([.fraction(0.78), .large])
        .task { await scanner.start() }
        .onChange(of: scanner.value) { _, value in
            guard !value.isEmpty else { return }
            scanner.stop()
            dismiss()
            Task { _ = await model.pair(payload: value) }
        }
        .onDisappear { scanner.stop() }
        .onChange(of: phase) { _, value in if value == .background { scanner.stop() } }
    }

    private var camera: some View {
        ZStack {
            Color.black
            CameraPreview(session: scanner.session)
            if scanner.scanning {
                ViewfinderCorners()
                    .stroke(.white, style: StrokeStyle(lineWidth: 4, lineCap: .round, lineJoin: .round))
                    .frame(width: 190, height: 190)
            } else if scanner.starting {
                ProgressView().tint(.white)
            } else {
                VStack(spacing: 10) {
                    Image(systemName: "video.slash").font(.largeTitle)
                    if !scanner.notice.isEmpty {
                        Text(scanner.notice).font(.subheadline).multilineTextAlignment(.center)
                    }
                }
                .foregroundStyle(.white.opacity(0.7))
                .padding(24)
            }
        }
        .frame(maxWidth: .infinity)
        .aspectRatio(1.1, contentMode: .fit)
        .clipShape(RoundedRectangle(cornerRadius: 30, style: .continuous))
        .accessibilityElement(children: .contain)
        .accessibilityLabel(scanner.starting ? "Camera starting" : scanner.scanning ? "Camera scanning" : "Camera stopped")
        .accessibilityIdentifier("scanner.state")
    }
}

private struct ViewfinderCorners: Shape {
    func path(in rect: CGRect) -> Path {
        let arm: CGFloat = 34
        let radius: CGFloat = 14
        var path = Path()
        for (x, y, sx, sy) in [(rect.minX, rect.minY, 1.0, 1.0), (rect.maxX, rect.minY, -1.0, 1.0),
                               (rect.maxX, rect.maxY, -1.0, -1.0), (rect.minX, rect.maxY, 1.0, -1.0)] {
            path.move(to: CGPoint(x: x, y: y + sy * arm))
            path.addLine(to: CGPoint(x: x, y: y + sy * radius))
            path.addQuadCurve(to: CGPoint(x: x + sx * radius, y: y), control: CGPoint(x: x, y: y))
            path.addLine(to: CGPoint(x: x + sx * arm, y: y))
        }
        return path
    }
}

/// The fallback for a computer whose QR cannot be scanned: its 8-digit code, and where to find it.
private struct TypeCodeView: View {
    let model: DevicesModel
    let done: () -> Void
    @State private var code = ""
    @State private var address = ""
    @State private var failure = ""
    @FocusState private var focused: Bool

    private var digits: String { code.filter(\.isNumber) }

    var body: some View {
        List {
            Section {
                LabeledContent("Code") {
                    TextField("0000 0000", text: $code)
                        .keyboardType(.numberPad)
                        .multilineTextAlignment(.trailing)
                        .font(.body.monospacedDigit())
                        .focused($focused)
                        .onChange(of: code) { _, value in
                            let d = String(value.filter(\.isNumber).prefix(8))
                            let spaced = d.count > 4 ? "\(d.prefix(4)) \(d.dropFirst(4))" : d
                            if spaced != value { code = spaced }
                        }
                        .accessibilityIdentifier("type.code")
                }
                LabeledContent("Address") {
                    TextField("Automatic", text: $address)
                        .multilineTextAlignment(.trailing)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.URL)
                        .accessibilityIdentifier("type.address")
                }
            } footer: {
                Text(failure.isEmpty
                    ? "Type the code shown in Hushpen on your computer. It works once, for 2 minutes. Leave Address on Automatic unless your computer is not found; then type host:port."
                    : failure)
                    .foregroundStyle(failure.isEmpty ? Color.secondary : Color.red)
            }
            Section {
                Button {
                    Task {
                        failure = ""
                        if await model.pair(code: digits, address: address) { done() } else { failure = model.notice }
                    }
                } label: {
                    HStack {
                        Text("Pair").font(.headline)
                        if model.busy { ProgressView() }
                    }
                    .frame(maxWidth: .infinity)
                }
                .disabled(digits.count != 8 || model.busy)
                .accessibilityIdentifier("type.pair")
            }
        }
        .navigationTitle("Type Code")
        .navigationBarTitleDisplayMode(.inline)
        .onAppear { focused = true }
    }
}

/// Both screens show this number while a computer pairs through Nearby. Pair on each, or Cancel.
struct NearbyPairSheet: View {
    let number: String
    let answer: (Bool) -> Void

    var body: some View {
        VStack(spacing: 14) {
            Text("Pair with this computer?").font(.title3.bold())
            Text("It will use the models on this iPhone. Check that your computer shows the same number.")
                .font(.subheadline).foregroundStyle(.secondary).multilineTextAlignment(.center)
            Text("\(number.prefix(3)) \(number.dropFirst(3))")
                .font(.system(size: 44, weight: .semibold, design: .rounded).monospacedDigit())
                .padding(.vertical, 8)
                .accessibilityIdentifier("pairing.number")
            HStack(spacing: 12) {
                Button { answer(false) } label: { Text("Cancel").frame(maxWidth: .infinity, minHeight: 36) }
                    .buttonStyle(.glass)
                    .accessibilityIdentifier("pairing.cancel")
                Button { answer(true) } label: { Text("Pair").bold().frame(maxWidth: .infinity, minHeight: 36) }
                    .buttonStyle(.glassProminent)
                    .accessibilityIdentifier("pairing.pair")
            }
            .controlSize(.large)
        }
        .padding(24)
        .presentationDetents([.height(340)])
        .interactiveDismissDisabled()
    }
}

extension View {
    /// Shows the Nearby comparison over this screen while a computer is pairing.
    func nearbyPairing(_ model: DevicesModel, enabled: Bool = true) -> some View {
        sheet(isPresented: Binding(get: { enabled && model.comparing != nil }, set: { if !$0 { model.answer(false) } })) {
            NearbyPairSheet(number: model.comparing ?? "000000", answer: model.answer)
        }
    }
}
