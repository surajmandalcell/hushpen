@preconcurrency import ActivityKit
import Foundation

/// Shared by the app, which starts the activity, and the widget extension, which draws it.
struct DictationActivityAttributes: ActivityAttributes, Sendable {
    struct ContentState: Codable, Hashable, Sendable {
        let phase: String
    }

    let sessionID: UUID
    /// The system counts the elapsed time from this date, so the app never wakes to tick it.
    var startedAt = Date()
}
