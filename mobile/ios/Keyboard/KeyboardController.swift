import Observation
import SwiftUI
import UIKit

/// What the 44 pt band above the keys shows. `idle` shows nothing.
enum KeyboardPill: Equatable {
    case idle, listening, transcribing, inserted
    case pasteFailed(copied: Bool)
    case notice(String)
}

@Observable
@MainActor
final class KeyboardModel {
    var pill = KeyboardPill.idle
    var url: URL?
    var fullAccess = true
    var needsGlobe = false
    var shift = false
    var symbols = false
    weak var controller: KeyboardController?

    func insert(_ text: String) { controller?.insert(text) }
    func backspace() { controller?.backspace() }
    func undo() { controller?.undo() }
    func cancel() { controller?.cancel() }
    func stop() { controller?.stop() }
    func copy() { controller?.copyRescuedText() }
    func dismiss() { controller?.dismissFailure() }
}

final class KeyboardController: UIInputViewController {
    private let model = KeyboardModel()
    private var store: FlowSessionStore?
    private var observation: FlowSessionObservation?
    private var visible = false
    private var hostActive = true
    private var delivering = false
    private var insertion: (document: UUID?, text: String, context: String, replaced: String)?
    private var rescued: String?
    private var expiry: Task<Void, Never>?
    private var heartbeat: KeyboardHeartbeat?
    /// Counts the host's text and cursor notices, so a cursor move shows whether the host answered.
    private var contextEvents = 0

