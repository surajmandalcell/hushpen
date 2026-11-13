import Charts
import SwiftUI

/// Words, time saved, streak, and how much ran on this iPhone. All of it comes from this phone's history.
struct StatsScreen: View {
    let history: HistoryModel
    @AppStorage("typingSpeed") private var typingSpeed = 40
    @State private var range = StatsRange.week
    @State private var selected: Double?

    private var summary: StatsSummary { StatsSummary(activity: history.activity, range: range, typingSpeed: typingSpeed) }

    var body: some View {
        let summary = summary
        ScrollView {
            VStack(spacing: 12) {
                Picker("Range", selection: $range) {
                    ForEach(StatsRange.allCases) { Text($0.rawValue).tag($0) }
                }
                .pickerStyle(.segmented)
                words(summary)
                timeSaved(summary)
                HStack(alignment: .top, spacing: 12) {
                    streak
                    onThisPhone(summary)
                }
            }
            .padding(.horizontal, 16)
            .padding(.bottom, 16)
        }
        .background(Color(.systemGroupedBackground))
        .navigationTitle("Stats")
        .task { await history.refresh() }
        .onChange(of: range) { _, _ in selected = nil }
    }

    private func words(_ summary: StatsSummary) -> some View {
        let index = min(max(selected.map { Int($0.rounded()) } ?? summary.current, 0), summary.buckets.count - 1)
        let bucket = summary.buckets[index]
        return VStack(alignment: .leading, spacing: 2) {
            BigNumber(value: summary.words.formatted(), unit: "words")
            Text(summary.comparison ?? (summary.words == 0 ? "No dictations yet" : " "))
                .font(.subheadline).foregroundStyle(.secondary)
            Chart {
                ForEach(Array(summary.buckets.enumerated()), id: \.offset) { offset, item in
                    bar(offset, words: item.words, current: offset == index)
                }
                if selected != nil || summary.words > 0 {
                    RuleMark(x: .value("Day", Double(index)))
                        .foregroundStyle(.secondary.opacity(0.5))
                        .lineStyle(StrokeStyle(lineWidth: 1))
                        .annotation(position: .top, spacing: 0, overflowResolution: .init(x: .fit(to: .chart), y: .disabled)) {
                            callout(bucket, now: index == summary.current, words: bucket.words)
                        }
                }
            }
            .chartXSelection(value: $selected)
            .chartXScale(domain: -0.6...(Double(summary.buckets.count) - 0.4))
            .chartXAxis {
                AxisMarks(values: summary.buckets.indices.filter { labelled($0) }.map(Double.init)) { value in
                    AxisValueLabel {
                        if let i = value.as(Double.self).map({ Int($0) }), summary.buckets.indices.contains(i) {
                            Text(summary.buckets[i].label)
                                .fontWeight(i == summary.current ? .semibold : .regular)
                                .foregroundStyle(i == summary.current ? Color.primary : Color.secondary)
                        }
                    }
                }
            }
            .chartYAxis { AxisMarks(position: .trailing) }
            .frame(height: 190)
            .padding(.top, 34)
            .accessibilityLabel("Words for each \(range == .day ? "hour" : range == .year ? "month" : "day")")
        }
        .card()
    }

    private func bar(_ offset: Int, words: Int, current: Bool) -> some ChartContent {
        let center = Double(offset)
        let from: Double = center - 0.31
        let to: Double = center + 0.31
        let top: Double = Double(words)
        let fill: Color = current ? .accentColor : .accentColor.opacity(0.35)
        let x0 = PlottableValue.value("Start", from)
        let x1 = PlottableValue.value("End", to)
        let y0 = PlottableValue.value("Base", 0.0)
        let y1 = PlottableValue.value("Words", top)
        return RectangleMark(xStart: x0, xEnd: x1, yStart: y0, yEnd: y1)
            .foregroundStyle(fill)
            .cornerRadius(6)
    }

