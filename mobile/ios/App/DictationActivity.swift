@preconcurrency import ActivityKit
import Foundation
import OSLog

struct DictationActivityAttributes: ActivityAttributes, Sendable {
    struct ContentState: Codable, Hashable, Sendable {
        let phase: String
    }

    let sessionID: UUID
}

@MainActor
final class DictationActivity {
    private var activity: Activity<DictationActivityAttributes>?
    private var operation: Task<Void, Never>?
    private var phase = ""
    private let logger = Logger(subsystem: "com.mandalsuraj.hushpen", category: "DictationActivity")

    init() {
        let abandoned = Activity<DictationActivityAttributes>.activities
        guard !abandoned.isEmpty else { return }
        operation = Task {
            for activity in abandoned { await activity.end(nil, dismissalPolicy: .immediate) }
        }
    }

    func update(session: FlowSession, phase: String) {
        guard self.phase != phase else { return }
        self.phase = phase
        let content = ActivityContent(
            state: DictationActivityAttributes.ContentState(phase: phase),
            staleDate: session.expiresAt
        )
        if let activity {
            let previous = operation
            operation = Task {
                await previous?.value
                await activity.update(content)
            }
        } else if ActivityAuthorizationInfo().areActivitiesEnabled {
            do {
                activity = try Activity.request(
                    attributes: DictationActivityAttributes(sessionID: session.id),
                    content: content, pushType: nil
                )
            } catch { logger.error("Live Activity could not start: \(error.localizedDescription, privacy: .public)") }
        }
    }

    func end() {
        phase = ""
        guard let activity else { return }
        self.activity = nil
        let previous = operation
        operation = Task {
            await previous?.value
            await activity.end(nil, dismissalPolicy: .immediate)
        }
    }
}
