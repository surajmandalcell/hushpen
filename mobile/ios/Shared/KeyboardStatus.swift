import Foundation

struct KeyboardHeartbeat: Codable, Equatable {
    let seenAt: Date
    let hasFullAccess: Bool

    static func read(root: URL?, now: Date = Date()) -> KeyboardHeartbeat? {
        guard let root else { return nil }
        let file = root.appendingPathComponent("keyboard-status.json")
        guard let size = (try? file.resourceValues(forKeys: [.fileSizeKey]))?.fileSize,
              size <= 4096,
              let data = try? Data(contentsOf: file),
              data.count <= 4096,
              let value = try? JSONDecoder().decode(Self.self, from: data),
              value.seenAt <= now else { return nil }
        return value
    }

    func write(root: URL?) throws {
        guard let root else { throw FlowSessionError.unavailable }
        var options: Data.WritingOptions = [.atomic]
        #if os(iOS)
        options.insert(.completeFileProtectionUnlessOpen)
        #endif
        try JSONEncoder().encode(self).write(to: root.appendingPathComponent("keyboard-status.json"), options: options)
    }
}

/// Keyboard settings the app writes and the keyboard reads, in the App Group.
enum KeyboardSetting {
    static let leftHandedKey = "leftHandedKeyboard"
    static var defaults: UserDefaults? { UserDefaults(suiteName: FlowSessionStore.groupIdentifier) }

    /// Mirrors the strip: dictate on the left, Undo and Redo on the right. Off without Full Access.
    static var leftHanded: Bool { defaults?.bool(forKey: leftHandedKey) ?? false }
}

struct KeyboardStatus: Equatable {
    static let bundleIdentifier = "com.mandalsuraj.hushpen.keyboard"
    let added: Bool
    let fullAccess: Bool?

    var text: String {
        guard added else { return "Open keyboard to check" }
        switch fullAccess {
        case true: return "Full Access last seen"
        case false: return "Allow Full Access"
        case nil: return "Added"
        }
    }

    init(modes: [String], heartbeat: KeyboardHeartbeat?, legacySeenAt: Date?, now: Date = Date()) {
        let heartbeat = heartbeat.flatMap { $0.seenAt <= now ? $0 : nil }
        let legacySeen = legacySeenAt.map { $0 <= now } ?? false
        // UIKit exposes no public bundle identifier; its description is only a fallback signal.
        let tokens = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: ".-_"))
        let listed = modes.contains { $0.components(separatedBy: tokens.inverted).contains(Self.bundleIdentifier) }
        added = heartbeat != nil || legacySeen || listed
        fullAccess = heartbeat.flatMap { now.timeIntervalSince($0.seenAt) < 86_400 ? $0.hasFullAccess : nil }
    }

    static var groupRoot: URL? {
        FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: FlowSessionStore.groupIdentifier)
    }
}
