import SwiftUI

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
            Text(first).accessibilityIdentifier("editor.first.value")
            Text(second).accessibilityIdentifier("editor.second.value")
        }
    }
}
