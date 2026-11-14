import Foundation

/// One edit the keyboard made at the cursor: `removed` stood before the cursor, `added` stands there now.
struct TextEdit: Equatable {
    var added: String
    var removed: String
}

/// The keyboard's own recent edits (typed text and inserted dictation), for the Undo and Redo buttons.
/// The edits are applied at the cursor through the text proxy, so the owner checks that the cursor
/// still sits behind the edit before it applies one.
struct EditHistory {
    static let limit = 200

    private(set) var undoStack: [TextEdit] = []
    private(set) var redoStack: [TextEdit] = []
    /// The text field these edits belong to (`documentIdentifier`).
    private(set) var document: UUID?

    var canUndo: Bool { !undoStack.isEmpty }
    var canRedo: Bool { !redoStack.isEmpty }
    var nextUndo: TextEdit? { undoStack.last }
    var nextRedo: TextEdit? { redoStack.last }

    /// Typed letters join into one word, and deleted letters into one run, so Undo steps by word.
    /// A space, a return, or dictation always starts its own step.
    mutating func record(added: String, removed: String, joining: Bool = true) {
        guard !added.isEmpty || !removed.isEmpty else { return }
        redoStack.removeAll()
        if joining, let top = undoStack.last {
            if top.removed.isEmpty, removed.isEmpty, !Self.isBlank(added), !Self.isBlank(top.added) {
                undoStack[undoStack.count - 1].added += added
                return
            }
            if top.added.isEmpty, added.isEmpty {
                // Delete walks backward, so the new letter stood before the ones already deleted.
                undoStack[undoStack.count - 1].removed = removed + top.removed
                return
            }
        }
        undoStack.append(TextEdit(added: added, removed: removed))
        if undoStack.count > Self.limit { undoStack.removeFirst(undoStack.count - Self.limit) }
    }

    /// Takes the newest edit off the undo stack and keeps it for Redo.
    mutating func takeUndo() -> TextEdit? {
        guard let edit = undoStack.popLast() else { return nil }
        redoStack.append(edit)
        return edit
    }

    mutating func takeRedo() -> TextEdit? {
        guard let edit = redoStack.popLast() else { return nil }
        undoStack.append(edit)
        return edit
    }

    /// Forgets the newest edit when something else already reverted it (the Undo button of a dictation).
    mutating func discardLast(added: String) {
        if undoStack.last?.added == added { undoStack.removeLast() }
    }

    mutating func clear() {
        undoStack.removeAll()
        redoStack.removeAll()
        document = nil
    }

    /// Another text field starts an empty history.
    mutating func bind(to document: UUID?) {
        if document != self.document {
            clear()
            self.document = document
        }
    }

    /// Edits apply only in the field that made them, and only when the system names that field.
    func allows(document: UUID?) -> Bool { document != nil && document == self.document }

    private static func isBlank(_ text: String) -> Bool {
        text.contains { $0.isWhitespace }
    }
}
