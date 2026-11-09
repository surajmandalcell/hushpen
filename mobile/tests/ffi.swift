import Foundation

final class TestSystemLlm: SystemLlm {
    func available() -> Bool { true }

    func cleanup(text: String, instructions: String) throws -> String {
        precondition(!instructions.isEmpty)
        precondition(text.lowercased().contains("hello world"))
        return "Hello world."
    }
}

@main
struct FfiSmoke {
    static func main() throws {
        precondition(CommandLine.arguments.count == 2)
        let root = URL(fileURLWithPath: CommandLine.arguments[1])
            .appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }

        let models = try ModelRepository(data: root.path).models()
        precondition(models.filter { $0.kind == .speech }.count == 2)
        precondition(models.filter { $0.kind == .systemCleanup }.count == 1)

        let dictation = try Dictation(maxMinutes: 5)
        let effects = try dictation.handle(event: .toggle, nowMs: 0)
        precondition(dictation.snapshot(nowMs: 0).state == "listening")
        precondition(effects.contains { if case .startCapture = $0 { return true }; return false })

        let cleanup = SystemCleanup(system: TestSystemLlm())
        precondition(cleanup.available())
        let result = try cleanup.clean(raw: "hello world")
        precondition(result.text == "Hello world." && result.outcome == "used")

        let engine = try Engine(dataDir: root.path)
        defer { engine.shutdown() }
        let detector = root.appendingPathComponent("cache/resources/ggml-silero-v5.1.2.bin")
        let bytes = try detector.resourceValues(forKeys: [.fileSizeKey]).fileSize
        precondition(bytes == 885_098)
        print("Swift FFI: catalog, dictation, cleanup callback, and bundled detector passed.")
    }
}
