import Observation
import SwiftUI
import UIKit

@Observable
@MainActor
final class KeyboardModel {
    var status = "Ready"
    var url: URL?
    var canUndo = false
    var shift = false
    var symbols = false
    weak var controller: KeyboardController?

    func insert(_ text: String) { controller?.insert(text) }
    func backspace() { controller?.backspace() }
    func undo() { controller?.undo() }
    func cancel() { controller?.cancel() }
    func nextKeyboard() { controller?.advanceToNextInputMode() }
}

final class KeyboardController: UIInputViewController {
    private let model = KeyboardModel()
    private var store: FlowSessionStore?
    private var timer: Timer?
    private var insertion: (document: UUID, text: String, context: String, replaced: String)?

    override func viewDidLoad() {
        super.viewDidLoad()
        model.controller = self
        let host = UIHostingController(rootView: PlaceholderKeyboard(model: model))
        addChild(host)
        host.view.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(host.view)
        NSLayoutConstraint.activate([
            host.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            host.view.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            host.view.topAnchor.constraint(equalTo: view.topAnchor),
            host.view.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])
        host.didMove(toParent: self)
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        refresh()
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.refresh() }
        }
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        if hasFullAccess {
            UserDefaults(suiteName: FlowSessionStore.groupIdentifier)?.set(Date(), forKey: "keyboardLastSeen")
        }
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        timer?.invalidate()
        timer = nil
    }

    override func textDidChange(_ textInput: (any UITextInput)?) {
        super.textDidChange(textInput)
        refresh()
    }

    override func selectionDidChange(_ textInput: (any UITextInput)?) {
        super.selectionDidChange(textInput)
        refresh()
    }

    private var context: String {
        FlowSessionStore.context(
            before: textDocumentProxy.documentContextBeforeInput,
            after: textDocumentProxy.documentContextAfterInput,
            selection: textDocumentProxy.selectedText
        )
    }

    private func refresh() {
        guard hasFullAccess else {
            model.url = nil
            model.status = "Allow Full Access in Settings to use dictation."
            return
        }
        do {
            if store == nil { store = try FlowSessionStore() }
            guard let store else { return }
            let current: FlowSession?
            do {
                current = try store.current()
            } catch FlowSessionError.damaged {
                model.url = try store.begin(
                    documentIdentifier: textDocumentProxy.documentIdentifier, context: context
                ).url
                model.status = "The old session could not be read. Start dictation again."
                return
            }
            if let session = current {
                switch session.state {
                case .completed:
                    if let text = try store.consume(
                        id: session.id, documentIdentifier: textDocumentProxy.documentIdentifier,
                        context: context
                    ) {
                        let replaced = textDocumentProxy.selectedText ?? ""
                        textDocumentProxy.insertText(text)
                        insertion = (textDocumentProxy.documentIdentifier, text, context, replaced)
                        model.status = "Inserted"
                    } else {
                        model.status = "The text field changed. Start dictation again."
                    }
                case .recording:
                    model.url = nil
                    model.status = "Finish dictation in Hushpen, then return here."
                    return
                case .pending:
                    if session.documentIdentifier == textDocumentProxy.documentIdentifier,
                       session.context == context {
                        model.url = session.url
                        updateUndo()
                        return
                    }
                case .cancelled, .consumed:
                    break
                }
            }
            let session = try store.begin(
                documentIdentifier: textDocumentProxy.documentIdentifier, context: context
            )
            model.url = session.url
            updateUndo()
        } catch {
            model.url = nil
            model.status = error.localizedDescription
        }
    }

    func insert(_ text: String) {
        textDocumentProxy.insertText(model.shift ? text.uppercased() : text)
        model.shift = false
        insertion = nil
        refresh()
    }

    func backspace() {
        textDocumentProxy.deleteBackward()
        insertion = nil
        refresh()
    }

    func undo() {
        updateUndo()
        guard model.canUndo, let insertion else { return }
        for _ in insertion.text { textDocumentProxy.deleteBackward() }
        if !insertion.replaced.isEmpty { textDocumentProxy.insertText(insertion.replaced) }
        self.insertion = nil
        model.status = "Undone"
        refresh()
    }

    func cancel() {
        do {
            if let session = try store?.current() {
                try store?.cancel(id: session.id)
            }
            model.status = "Cancelled"
            refresh()
        } catch {
            model.status = error.localizedDescription
        }
    }

    private func updateUndo() {
        model.canUndo = insertion.map {
            $0.document == textDocumentProxy.documentIdentifier && $0.context == context
                && textDocumentProxy.documentContextBeforeInput?.hasSuffix($0.text) == true
        } ?? false
        if !model.canUndo { insertion = nil }
    }
}

struct PlaceholderKeyboard: View {
    @Bindable var model: KeyboardModel

    var body: some View {
        VStack {
            Text(model.status).accessibilityIdentifier("keyboard.status")
            if let url = model.url {
                Link("Microphone", destination: url)
                    .accessibilityIdentifier("keyboard.mic")
            }
            HStack {
                Button("Next keyboard", action: model.nextKeyboard)
                    .accessibilityIdentifier("keyboard.next")
                Button("Shift") { model.shift.toggle() }
                Button("Symbols") { model.symbols.toggle() }
                Button("Undo", action: model.undo).disabled(!model.canUndo)
                    .accessibilityIdentifier("keyboard.undo")
                Button("Cancel dictation", action: model.cancel)
            }
            ForEach(model.symbols ? ["1234567890", "-/:;()$&@\"", ".,?!'"] : ["qwertyuiop", "asdfghjkl", "zxcvbnm"], id: \.self) { row in
                HStack {
                    ForEach(Array(row).map(String.init), id: \.self) { key in
                        Button(model.shift ? key.uppercased() : key) { model.insert(key) }
                            .accessibilityIdentifier("keyboard.key.\(key)")
                    }
                }
            }
            HStack {
                Button("Space") { model.insert(" ") }
                    .accessibilityIdentifier("keyboard.space")
                Button("Backspace", action: model.backspace)
                    .accessibilityIdentifier("keyboard.backspace")
                Button("Return") { model.insert("\n") }
                    .accessibilityIdentifier("keyboard.return")
            }
        }
    }
}
