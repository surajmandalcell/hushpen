import XCTest
@testable import Hushpen

final class InsertionCheckTests: XCTestCase {
    func testTextAppearsInAnEmptyField() {
        let check = InsertionCheck(before: nil)
        XCTAssertTrue(check.landed(before: "Hello"))
        XCTAssertFalse(check.landed(before: nil))
        XCTAssertFalse(check.landed(before: ""))
        XCTAssertFalse(InsertionCheck(before: "").landed(before: nil))
    }

    func testTextBeforeTheCursorChangesWhenTheTextLands() {
        let check = InsertionCheck(before: "Hi team, ")
        XCTAssertTrue(check.landed(before: "Hi team, Book a table."))
        XCTAssertFalse(check.landed(before: "Hi team, "))
    }
}
