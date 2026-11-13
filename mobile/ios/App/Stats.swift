import Foundation

enum StatsRange: String, CaseIterable, Identifiable {
    case day = "Day", week = "Week", month = "Month", year = "Year"

    var id: String { rawValue }

    var unit: Calendar.Component {
        switch self {
        case .day: .day
        case .week: .weekOfYear
        case .month: .month
        case .year: .year
        }
    }

    /// The bars: hours of a day, days of a week or month, months of a year.
    fileprivate var step: Calendar.Component {
        switch self {
        case .day: .hour
        case .week, .month: .day
        case .year: .month
        }
    }

    var before: String {
        switch self {
        case .day: "yesterday"
        case .week: "last week"
        case .month: "last month"
        case .year: "last year"
        }
    }
}

struct StatsBucket: Identifiable, Equatable {
    let start: Date
    let label: String
    let words: Int
    var id: Date { start }
}

/// The numbers on Home and Stats, worked out from the phone's own history only.
struct StatsSummary {
    let range: StatsRange
    let buckets: [StatsBucket]
    /// The bar that holds `now`.
    let current: Int
    let words: Int
    let previousWords: Int
    let spokenSeconds: Double
    let typedSeconds: Double
    let sessions: Int
    let onThisPhone: Int
    let otherDevices: Int

    var savedSeconds: Double { max(0, typedSeconds - spokenSeconds) }

    /// "+26% on last week so far", or nothing when there is no earlier period to compare.
    var comparison: String? {
        guard previousWords > 0 else { return nil }
        let change = Int((Double(words - previousWords) / Double(previousWords) * 100).rounded())
        return "\(change >= 0 ? "+" : "")\(change)% on \(range.before) so far"
    }

    init(activity: [HistoryActivity], range: StatsRange, now: Date = Date(), typingSpeed: Int = 40, calendar: Calendar = .current) {
        self.range = range
        let period = calendar.dateInterval(of: range.unit, for: now) ?? DateInterval(start: now, duration: 1)
        let inside = activity.filter { period.contains($0.date) }
        let count = calendar.dateComponents([range.step], from: period.start, to: period.end).value(for: range.step) ?? 1
        var totals = [Int](repeating: 0, count: max(count, 1))
        for item in inside {
            let index = calendar.dateComponents([range.step], from: period.start, to: item.date).value(for: range.step) ?? 0
            if totals.indices.contains(index) { totals[index] += Int(item.words) }
        }
        buckets = totals.enumerated().map { index, words in
            let start = calendar.date(byAdding: range.step, value: index, to: period.start) ?? period.start
            return StatsBucket(start: start, label: Self.label(range, start, calendar), words: words)
        }
        current = buckets.lastIndex { $0.start <= now } ?? 0
        words = totals.reduce(0, +)
        spokenSeconds = inside.reduce(0) { $0 + Double($1.spokenMs) / 1000 }
        typedSeconds = typingSpeed > 0 ? Double(words) / Double(typingSpeed) * 60 : 0
        sessions = inside.count
        onThisPhone = inside.filter { $0.device.isEmpty }.count
        otherDevices = Set(inside.map(\.device).filter { !$0.isEmpty }).count
        let earlier = calendar.date(byAdding: range.unit, value: -1, to: now)
            .flatMap { calendar.dateInterval(of: range.unit, for: $0) }
        if let earlier {
            let window = DateInterval(start: earlier.start, end: min(earlier.end, earlier.start.addingTimeInterval(now.timeIntervalSince(period.start))))
            previousWords = activity.filter { $0.date >= window.start && $0.date < window.end }.reduce(0) { $0 + Int($1.words) }
        } else {
            previousWords = 0
        }
    }

    private static func label(_ range: StatsRange, _ date: Date, _ calendar: Calendar) -> String {
        switch range {
        case .day: String(calendar.component(.hour, from: date))
        case .week: calendar.veryShortWeekdaySymbols[calendar.component(.weekday, from: date) - 1]
        case .month: String(calendar.component(.day, from: date))
        case .year: calendar.veryShortMonthSymbols[calendar.component(.month, from: date) - 1]
        }
    }
}

/// Days in a row with at least one dictation, counted back from today (or yesterday while today is empty).
struct Streak {
    enum Day { case done, today, none }

    let days: Int
    let since: Date?
    /// One mark for each day of this week, from the first weekday.
    let week: [Day]

    init(activity: [HistoryActivity], now: Date = Date(), calendar: Calendar = .current) {
        let active = Set(activity.map { calendar.startOfDay(for: $0.date) })
        let today = calendar.startOfDay(for: now)
        var cursor = active.contains(today) ? today : (calendar.date(byAdding: .day, value: -1, to: today) ?? today)
        var count = 0
        var first: Date?
        while active.contains(cursor) {
            count += 1
            first = cursor
            guard let previous = calendar.date(byAdding: .day, value: -1, to: cursor) else { break }
            cursor = previous
        }
        days = count
        since = first
        let start = calendar.dateInterval(of: .weekOfYear, for: now)?.start ?? today
        week = (0..<7).map { offset in
            let day = calendar.date(byAdding: .day, value: offset, to: start) ?? start
            if day == today { return .today }
            return active.contains(day) ? .done : .none
        }
    }
}

/// A length of time as a big number and a small unit: `20` `min`, `45` `s`, `1.5` `h`.
func durationParts(_ seconds: Double) -> (value: String, unit: String) {
    if seconds < 60 { return (String(Int(seconds.rounded())), "s") }
    if seconds < 90 * 60 { return (String(Int((seconds / 60).rounded())), "min") }
    return (String(format: "%.1f", seconds / 3600), "h")
}

extension HistoryActivity {
    var date: Date { Date(timeIntervalSince1970: Double(createdMs) / 1000) }
}
