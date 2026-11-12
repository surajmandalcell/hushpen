import Darwin
import Foundation
import os
import XCTest
@testable import Hushpen

final class PhoneMeasurements: XCTestCase {
    func testS1MiniCleanupLatencyAndPeakMemory() throws {
        let root = try modelRoot(file: "models/llm/s1-mini.gguf")
        let catalog = try ModelRepository.withMemory(data: root.path, physicalMemoryBytes: 6_000_000_000)
        XCTAssertTrue(try catalog.verify(id: "s1-mini"))
        let local = try LocalCleanup(dataDir: root.path, physicalMemoryBytes: 6_000_000_000)
        defer { local.shutdown() }
        try local.setModel(modelId: "s1-mini")
        local.setMemoryBudget(bytes: 6_000_000_000)
        let system = SystemCleanup(system: MeasurementSystem())
        system.setLocal(local: local)
        XCTAssertFalse(system.available())
        print("PHONE_MEMORY_BEFORE s1-mini app_available_bytes=\(os_proc_available_memory()) capacity_override_bytes=6000000000 host_physical_bytes=\(ProcessInfo.processInfo.physicalMemory) ready=\(local.available())")
        let start = DispatchTime.now().uptimeNanoseconds
        let result = try system.clean(raw: "um the quick brown fox jumps over the lazy dog")
        let elapsed = Double(DispatchTime.now().uptimeNanoseconds - start) / 1_000_000
        XCTAssertEqual(result.outcome, "used")
        XCTAssertTrue(result.text.lowercased().contains("quick brown fox"))
        print("PHONE_MEASUREMENT s1-mini latency_ms=\(elapsed) peak_rss_bytes=\(try peakMemory()) outcome=\(result.outcome)")
    }

    func testTurboQ5LoadPeakMemory() throws {
        let root = try modelRoot(file: "models/whisper/ggml-large-v3-turbo-q5_0.bin")
        let catalog = try ModelRepository.withMemory(data: root.path, physicalMemoryBytes: 6_000_000_000)
        XCTAssertTrue(try catalog.verify(id: "large-v3-turbo-q5_0"))
        let engine = try Engine(dataDir: root.path)
        defer { engine.shutdown() }
        print("PHONE_MEMORY_BEFORE turbo-q5 app_available_bytes=\(os_proc_available_memory()) host_physical_bytes=\(ProcessInfo.processInfo.physicalMemory)")
        let start = DispatchTime.now().uptimeNanoseconds
        try engine.load(modelId: "large-v3-turbo-q5_0")
        let elapsed = Double(DispatchTime.now().uptimeNanoseconds - start) / 1_000_000
        print("PHONE_MEASUREMENT turbo-q5 load_ms=\(elapsed) peak_rss_bytes=\(try peakMemory())")
        try engine.unload()
    }

    func testQwenCleanupLatencyAndPeakMemory() throws {
        let root = try modelRoot(file: "models/llm/qwen3.5-2b.gguf")
        let catalog = try ModelRepository.withMemory(data: root.path, physicalMemoryBytes: 8_000_000_000)
        XCTAssertTrue(try catalog.verify(id: "qwen3.5-2b"))
        let local = try LocalCleanup(dataDir: root.path, physicalMemoryBytes: 8_000_000_000)
        defer { local.shutdown() }
        try local.setModel(modelId: "qwen3.5-2b")
        local.setMemoryBudget(bytes: 8_000_000_000)
        print("PHONE_MEMORY_BEFORE qwen3.5-2b app_available_bytes=\(os_proc_available_memory()) capacity_override_bytes=8000000000 host_physical_bytes=\(ProcessInfo.processInfo.physicalMemory) ready=\(local.available())")
        let start = DispatchTime.now().uptimeNanoseconds
        let result = try local.clean(raw: "um please send the meeting notes to the team tomorrow morning", profile: "general")
        let elapsed = Double(DispatchTime.now().uptimeNanoseconds - start) / 1_000_000
        XCTAssertEqual(result.outcome, "used")
        XCTAssertTrue(result.text.lowercased().contains("meeting notes"))
        print("PHONE_MEASUREMENT qwen3.5-2b capacity_override_bytes=8000000000 host_physical_bytes=\(ProcessInfo.processInfo.physicalMemory) latency_ms=\(elapsed) peak_rss_bytes=\(try peakMemory()) outcome=\(result.outcome)")
    }

    private func modelRoot(file: String) throws -> URL {
        let documents = try XCTUnwrap(FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first)
        let root = documents.appendingPathComponent("PhoneMeasurements", isDirectory: true)
        guard FileManager.default.fileExists(atPath: root.appendingPathComponent(file).path) else {
            throw XCTSkip("Place the pinned model in Documents/PhoneMeasurements/\(file) to run this measurement.")
        }
        return root
    }

    private func peakMemory() throws -> Int64 {
        var usage = rusage()
        guard getrusage(RUSAGE_SELF, &usage) == 0 else {
            throw MobileError.Failed(detail: "Peak memory could not be read.")
        }
        return Int64(usage.ru_maxrss)
    }
}

private final class MeasurementSystem: SystemLlm, Sendable {
    func available() -> Bool { false }
    func cleanup(text: String, instructions: String) throws -> String {
        throw MobileError.Failed(detail: "This measurement has no native system model.")
    }
}
