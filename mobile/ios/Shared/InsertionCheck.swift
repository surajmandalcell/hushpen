import Foundation

/// Tells whether inserted dictation landed in the text field.
///
/// iOS reports no result for `insertText`, and a host can ignore it. The text landed when what the
/// keyboard reads from the field changed: the text before the cursor, the text after it, or the
/// selection. `hasText` cannot serve: the proxy turns it on at once, even when the host drops the text.
///
/// A host can also edit without telling the keyboard (Messages does not), so the proxy keeps
/// its old text until the cursor moves. An unchanged reading proves a failure only when it is fresh.
struct InsertionCheck: Equatable {
    /// What the keyboard reads around the cursor. An empty field reports nil at one moment and an
    /// empty string at another, so both mean the same.
    struct Context: Equatable {
        let before: String
        let after: String
        let selected: String

        init(before: String?, after: String?, selected: String?) {
            self.before = before ?? ""
            self.after = after ?? ""
            self.selected = selected ?? ""
        }
    }

    let start: Context

    func landed(now: Context) -> Bool { now != start }
}
