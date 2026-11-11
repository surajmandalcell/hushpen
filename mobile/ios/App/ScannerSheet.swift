import SwiftUI

/// The sheet that reads the desktop's pairing code. Pairing itself stays on the Devices list.
struct ScannerSheet: View {
    @State private var model = ScannerModel()
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var phase
    let scanned: (String) -> Void
    let pasteLink: () -> Void

    private var denied: Bool { model.permission == "Denied" || model.permission == "Restricted" }

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
                            Button { model.openSettings() } label: { Label("Open Settings", systemImage: "gearshape") }
                        } else if !model.starting && !model.scanning {
                            Button { Task { await model.start() } } label: { Label("Try Again", systemImage: "arrow.clockwise") }
                        }
                        Button(action: pasteLink) { Label("Paste Link", systemImage: "link") }
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
                    Button(role: .close) { model.stop(); dismiss() }
                }
            }
        }
        .presentationDetents([.fraction(0.78), .large])
        .task { await model.start() }
        .onChange(of: model.value) { _, value in if !value.isEmpty { scanned(value) } }
        .onDisappear { model.stop() }
        .onChange(of: phase) { _, value in if value == .background { model.stop() } }
    }

    private var camera: some View {
        ZStack {
            Color.black
            CameraPreview(session: model.session)
            if model.scanning {
                ViewfinderCorners()
                    .stroke(.white, style: StrokeStyle(lineWidth: 4, lineCap: .round, lineJoin: .round))
                    .frame(width: 190, height: 190)
            } else if model.starting {
                ProgressView().tint(.white)
            } else {
                VStack(spacing: 10) {
                    Image(systemName: "video.slash").font(.largeTitle)
                    if !model.notice.isEmpty {
                        Text(model.notice).font(.subheadline).multilineTextAlignment(.center)
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
        .accessibilityLabel(model.starting ? "Camera starting" : model.scanning ? "Camera scanning" : "Camera stopped")
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