    override func viewDidLoad() {
        super.viewDidLoad()
        model.controller = self
        // The mic key is our dictation key, so iOS must not draw its own below the keyboard.
        hasDictationKey = true
        shareStatus()
        NotificationCenter.default.addObserver(self, selector: #selector(pauseHost), name: .NSExtensionHostWillResignActive, object: extensionContext)
        NotificationCenter.default.addObserver(self, selector: #selector(pauseHost), name: .NSExtensionHostDidEnterBackground, object: extensionContext)
        NotificationCenter.default.addObserver(self, selector: #selector(resumeHost), name: .NSExtensionHostDidBecomeActive, object: extensionContext)
        let host = UIHostingController(rootView: KeyboardView(model: model))
        host.view.backgroundColor = .clear
        host.safeAreaRegions = []
        addChild(host)
        host.view.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(host.view)
        // The pill band (44) sits above the four key rows (216).
        let height = view.heightAnchor.constraint(equalToConstant: 260)
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
        } catch { model.pill = .notice(error.localizedDescription) }
        refresh()
    }

    @objc private func pauseHost() {
        hostActive = false
        if model.pill == .listening || model.pill == .transcribing { model.pill = .idle }
        observation = nil
    }

    @objc private func resumeHost() {
        hostActive = true
        if viewIfLoaded?.window != nil { startRefreshing() }
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        shareStatus()
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        visible = false
        observation = nil
    }

    override func textDidChange(_ textInput: (any UITextInput)?) {
        super.textDidChange(textInput)
        contextEvents += 1
        refresh()
    }

    override func selectionDidChange(_ textInput: (any UITextInput)?) {
        super.selectionDidChange(textInput)
        contextEvents += 1
        refresh()
    }

    private var proxyContext: InsertionCheck.Context {
        InsertionCheck.Context(
            before: textDocumentProxy.documentContextBeforeInput,
            after: textDocumentProxy.documentContextAfterInput,
            selected: textDocumentProxy.selectedText
        )
    }

    private var context: String {
        FlowSessionStore.context(
            before: textDocumentProxy.documentContextBeforeInput,
            after: textDocumentProxy.documentContextAfterInput,
            selection: textDocumentProxy.selectedText
        )
    }

    private var document: UUID? {
        // UIKit can return nil for this required property while input fields change.
        (textDocumentProxy as? NSObject)?.value(forKey: "documentIdentifier") as? UUID
    }

    private func refresh() {
        guard hostActive, visible else { return }
        shareStatus()
        model.fullAccess = hasFullAccess
        guard hasFullAccess else {
            model.url = nil
            model.pill = .notice("Turn on Full Access to dictate.")
            return
        }
        do {
            if store == nil { store = try FlowSessionStore() }
            guard let store else { return }
            var damaged = false
            var current: FlowSession?
            do { current = try store.current() } catch FlowSessionError.damaged { damaged = true }
            if let current {
                switch current.state {
                case .recording:
                    rescued = nil
                    model.url = nil
                    model.pill = current.command == .stop ? .transcribing : .listening
                    return
                case .pending:
                    model.url = current.url
                    settle()
                    return
                case .completed:
                    try deliver(current.id, from: store)
                case .cancelled, .consumed:
                    break
                }
            }
            model.url = try store.begin().url
            settle()
            if damaged { model.pill = .notice("The last dictation could not be read. Try again.") }
        } catch {
            model.url = nil
            model.pill = .notice(error.localizedDescription)
        }
    }

    /// Drops a pill that no longer matches the session. A failure stays until it is dismissed.
    private func settle() {
        switch model.pill {
        case .listening, .notice: model.pill = .idle
        case .transcribing where !delivering: model.pill = .idle
        case .inserted where !undoAvailable: model.pill = .idle
        default: break
        }
    }

    private func shareStatus() {
        let value = KeyboardHeartbeat(seenAt: Date(), hasFullAccess: hasFullAccess)
        if let heartbeat, heartbeat.hasFullAccess == value.hasFullAccess,
           value.seenAt.timeIntervalSince(heartbeat.seenAt) < 5 { return }
        do {
            try value.write(root: KeyboardStatus.groupRoot)
            heartbeat = value
        } catch {
            // iOS denies shared-container writes while Full Access is off.
            if hasFullAccess { model.pill = .notice("Keyboard status could not be shared. Reopen the keyboard.") }
        }
    }

    /// Inserts finished text at the cursor the field has now. The session is consumed first so a
    /// second refresh cannot insert it again.
    private func deliver(_ id: UUID, from store: FlowSessionStore) throws {
        guard let text = try store.consume(id: id) else { return }
        let proxy = textDocumentProxy
        let check = InsertionCheck(start: proxyContext)
        let replaced = proxy.selectedText ?? ""
        delivering = true
        model.pill = .transcribing
        proxy.insertText(text)
        Task { [weak self] in await self?.confirm(text, check: check, replaced: replaced) }
    }

    private func confirm(_ text: String, check: InsertionCheck, replaced: String) async {
        let landed = await didLand(check)
        delivering = false
        guard landed else {
            rescued = text
            model.pill = .pasteFailed(copied: copyToPasteboard(text))
            return
        }
        insertion = (document, text, context, replaced)
        model.pill = .inserted
        expiry?.cancel()
        expiry = Task { [weak self] in
            try? await Task.sleep(for: .seconds(4))
            guard !Task.isCancelled, let self, self.model.pill == .inserted else { return }
            self.model.pill = .idle
            self.insertion = nil
        }
    }

    /// A host can report the new text late or not at all, so a miss is final only after a slow host
    /// had three seconds and a fresh reading still shows the old text.
    private func didLand(_ check: InsertionCheck) async -> Bool {
        if await changes(check, within: .seconds(1)) { return true }
        await refreshReading()
        return await changes(check, within: .seconds(2))
    }

    private func changes(_ check: InsertionCheck, within limit: Duration) async -> Bool {
        let end = ContinuousClock.now + limit
        while ContinuousClock.now < end {
            try? await Task.sleep(for: .milliseconds(100))
            if await stays(check) { return true }
        }
        return false
    }

    /// A host that refuses the text can show it for a moment and take it back, so a change counts
    /// only when it is still there a moment later.
    private func stays(_ check: InsertionCheck) async -> Bool {
        guard check.landed(now: proxyContext) else { return false }
        try? await Task.sleep(for: .milliseconds(300))
        return check.landed(now: proxyContext)
    }

    /// Messages edits the field without telling the keyboard, so the proxy keeps the old text until the
    /// cursor moves. Moving it one character left and back makes the host send its real text.
    private func refreshReading() async {
        let events = contextEvents
        textDocumentProxy.adjustTextPosition(byCharacterOffset: -1)
        try? await Task.sleep(for: .milliseconds(300))
        // No notice means the cursor could not move, so there is nothing to put back.
        guard contextEvents != events else { return }
        textDocumentProxy.adjustTextPosition(byCharacterOffset: 1)
        try? await Task.sleep(for: .milliseconds(300))
    }

    private func copyToPasteboard(_ text: String) -> Bool {
        let board = UIPasteboard.general
        let before = board.changeCount
        board.string = text
        return board.changeCount != before
    }

    func copyRescuedText() {
        guard let rescued else { return }
        if copyToPasteboard(rescued) { model.pill = .pasteFailed(copied: true) }
    }

    func dismissFailure() {
        rescued = nil
        model.pill = .idle
    }

    func insert(_ text: String) {
        textDocumentProxy.insertText(model.shift ? text.uppercased() : text)
        model.shift = false
        typed()
    }

    func backspace() {
        textDocumentProxy.deleteBackward()
        typed()
    }

    private func typed() {
        insertion = nil
        if model.pill == .inserted { model.pill = .idle }
        refresh()
    }

    func undo() {
        guard undoAvailable, let insertion else { return }
        for _ in insertion.text { textDocumentProxy.deleteBackward() }
        if !insertion.replaced.isEmpty { textDocumentProxy.insertText(insertion.replaced) }
        self.insertion = nil
        model.pill = .idle
        refresh()
    }

    func cancel() {
        do {
            if let session = try store?.current() {
                try store?.cancel(id: session.id)
            }
            refresh()
        } catch {
            model.pill = .notice(error.localizedDescription)
        }
    }

    func stop() {
        guard hostActive, visible else { return }
        do {
            if let session = try store?.current() { try store?.stop(id: session.id) }
            refresh()
        } catch { model.pill = .notice(error.localizedDescription) }
    }

    private var undoAvailable: Bool {
        guard let insertion else { return false }
        return insertion.document == document && insertion.context == context
            && textDocumentProxy.documentContextBeforeInput?.hasSuffix(insertion.text) == true
    }
}