    private func labelled(_ index: Int) -> Bool {
        switch range {
        case .week: true
        case .day: index % 6 == 0
        case .month: index % 5 == 0 || index == 0
        case .year: true
        }
    }

    private func callout(_ bucket: StatsBucket, now: Bool, words: Int) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(now ? "Today" : title(bucket)).font(.caption).foregroundStyle(.secondary)
            Text("\(words.formatted()) \(words == 1 ? "word" : "words")").font(.subheadline.weight(.semibold))
        }
        .padding(.horizontal, 10).padding(.vertical, 5)
        .background(Color(.tertiarySystemGroupedBackground), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .accessibilityHidden(true)
    }

    private func title(_ bucket: StatsBucket) -> String {
        switch range {
        case .day: bucket.start.formatted(.dateTime.hour())
        case .week: bucket.start.formatted(.dateTime.weekday(.wide))
        case .month: bucket.start.formatted(.dateTime.day().month(.abbreviated))
        case .year: bucket.start.formatted(.dateTime.month(.wide))
        }
    }

    private func timeSaved(_ summary: StatsSummary) -> some View {
        let saved = durationParts(summary.savedSeconds)
        let spoken = durationParts(summary.spokenSeconds)
        let typed = durationParts(summary.typedSeconds)
        return VStack(alignment: .leading, spacing: 4) {
            CardHeader(symbol: "timer", title: "Time saved")
            BigNumber(value: saved.value, unit: saved.unit).padding(.top, 2)
            Text(summary.words == 0
                ? "Dictate something and the time you saved shows here."
                : "You spoke for \(spoken.value) \(spoken.unit). Typing the same words at \(typingSpeed) a minute takes \(typed.value) \(typed.unit).")
                .font(.subheadline).foregroundStyle(.secondary)
        }
        .card()
    }

    private var streak: some View {
        let streak = Streak(activity: history.activity)
        return VStack(alignment: .leading, spacing: 4) {
            CardHeader(symbol: "flame", title: "Streak")
            BigNumber(value: String(streak.days), unit: streak.days == 1 ? "day" : "days").padding(.top, 2)
            HStack(spacing: 4) {
                ForEach(Array(streak.week.enumerated()), id: \.offset) { _, day in
                    Circle()
                        .fill(day == .done ? Color.accentColor : Color(.tertiarySystemFill))
                        .overlay { if day == .today { Circle().strokeBorder(Color.accentColor, lineWidth: 2) } }
                        .aspectRatio(1, contentMode: .fit)
                }
            }
            .padding(.top, 6)
            .accessibilityHidden(true)
        }
        .card()
        .accessibilityElement(children: .combine)
    }

    private func onThisPhone(_ summary: StatsSummary) -> some View {
        let percent = summary.sessions == 0 ? nil : Int((Double(summary.onThisPhone) / Double(summary.sessions) * 100).rounded())
        return VStack(alignment: .leading, spacing: 4) {
            CardHeader(symbol: "lock.fill", title: "On this \(deviceKind)")
            BigNumber(value: percent.map(String.init) ?? "\u{2013}", unit: percent == nil ? nil : "%").padding(.top, 2)
            Capsule().fill(Color(.tertiarySystemFill)).frame(height: 6)
                .overlay(alignment: .leading) {
                    GeometryReader { proxy in
                        Capsule().fill(Color.accentColor)
                            .frame(width: proxy.size.width * CGFloat(percent ?? 0) / 100)
                    }
                }
                .padding(.top, 6)
            HStack {
                Text("\(summary.onThisPhone) of \(summary.sessions)")
                Spacer(minLength: 4)
                if summary.otherDevices > 0 { Text("+\(summary.otherDevices) \(summary.otherDevices == 1 ? "device" : "devices")") }
            }
            .font(.footnote).foregroundStyle(.secondary).lineLimit(1).minimumScaleFactor(0.8)
        }
        .card()
        .accessibilityElement(children: .combine)
    }
}
