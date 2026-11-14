import XCTest
@testable import Hushpen

final class EditHistoryTests: XCTestCase {
    private func typing(_ text: String, into history: inout EditHistory) {
        for character in text { history.record(added: String(character), removed: "") }
    }

    func testLettersOfAWordJoinAndSpacesStandAlone() {
        var history = EditHistory()
        typing("hi yo", into: &history)
        XCTAssertEqual(history.undoStack, [
            TextEdit(added: "hi", removed: ""), TextEdit(added: " ", removed: ""), TextEdit(added: "yo", removed: "")
        ])
    }

    func testDeletedLettersJoinIntoOneRunInReadingOrder() {
        var history = EditHistory()
        for letter in ["c", "b", "a"] { history.record(added: "", removed: letter) }
        XCTAssertEqual(history.undoStack, [TextEdit(added: "", removed: "abc")])
    }

    func testDictationAndSelectionReplacementAreSeparateSteps() {
        var history = EditHistory()
        typing("ab", into: &history)
        history.record(added: "quick brown fox", removed: "", joining: false)
        history.record(added: "c", removed: "")
        history.record(added: "x", removed: "sel")
        XCTAssertEqual(history.undoStack.count, 4)
    }

    func testUndoThenRedoMovesTheSameEditBack() {
        var history = EditHistory()
        typing("hi", into: &history)
        let edit = history.takeUndo()
        XCTAssertEqual(edit, TextEdit(added: "hi", removed: ""))
        XCTAssertFalse(history.canUndo)
        XCTAssertTrue(history.canRedo)
        XCTAssertEqual(history.takeRedo(), edit)
        XCTAssertTrue(history.canUndo)
        XCTAssertFalse(history.canRedo)
    }

    func testNewEditClearsRedo() {
        var history = EditHistory()
        typing("hi", into: &history)
        _ = history.takeUndo()
        history.record(added: "yo", removed: "")
        XCTAssertFalse(history.canRedo)
    }

    func testTypingAfterUndoOfASpaceContinuesTheWordAndDropsRedo() {
        var history = EditHistory()
        typing("hi", into: &history)
        history.record(added: " ", removed: "")
        _ = history.takeUndo()
        history.record(added: "x", removed: "")
        XCTAssertEqual(history.undoStack, [TextEdit(added: "hix", removed: "")])
        XCTAssertFalse(history.canRedo)
    }

    func testEmptyEditIsIgnoredAndStackIsCapped() {
        var history = EditHistory()
        history.record(added: "", removed: "")
        XCTAssertFalse(history.canUndo)
        for index in 0..<(EditHistory.limit + 20) { history.record(added: "w\(index)", removed: "", joining: false) }
        XCTAssertEqual(history.undoStack.count, EditHistory.limit)
        XCTAssertEqual(history.undoStack.last?.added, "w\(EditHistory.limit + 19)")
    }

    func testDiscardLastDropsOnlyAMatchingEdit() {
        var history = EditHistory()
        history.record(added: "dictated", removed: "", joining: false)
        history.discardLast(added: "other")
        XCTAssertEqual(history.undoStack.count, 1)
        history.discardLast(added: "dictated")
        XCTAssertFalse(history.canUndo)
        XCTAssertFalse(history.canRedo)
    }

    func testAnotherDocumentClearsTheHistoryAndUndoNeedsTheSameDocument() {
        let first = UUID(), second = UUID()
        var history = EditHistory()
        history.bind(to: first)
        history.record(added: "hi", removed: "")
        XCTAssertTrue(history.allows(document: first))
        XCTAssertFalse(history.allows(document: second))
        XCTAssertFalse(history.allows(document: nil))
        history.bind(to: first)
        XCTAssertTrue(history.canUndo)
        history.bind(to: second)
        XCTAssertFalse(history.canUndo)
        XCTAssertFalse(history.canRedo)
        XCTAssertFalse(history.allows(document: first))
    }

    func testAnUnnamedDocumentNeverAllowsUndo() {
        var history = EditHistory()
        history.bind(to: nil)
        history.record(added: "hi", removed: "")
        XCTAssertFalse(history.allows(document: nil))
        history.clear()
        XCTAssertFalse(history.canUndo)
    }

    func testLevelUsesAFewCoarseStepsAndRoundTrips() {
        XCTAssertEqual(FlowLevel.encode(7), UInt8(FlowLevel.steps))
        XCTAssertEqual(FlowLevel.encode(-1), 0)
        XCTAssertEqual(FlowLevel.encode(0.5), 4)
        XCTAssertEqual(FlowLevel.decode(0), 0)
        XCTAssertEqual(FlowLevel.decode(UInt8(FlowLevel.steps)), 1)
        XCTAssertEqual(FlowLevel.decode(200), 1)
        XCTAssertEqual(Set((0...100).map { FlowLevel.encode(Double($0) / 100) }).count, FlowLevel.steps + 1)
        XCTAssertEqual(FlowLevel.normalized(decibels: -80), 0)
        XCTAssertEqual(FlowLevel.normalized(decibels: 0), 1)
        XCTAssertLessThan(FlowLevel.normalized(decibels: -40), FlowLevel.normalized(decibels: -20))
    }
}
