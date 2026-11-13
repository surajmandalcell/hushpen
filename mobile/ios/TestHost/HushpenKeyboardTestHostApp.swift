import SwiftUI
import UIKit

@main
struct HushpenKeyboardTestHostApp: App {
    var body: some Scene {
        WindowGroup { PlaceholderEditor() }
    }
}

struct PlaceholderEditor: View {
    @State private var first = ""
    @State private var second = ""

    var body: some View {
        Form {
            TextField("First field", text: $first)
                .accessibilityIdentifier("editor.first")
            TextField("Second field", text: $second)
                .accessibilityIdentifier("editor.second")
            RefusingEditor()
                .frame(height: 32)
                .accessibilityIdentifier("editor.refusing")
            Text(first).accessibilityIdentifier("editor.first.value")
            Text(second).accessibilityIdentifier("editor.second.value")
        }
    }
}

/// An editor that takes keyboard focus but turns every change down, like a field with its own
/// input rules.
private struct RefusingEditor: UIViewRepresentable {
    func makeUIView(context: Context) -> UITextView {
        let view = UITextView()
        view.font = .preferredFont(forTextStyle: .body)
        view.backgroundColor = .clear
        view.isScrollEnabled = false
        view.delegate = context.coordinator
        return view
    }

    func updateUIView(_ view: UITextView, context: Context) {}

    func makeCoordinator() -> Refuser { Refuser() }

    final class Refuser: NSObject, UITextViewDelegate {
        func textView(_ textView: UITextView, shouldChangeTextIn range: NSRange, replacementText text: String) -> Bool { false }
    }
}
