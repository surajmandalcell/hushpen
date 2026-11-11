import Observation
import SwiftUI
import UIKit

@Observable
@MainActor
final class KeyboardModel {
    var status = "Ready"
    var url: URL?
    var canUndo = false
    var canInsert = false
    var canStop = false
    var recording = false
    var fullAccess = true
    var needsGlobe = false
    var shift = false
    var symbols = false
    weak var controller: KeyboardController?

    func insert(_ text: String) { controller?.insert(text) }
    func insertResult() { controller?.insertResult() }
    func backspace() { controller?.backspace() }
    func undo() { controller?.undo() }
    func cancel() { controller?.cancel() }
    func stop() { controller?.stop() }
    func nextKeyboard() { controller?.advanceToNextInputMode() }
}

final class KeyboardController: UIInputViewController {
    private let model = KeyboardModel()
    private var store: FlowSessionStore?
    private var observation: FlowSessionObservation?
    private var visible = false
    private var hostActive = true
    private var offer: (session: UUID, document: UUID, context: String)?
    private var insertion: (document: UUID, text: String, context: String, replaced: String)?

    override func viewDidLoad() {
        super.viewDidLoad()
        model.controller = self
        NotificationCenter.default.addObserver(self, selector: #selector(pauseHost), name: .NSExtensionHostWillResignActive, object: extensionContext)
        NotificationCenter.default.addObserver(self, selector: #selector(pauseHost), name: .NSExtensionHostDidEnterBackground, object: extensionContext)
        NotificationCenter.default.addObserver(self, selector: #selector(resumeHost), name: .NSExtensionHostDidBecomeActive, object: extensionContext)
        let host = UIHostingController(rootView: KeyboardView(model: model))
        host.view.backgroundColor = .clear
        host.safeAreaRegions = []
        addChild(host)
        host.view.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(host.view)
        // The voice bar (48) sits above the four key rows (216).
        let height = view.heightAnchor.constraint(equalToConstant: 264)
        height.priority = UILayoutPriority(999)
        NSLayoutConstraint.activate([
            height,
            host.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            host.view.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            host.view.topAnchor.constraint(equalTo: view.topAnchor),
            host.view.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])
        host.didMove(toParent: self)
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        visible = true
        model.needsGlobe = needsInputModeSwitchKey
        startRefreshing()
    }

    private func startRefreshing() {
        guard hostActive, visible else { return }
        do {
            if observation == nil {
                observation = try FlowSessionObservation { [weak self] in
                    Task { @MainActor in self?.refresh() }
                }
            }
        } catch { model.status = error.localizedDescription }
        refresh()
    }

    @objc private func pauseHost() {
        hostActive = false
        offer = nil
        model.canInsert = false
        model.canStop = false
        model.recording = false
        observation = nil
    }

    @objc private func resumeHost() {
        hostActive = true
        if viewIfLoaded?.window != nil { startRefreshing() }
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        if hasFullAccess {
            UserDefaults(suiteName: FlowSessionStore.groupIdentifier)?.set(Date(), forKey: "keyboardLastSeen")
        }
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        visible = false
        observation = nil
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

    private func documentIdentifier(for proxy: any UITextDocumentProxy) -> UUID? {
        // UIKit can return nil for this required property while input fields change.
        (proxy as? NSObject)?.value(forKey: "documentIdentifier") as? UUID
    }

    private func refresh() {
        guard hostActive, visible else { return }
        model.canStop = false
        model.recording = false
        model.fullAccess = hasFullAccess
        guard hasFullAccess else {
            model.url = nil
            model.canInsert = false
            model.status = "Allow Full Access in Settings to use dictation."
            return
        }
        guard let document = documentIdentifier(for: textDocumentProxy) else {
            model.url = nil
            model.canInsert = false
            model.canUndo = false
            return
        }
        do {
            if store == nil { store = try FlowSessionStore() }
            guard let store else { return }
            let current: FlowSession?
            do {
                current = try store.current()
            } catch FlowSessionError.damaged {
                offer = nil
                model.canInsert = false
                model.url = try store.begin(
                    documentIdentifier: document, context: context
                ).url
                model.status = "The old session could not be read. Start dictation again."
                return
            }
            if current?.state != .completed {
                offer = nil
                model.canInsert = false
            }
            if let session = current {
                switch session.state {
                case .completed:
                    let cursor = context
                    let sameOffer = offer.map { $0.session == session.id && $0.document == document && $0.context == cursor } ?? true
                    if session.context != cursor || !sameOffer {
                        try store.cancel(id: session.id)
                        offer = nil
                        model.canInsert = false
                        model.status = "The text field changed. Start dictation again."
                        break
                    }
                    offer = (session.id, document, cursor)
                    model.canInsert = true
                    model.url = nil
                    model.status = "Dictation is ready. Tap Insert dictation."
                    updateUndo()
                    return
                case .recording:
                    model.url = nil
                    model.recording = true
                    model.canStop = session.command != .stop
                    model.status = "Finish dictation in Hushpen, then return here."
                    return
                case .pending:
                    if session.documentIdentifier == document,
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
                documentIdentifier: document, context: context
            )
            model.url = session.url
            updateUndo()
        } catch {
            model.url = nil
            model.status = error.localizedDescription
        }
    }

    func insertResult() {
        guard hostActive, let offer, let store else { return }
        let proxy = textDocumentProxy
        let cursor = context
        guard let document = documentIdentifier(for: proxy), offer.document == document,
              offer.context == cursor else { refresh(); return }
        do {
            try store.confirmDestination(id: offer.session, documentIdentifier: offer.document, context: cursor)
            guard let text = try store.consume(id: offer.session, documentIdentifier: offer.document, context: cursor)
            else { throw FlowSessionError.stale }
            let replaced = proxy.selectedText ?? ""
            proxy.insertText(text)
            insertion = (document, text, context, replaced)
            self.offer = nil
            model.canInsert = false
            model.status = "Inserted"
            refresh()
        } catch { model.status = error.localizedDescription }
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

    func stop() {
        guard hostActive, visible else { return }
        do {
            if let session = try store?.current() { try store?.stop(id: session.id) }
            refresh()
        } catch { model.status = error.localizedDescription }
    }

    private func updateUndo() {
        guard let document = documentIdentifier(for: textDocumentProxy) else { model.canUndo = false; return }
        model.canUndo = insertion.map {
            $0.document == document && $0.context == context
                && textDocumentProxy.documentContextBeforeInput?.hasSuffix($0.text) == true
        } ?? false
        if !model.canUndo { insertion = nil }
    }
}
