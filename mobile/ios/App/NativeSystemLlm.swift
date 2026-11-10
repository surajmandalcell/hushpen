import Foundation
import FoundationModels

final class NativeSystemLlm: SystemLlm, @unchecked Sendable {
    private let lock = NSLock()
    private var enabledValue: Bool
    private var foregroundValue = false

    init(enabled: Bool) { enabledValue = enabled }

    var enabled: Bool {
        get { lock.withLock { enabledValue } }
        set { lock.withLock { enabledValue = newValue } }
    }

    var foreground: Bool {
        get { lock.withLock { foregroundValue } }
        set { lock.withLock { foregroundValue = newValue } }
    }

    func available() -> Bool {
        enabled && foreground && SystemLanguageModel.default.availability == .available
    }

    func cleanup(text: String, instructions: String) throws -> String {
        guard available() else { throw MobileError.Failed(detail: "System cleanup is unavailable.") }
        let result = PromptResult()
        let completion = DispatchSemaphore(value: 0)
        let task = Task.detached {
            defer { completion.signal() }
            do {
                let session = LanguageModelSession(model: SystemLanguageModel.default, instructions: instructions)
                let reply = try await session.respond(to: text, options: GenerationOptions(samplingMode: .greedy, maximumResponseTokens: 2048))
                result.set(.success(reply.content))
            } catch {
                result.set(.failure(MobileError.Failed(detail: error.localizedDescription)))
            }
        }
        guard completion.wait(timeout: .now() + 28) == .success else {
            task.cancel()
            throw MobileError.Failed(detail: "System cleanup timed out.")
        }
        guard let reply = result.get() else { throw MobileError.Failed(detail: "System cleanup returned no text.") }
        return try reply.get()
    }
}

private final class PromptResult: @unchecked Sendable {
    private let lock = NSLock()
    private var value: Result<String, MobileError>?
    func set(_ result: Result<String, MobileError>) { lock.withLock { value = result } }
    func get() -> Result<String, MobileError>? { lock.withLock { value } }
}
