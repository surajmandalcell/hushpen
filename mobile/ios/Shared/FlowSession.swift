import CryptoKit
import Darwin
import Foundation

enum FlowSessionState: String, Codable {
    case pending, recording, completed, cancelled, consumed
}

enum FlowSessionCommand: String, Codable {
    case start, stop, cancel
}

struct FlowSession: Codable, Equatable {
    let id: UUID
    var documentIdentifier: UUID
    let context: String
    let createdAt: Date
    let expiresAt: Date
    var state: FlowSessionState
    var text: String?
    var command: FlowSessionCommand?

    var url: URL {
        URL(string: "hushpen://dictate?session=\(id.uuidString)")!
    }
}

enum FlowSessionError: LocalizedError {
    case unavailable, invalidURL, stale, wrongState, invalidText, damaged

    var errorDescription: String? {
        switch self {
        case .unavailable: "Allow Full Access for the Hushpen keyboard."
        case .invalidURL: "This dictation link is not valid."
        case .stale: "This dictation session has ended. Start again from the keyboard."
        case .wrongState: "This dictation session is no longer waiting."
        case .invalidText: "The dictation text is empty or too long."
        case .damaged: "The dictation session could not be read. Start again from the keyboard."
        }
    }
}

final class FlowSessionStore: @unchecked Sendable {
    static let groupIdentifier = "group.com.mandalsuraj.hushpen"
    private let file: URL

    convenience init(groupIdentifier: String = FlowSessionStore.groupIdentifier) throws {
        guard let root = FileManager.default.containerURL(
            forSecurityApplicationGroupIdentifier: groupIdentifier
        ) else { throw FlowSessionError.unavailable }
        try self.init(root: root)
    }

