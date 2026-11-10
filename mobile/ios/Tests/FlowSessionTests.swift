import Foundation
import XCTest
@testable import Hushpen

final class FlowSessionTests: XCTestCase {
    private var root: URL!
    private var store: FlowSessionStore!
    private let now = Date(timeIntervalSince1970: 1_800_000_000)
    private let document = UUID()

    override func setUpWithError() throws {
        root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        store = try FlowSessionStore(root: root)
    }

    override func tearDownWithError() throws {
        try FileManager.default.removeItem(at: root)
    }

    func testResultReturnsToItsOriginalDocumentOnlyOnce() throws {
        let request = try store.begin(documentIdentifier: document, context: "cursor", now: now)
        let claimed = try store.request(for: request.url, now: now)
        XCTAssertEqual(claimed.id, request.id)
        try store.complete(id: request.id, text: "Hello world.", now: now)
        let reopened = try FlowSessionStore(root: root)
        XCTAssertEqual(try reopened.consume(id: request.id, documentIdentifier: document, context: "cursor", now: now), "Hello world.")
        XCTAssertNil(try store.consume(id: request.id, documentIdentifier: document, context: "cursor", now: now))
        XCTAssertNil(try store.current(now: now)?.text)
    }

    func testCancelledAndReplacedRequestsCannotFinish() throws {
        let first = try store.begin(documentIdentifier: document, context: "cursor", now: now)
        _ = try store.request(for: first.url, now: now)
        try store.cancel(id: first.id, now: now)
        XCTAssertThrowsError(try store.complete(id: first.id, text: "late", now: now))
        let second = try store.begin(documentIdentifier: document, context: "cursor", now: now)
        XCTAssertThrowsError(try store.request(for: first.url, now: now))
        XCTAssertEqual(try store.current(now: now)?.id, second.id)
    }

    func testExpiredAndFutureRequestsCannotStart() throws {
        let session = try store.begin(documentIdentifier: document, context: "cursor", now: now)
        XCTAssertThrowsError(try store.request(for: session.url, now: now.addingTimeInterval(-1)))
        XCTAssertThrowsError(try store.request(for: session.url, now: now.addingTimeInterval(600)))
        XCTAssertNil(try store.current(now: now.addingTimeInterval(600)))
    }

    func testDuplicateAndUntrustedURLsCannotStartRecording() throws {
        let session = try store.begin(documentIdentifier: document, context: "cursor", now: now)
        let suffix = session.id.uuidString
        for value in [
            "https://dictate?session=\(suffix)", "hushpen://other?session=\(suffix)",
            "hushpen://dictate/path?session=\(suffix)", "hushpen://dictate?session=\(suffix)&session=\(suffix)",
            "hushpen://dictate?session=\(suffix)#fragment", "hushpen://dictate?session=bad"
        ] {
            XCTAssertThrowsError(try store.request(for: XCTUnwrap(URL(string: value)), now: now))
        }
        _ = try store.request(for: session.url, now: now)
        XCTAssertThrowsError(try store.request(for: session.url, now: now))
    }

    func testMovedCursorAndDifferentDocumentRefuseInsertion() throws {
        for (target, context) in [(UUID(), "cursor"), (document, "moved")] {
            let session = try store.begin(documentIdentifier: document, context: "cursor", now: now)
            _ = try store.request(for: session.url, now: now)
            try store.complete(id: session.id, text: "private text", now: now)
            XCTAssertNil(try store.consume(id: session.id, documentIdentifier: target, context: context, now: now))
            XCTAssertEqual(try store.current(now: now)?.state, .cancelled)
            XCTAssertNil(try store.current(now: now)?.text)
        }
    }

    func testEmptyAndOversizedTextLeaveRecordingUnfinished() throws {
        let session = try store.begin(documentIdentifier: document, context: "cursor", now: now)
        _ = try store.request(for: session.url, now: now)
        XCTAssertThrowsError(try store.complete(id: session.id, text: " \n", now: now))
        XCTAssertThrowsError(try store.complete(id: session.id, text: String(repeating: "a", count: 1_048_577), now: now))
        for character in ["\"", "\\"] {
            let escaped = String(repeating: character, count: 600_000)
            XCTAssertLessThan(escaped.utf8.count, 1_048_576)
            XCTAssertThrowsError(try store.complete(id: session.id, text: escaped, now: now))
            XCTAssertEqual(try store.current(now: now)?.state, .recording)
            XCTAssertNil(try store.current(now: now)?.text)
        }
        XCTAssertEqual(try store.current(now: now)?.state, .recording)
    }

    func testContextDigestKeepsBoundariesAndDoesNotStoreSurroundingText() {
        let first = FlowSessionStore.context(before: "private", after: "text", selection: nil)
        XCTAssertEqual(first.count, 64)
        XCTAssertFalse(first.contains("private"))
        XCTAssertNotEqual(first, FlowSessionStore.context(before: "privatetext", after: "", selection: nil))
        XCTAssertNotEqual(first, FlowSessionStore.context(before: "private", after: "text", selection: ""))
    }

    func testNewRequestReplacesDamagedSessionWithoutHidingIOErrors() throws {
        let file = root.appendingPathComponent("Flow/session.json")
        try Data("broken json".utf8).write(to: file)
        XCTAssertThrowsError(try store.current(now: now))
        let session = try store.begin(documentIdentifier: document, context: "cursor", now: now)
        XCTAssertEqual(try store.current(now: now)?.id, session.id)
        try FileManager.default.removeItem(at: file)
        try FileManager.default.createDirectory(at: file, withIntermediateDirectories: false)
        XCTAssertThrowsError(try store.begin(documentIdentifier: document, context: "cursor", now: now))
    }

    func testTwoStoreInstancesCannotConsumeTheSameResult() async throws {
        let session = try store.begin(documentIdentifier: document, context: "cursor", now: now)
        _ = try store.request(for: session.url, now: now)
        try store.complete(id: session.id, text: "one result", now: now)
        let first = try XCTUnwrap(store)
        let second = try FlowSessionStore(root: root)
        let document = self.document
        let now = self.now
        let values = try await withThrowingTaskGroup(of: String?.self) { group in
            for store in [first, second] {
                group.addTask {
                    try store.consume(id: session.id, documentIdentifier: document, context: "cursor", now: now)
                }
            }
            var values: [String] = []
            for try await value in group {
                if let value { values.append(value) }
            }
            return values
        }
        XCTAssertEqual(values, ["one result"])
    }
}
