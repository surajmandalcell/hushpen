import XCTest
@testable import Hushpen

final class InsertionCheckTests: XCTestCase {
    private func context(_ before: String?, _ after: String? = nil, selected: String? = nil) -> InsertionCheck.Context {
        .init(before: before, after: after, selected: selected)
    }

    func testTextAppearsInAnEmptyField() {
        let check = InsertionCheck(start: context(nil))
        XCTAssertTrue(check.landed(now: context("Hello")))
        XCTAssertFalse(check.landed(now: context(nil)))
        XCTAssertFalse(check.landed(now: context("")))
        XCTAssertFalse(InsertionCheck(start: context("")).landed(now: context(nil)))
    }

    func testTextBeforeTheCursorChangesWhenTheTextLands() {
        let check = InsertionCheck(start: context("Hi team, "))
        XCTAssertTrue(check.landed(now: context("Hi team, Book a table.")))
        XCTAssertFalse(check.landed(now: context("Hi team, ")))
    }

    /// A host that cuts the context it reports can still change what follows the cursor.
    func testTextAfterTheCursorCountsAsAChange() {
        let check = InsertionCheck(start: context("Hi team, ", "see you"))
        XCTAssertTrue(check.landed(now: context("Hi team, ", "Book a table. see you")))
        XCTAssertFalse(check.landed(now: context("Hi team, ", "see you")))
    }

    /// A selection that the new text replaces leaves the text around it as it was.
    func testReplacingTheSelectionCountsAsAChange() {
        let check = InsertionCheck(start: context("Hi ", " there", selected: "team"))
        XCTAssertTrue(check.landed(now: context("Hi ", " there")))
        XCTAssertFalse(check.landed(now: context("Hi ", " there", selected: "team")))
    }

    /// A host that keeps its old reading looks the same as a host that refused the text, so the
    /// check never calls it a landing.
    func testAnUnchangedReadingIsNotALanding() {
        let check = InsertionCheck(start: context("hi"))
        XCTAssertFalse(check.landed(now: context("hi")))
        XCTAssertTrue(check.landed(now: context("hiThe quick brown fox.")))
    }
}