    init(root: URL) throws {
        let directory = root.appendingPathComponent("Flow", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        var privateDirectory = directory
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        try privateDirectory.setResourceValues(values)
        file = directory.appendingPathComponent("session.json")
    }

    static func context(before: String?, after: String?, selection: String?) -> String {
        let parts = [before, after, selection]
        let data = (try? JSONEncoder().encode(parts)) ?? Data()
        return SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }

    func begin(documentIdentifier: UUID, context: String, now: Date = Date()) throws -> FlowSession {
        try transaction(replacingDamaged: true) { stored in
            let session = FlowSession(
                id: UUID(), documentIdentifier: documentIdentifier, context: context,
                createdAt: now, expiresAt: now.addingTimeInterval(600), state: .pending,
                command: .start
            )
            stored = session
            return session
        }
    }

    func request(for url: URL, now: Date = Date()) throws -> FlowSession {
        guard let parts = URLComponents(url: url, resolvingAgainstBaseURL: false),
              parts.scheme == "hushpen", parts.host == "dictate", parts.path.isEmpty,
              parts.user == nil, parts.password == nil, parts.port == nil, parts.fragment == nil,
              let items = parts.queryItems, items.count == 1, items[0].name == "session",
              let value = items[0].value, let id = UUID(uuidString: value)
        else { throw FlowSessionError.invalidURL }
        return try transaction { stored in
            guard var session = stored, session.id == id, Self.isLive(session, now: now)
            else { throw FlowSessionError.stale }
            guard session.state == .pending else { throw FlowSessionError.wrongState }
            session.state = .recording
            session.command = nil
            stored = session
            return session
        }
    }

    func complete(id: UUID, text: String, now: Date = Date()) throws {
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              text.utf8.count <= 1_048_576 else { throw FlowSessionError.invalidText }
        try transaction { stored in
            guard var session = stored, session.id == id, Self.isLive(session, now: now)
            else { throw FlowSessionError.stale }
            guard session.state == .recording else { throw FlowSessionError.wrongState }
            session.text = text
            session.state = .completed
            session.command = nil
            stored = session
        }
    }

    func resume(id: UUID, now: Date = Date()) throws {
        try transaction { stored in
            guard var session = stored, session.id == id, Self.isLive(session, now: now)
            else { throw FlowSessionError.stale }
            guard session.state == .cancelled else { throw FlowSessionError.wrongState }
            session.state = .recording
            session.command = nil
            stored = session
        }
    }

    func cancel(id: UUID, now: Date = Date()) throws {
        try transaction { stored in
            guard var session = stored, session.id == id, Self.isLive(session, now: now)
            else { throw FlowSessionError.stale }
            guard session.state != .consumed else { throw FlowSessionError.wrongState }
            session.state = .cancelled
            session.text = nil
            session.command = .cancel
            stored = session
        }
    }

    func stop(id: UUID, now: Date = Date()) throws {
        try transaction { stored in
            guard var session = stored, session.id == id, Self.isLive(session, now: now)
            else { throw FlowSessionError.stale }
            guard session.state == .recording else { throw FlowSessionError.wrongState }
            session.command = .stop
            stored = session
        }
    }

    func cancelAbandonedRecording() throws {
        try transaction(replacingDamaged: true) { stored in
            guard var session = stored, session.state == .recording else { return }
            session.state = .cancelled
            session.text = nil
            session.command = .cancel
            stored = session
        }
    }

    func current(now: Date = Date()) throws -> FlowSession? {
        try transaction { stored in
            guard let session = stored else { return nil }
            guard Self.isLive(session, now: now) else {
                stored = nil
                return nil
            }
            return session
        }
    }

    func consume(
        id: UUID, documentIdentifier: UUID, context: String, now: Date = Date()
    ) throws -> String? {
        try transaction { stored in
            guard var session = stored, session.id == id, Self.isLive(session, now: now),
                  session.state == .completed else { return nil }
            guard session.documentIdentifier == documentIdentifier, session.context == context else {
                session.state = .cancelled
                session.text = nil
                stored = session
                return nil
            }
            let text = session.text
            session.state = .consumed
            session.text = nil
            stored = session
            return text
        }
    }

    func confirmDestination(
        id: UUID, documentIdentifier: UUID, context: String, now: Date = Date()
    ) throws {
        try transaction { stored in
            guard var session = stored, session.id == id, Self.isLive(session, now: now)
            else { throw FlowSessionError.stale }
            guard session.state == .completed else { throw FlowSessionError.wrongState }
            guard session.context == context else { throw FlowSessionError.stale }
            session.documentIdentifier = documentIdentifier
            stored = session
        }
    }

    private static func isLive(_ session: FlowSession, now: Date) -> Bool {
        session.createdAt <= now && session.expiresAt > now
            && session.expiresAt.timeIntervalSince(session.createdAt) == 600
    }

    private func transaction<T>(
        replacingDamaged: Bool = false, _ body: (inout FlowSession?) throws -> T
    ) throws -> T {
        let coordinator = NSFileCoordinator()
        var coordinationError: NSError?
        var result: Result<T, Error>?
        var changed = false
        coordinator.coordinate(writingItemAt: file, options: .forMerging, error: &coordinationError) { url in
            result = Result {
                var session: FlowSession?
                if FileManager.default.fileExists(atPath: url.path) {
                    do {
                        let size = try url.resourceValues(forKeys: [.fileSizeKey]).fileSize ?? 0
                        guard size <= 1_100_000 else { throw FlowSessionError.damaged }
                        let data = try Data(contentsOf: url)
                        guard data.count <= 1_100_000 else { throw FlowSessionError.damaged }
                        do {
                            session = try JSONDecoder().decode(FlowSession.self, from: data)
                        } catch is DecodingError {
                            throw FlowSessionError.damaged
                        }
                    } catch FlowSessionError.damaged where replacingDamaged {
                        session = nil
                    }
                }
                let original = session
                let value = try body(&session)
                guard session != original else { return value }
                if let session {
                    var options: Data.WritingOptions = [.atomic]
                    #if os(iOS)
                    options.insert(.completeFileProtection)
                    #endif
                    let encoded = try JSONEncoder().encode(session)
                    guard encoded.count <= 1_100_000 else { throw FlowSessionError.invalidText }
                    try encoded.write(to: url, options: options)
                } else if FileManager.default.fileExists(atPath: url.path) {
                    try FileManager.default.removeItem(at: url)
                }
                changed = true
                return value
            }
        }
        if let coordinationError { throw coordinationError }
        guard let result else { throw FlowSessionError.unavailable }
        if changed { notify_post(FlowSessionObservation.name) }
        return try result.get()
    }
}

final class FlowSessionObservation {
    static let name = "com.mandalsuraj.hushpen.flow.changed"
    private var token: Int32 = -1

    init(changed: @escaping @Sendable () -> Void) throws {
        let status = notify_register_dispatch(Self.name, &token, .main) { _ in changed() }
        guard status == NOTIFY_STATUS_OK else { throw FlowSessionError.unavailable }
    }

    deinit { if token != -1 { notify_cancel(token) } }
}
