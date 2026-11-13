import Foundation

/// Tells whether inserted dictation landed in the text field.
///
/// iOS reports no result for `insertText`, and a host can ignore it. The text landed when the
/// field's text before the cursor changed. `hasText` cannot serve: the proxy turns it on at once,
/// even when the host drops the text.
struct InsertionCheck: Equatable {
    let before: String?

    /// An empty field reports nil at one moment and an empty string at another.
    func landed(before now: String?) -> Bool {
        (now ?? "") != (before ?? "")
    }
}
