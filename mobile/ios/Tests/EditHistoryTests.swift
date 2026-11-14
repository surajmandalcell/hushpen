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

    func testLevelEncodingClampsAndRoundTrips() {
        XCTAssertEqual(FlowLevel.decode(FlowLevel.encode(0.5)), 0.5, accuracy: 0.001)
        XCTAssertEqual(FlowLevel.encode(7), 1000)
        XCTAssertEqual(FlowLevel.encode(-1), 0)
        XCTAssertEqual(FlowLevel.decode(99_999), 1)
        XCTAssertEqual(FlowLevel.normalized(decibels: -80), 0)
        XCTAssertEqual(FlowLevel.normalized(decibels: 0), 1)
        XCTAssertLessThan(FlowLevel.normalized(decibels: -40), FlowLevel.normalized(decibels: -20))
    }
}
