import XCTest
@testable import Hushpen

final class StatsTests: XCTestCase {
    private var calendar: Calendar = {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "UTC")!
        calendar.firstWeekday = 2
        return calendar
    }()

    /// Friday 13 November 2026, 09:41.
    private var now: Date { date(2026, 11, 13, 9, 41) }

    private func date(_ year: Int, _ month: Int, _ day: Int, _ hour: Int = 12, _ minute: Int = 0) -> Date {
        calendar.date(from: DateComponents(year: year, month: month, day: day, hour: hour, minute: minute))!
    }

    private func item(_ date: Date, words: UInt32, spoken: UInt64 = 7_000, device: String = "") -> HistoryActivity {
        HistoryActivity(createdMs: Int64(date.timeIntervalSince1970 * 1000), words: words, spokenMs: spoken, device: device)
    }

    func testWeekCountsWordsPerDayAndComparesWithLastWeekSoFar() {
        let activity = [
            item(date(2026, 11, 9), words: 232), item(date(2026, 11, 11), words: 362), item(date(2026, 11, 13, 9, 12), words: 57),
            item(date(2026, 11, 2), words: 400), item(date(2026, 11, 4), words: 200), item(date(2026, 11, 7), words: 900),
        ]
        let week = StatsSummary(activity: activity, range: .week, now: now, typingSpeed: 40, calendar: calendar)
        XCTAssertEqual(week.buckets.map(\.words), [232, 0, 362, 0, 57, 0, 0])
        XCTAssertEqual(week.buckets.map(\.label), ["M", "T", "W", "T", "F", "S", "S"])
        XCTAssertEqual(week.current, 4)
        XCTAssertEqual(week.words, 651)
        // Last week up to the same moment is Monday to Friday 09:41: 400 + 200, not the 900 words of Saturday.
        XCTAssertEqual(week.previousWords, 600)
        XCTAssertEqual(week.comparison, "+9% on last week so far")
    }

    func testTimeSavedIsTypingTimeMinusSpokenTime() {
        let week = StatsSummary(activity: [item(date(2026, 11, 13, 8), words: 80, spoken: 30_000)], range: .week, now: now, typingSpeed: 40, calendar: calendar)
        XCTAssertEqual(week.typedSeconds, 120)
        XCTAssertEqual(week.spokenSeconds, 30)
        XCTAssertEqual(week.savedSeconds, 90)
        XCTAssertEqual(durationParts(week.savedSeconds).unit, "min")
        XCTAssertEqual(durationParts(45).value, "45")
    }

    func testDayMonthAndYearBuckets() {
        let activity = [item(date(2026, 11, 13, 8, 5), words: 5), item(date(2026, 11, 13, 8, 50), words: 7), item(date(2026, 3, 1), words: 11)]
        let day = StatsSummary(activity: activity, range: .day, now: now, calendar: calendar)
        XCTAssertEqual(day.buckets.count, 24)
        XCTAssertEqual(day.buckets[8].words, 12)
        XCTAssertEqual(day.current, 9)
        let month = StatsSummary(activity: activity, range: .month, now: now, calendar: calendar)
        XCTAssertEqual(month.buckets.count, 30)
        XCTAssertEqual(month.buckets[12].words, 12)
        let year = StatsSummary(activity: activity, range: .year, now: now, calendar: calendar)
        XCTAssertEqual(year.buckets.count, 12)
        XCTAssertEqual(year.buckets[2].words, 11)
        XCTAssertEqual(year.words, 23)
    }

    func testOtherDevicesAreCountedOnlyWhenTheHistoryHasThem() {
        let activity = [item(date(2026, 11, 13, 8), words: 3), item(date(2026, 11, 13, 9), words: 3, device: "Kitchen iPad")]
        let week = StatsSummary(activity: activity, range: .week, now: now, calendar: calendar)
        XCTAssertEqual([week.sessions, week.onThisPhone, week.otherDevices], [2, 1, 1])
        let alone = StatsSummary(activity: [item(date(2026, 11, 13, 8), words: 3)], range: .week, now: now, calendar: calendar)
        XCTAssertEqual([alone.sessions, alone.onThisPhone, alone.otherDevices], [1, 1, 0])
        XCTAssertNil(StatsSummary(activity: [], range: .week, now: now, calendar: calendar).comparison)
    }

    func testStreakCountsDaysInARowFromTodayOrYesterday() {
        let run = [date(2026, 11, 13), date(2026, 11, 12), date(2026, 11, 11), date(2026, 11, 9)].map { item($0, words: 1) }
        let streak = Streak(activity: run, now: now, calendar: calendar)
        XCTAssertEqual(streak.days, 3)
        XCTAssertEqual(streak.since, calendar.startOfDay(for: date(2026, 11, 11)))
        XCTAssertEqual(streak.week, [.done, .none, .done, .done, .today, .none, .none])
        let yesterdayOnly = Streak(activity: [item(date(2026, 11, 12), words: 1)], now: now, calendar: calendar)
        XCTAssertEqual(yesterdayOnly.days, 1)
        XCTAssertEqual(Streak(activity: [], now: now, calendar: calendar).days, 0)
    }

    func testHistoryLineShortensLongStatsButKeepsTimeAndWords() {
        XCTAssertEqual(spokenLabel(7_000), "7 s")
        XCTAssertEqual(spokenLabel(125_000), "2 min")
        XCTAssertEqual(tookLabel(1_400), "1.4 s")
        XCTAssertEqual(wordCount(1), "1 word")
        XCTAssertEqual(wordCount(16), "16 words")
    }
}
