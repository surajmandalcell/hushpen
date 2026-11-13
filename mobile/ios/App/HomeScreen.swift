import SwiftUI

enum Route: Hashable {
    case models, devices, licenses
}

/// The opening page: this week in numbers, then the latest dictations. Setup shows only while something is missing.
struct HomeScreen: View {
    let onboarding: OnboardingModel
    let models: ModelsModel
    let devices: DevicesModel
    let server: ServerModel
    let history: HistoryModel
    let showStats: () -> Void
    let showHistory: () -> Void
    @AppStorage("typingSpeed") private var typingSpeed = 40

    private var speech: [Model] { models.models.filter { $0.kind == .speech } }
    private var current: Model? { speech.first { $0.id == models.selectedModel } ?? speech.first { $0.recommended } }

    var body: some View {
        let week = StatsSummary(activity: history.activity, range: .week, typingSpeed: typingSpeed)
        let streak = Streak(activity: history.activity)
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                VStack(alignment: .leading, spacing: 0) {
                    Text(Date().formatted(.dateTime.weekday(.wide).day().month(.wide)).uppercased())
                        .font(.subheadline.weight(.semibold)).foregroundStyle(.secondary)
                    Text("Home").font(.largeTitle.bold()).accessibilityAddTraits(.isHeader)
                }
                .padding(.horizontal, 4).padding(.top, 8)
                let notices = [models.notice, devices.notice, server.notice, history.notice].filter { !$0.isEmpty }
                ForEach(notices, id: \.self) { NoticeRow(text: $0).card() }
                if !onboarding.setupDone || !(current?.status == .ready) { setup }
                wordsCard(week)
                HStack(alignment: .top, spacing: 12) {
                    VStack(alignment: .leading, spacing: 4) {
                        CardHeader(symbol: "timer", title: "Time saved")
                        let saved = durationParts(week.savedSeconds)
                        BigNumber(value: saved.value, unit: saved.unit).padding(.top, 2)
                        Text("this week").font(.subheadline).foregroundStyle(.secondary)
                    }
                    .card()
                    VStack(alignment: .leading, spacing: 4) {
                        CardHeader(symbol: "flame", title: "Streak")
                        BigNumber(value: String(streak.days), unit: streak.days == 1 ? "day" : "days").padding(.top, 2)
                        Text(streak.since.map { "since \($0.formatted(.dateTime.day().month(.abbreviated)))" } ?? "Dictate to start one")
                            .font(.subheadline).foregroundStyle(.secondary)
                    }
                    .card()
                }
                HStack {
                    Text("Recent").font(.title3.bold()).accessibilityAddTraits(.isHeader)
                    Spacer()
                    if !history.entries.isEmpty { Button("See All", action: showHistory).font(.body) }
                }
                .padding(.horizontal, 4).padding(.top, 10)
                if history.entries.isEmpty {
                    Text("Dictations you make with the Hushpen keyboard appear here. They stay on this iPhone.")
                        .font(.subheadline).foregroundStyle(.secondary).card()
                } else {
                    ForEach(history.entries.prefix(3)) { HistoryCard(entry: $0, style: .home) }
                }
            }
            .padding(.horizontal, 16)
            .padding(.bottom, 16)
        }
        .background(Color(.systemGroupedBackground))
        .toolbarVisibility(.hidden, for: .navigationBar)
        .task {
            onboarding.refresh()
            await models.refresh()
            await devices.refresh()
            await history.refresh()
        }
    }

    private func wordsCard(_ week: StatsSummary) -> some View {
        Button(action: showStats) {
            VStack(alignment: .leading, spacing: 6) {
                CardHeader(symbol: "chart.bar.fill", title: "Words", trailing: "This week")
                HStack(alignment: .bottom) {
                    VStack(alignment: .leading, spacing: 4) {
                        BigNumber(value: week.words.formatted())
                        Text(week.comparison?.replacing(" so far", with: "") ?? (week.words == 0 ? "No dictations yet" : " "))
                            .font(.subheadline).foregroundStyle(.secondary)
                    }
                    Spacer(minLength: 12)
                    spark(week)
                }
            }
            .card()
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .combine)
        .accessibilityHint("Opens Stats")
        .accessibilityIdentifier("home.words")
    }

    private func spark(_ week: StatsSummary) -> some View {
        let peak = max(week.buckets.map(\.words).max() ?? 0, 1)
        return HStack(alignment: .bottom, spacing: 4) {
            ForEach(Array(week.buckets.enumerated()), id: \.offset) { index, bucket in
                let height = bucket.words == 0 ? 4 : max(8, CGFloat(bucket.words) / CGFloat(peak) * 52)
                RoundedRectangle(cornerRadius: 3, style: .continuous)
                    .fill(index == week.current ? Color.accentColor : Color.accentColor.opacity(bucket.words == 0 ? 0.15 : 0.35))
                    .frame(width: 14, height: height)
            }
        }
        .frame(height: 56, alignment: .bottom)
        .accessibilityHidden(true)
    }

    /// What is still missing before dictation works: the keyboard, the microphone, a speech model.
    private var setup: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Finish setup").font(.headline).padding(.bottom, 8)
            if !onboarding.keyboardUsed { setupRow("Keyboard", "keyboard", onboarding.keyboardAdded ? "Allow Full Access" : "Add", id: "onboarding.keyboard", onboarding.openSettings) }
            if !onboarding.microphoneGranted {
                setupRow("Microphone", "mic", onboarding.microphone == "Not requested" ? "Allow" : "Open Settings", id: "onboarding.microphone") {
                    if onboarding.microphone == "Not requested" { Task { await onboarding.requestMicrophone() } } else { onboarding.openSettings() }
                }
            }
            if current?.status != .ready {
                NavigationLink(value: Route.models) {
                    HStack {
                        RowTitle(title: "Speech model", symbol: "waveform")
                        Spacer(minLength: 8)
                        Text(speechLine).foregroundStyle(.tint)
                    }
                    .frame(minHeight: 44)
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("home.speech")
            }
        }
        .card()
    }

    private func setupRow(_ title: String, _ symbol: String, _ action: String, id: String, _ perform: @escaping () -> Void) -> some View {
        Button(action: perform) {
            HStack {
                RowTitle(title: title, symbol: symbol)
                Spacer(minLength: 8)
                Text(action).foregroundStyle(.tint)
            }
            .frame(minHeight: 44)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier(id)
    }

    private var speechLine: String {
        guard let current else { return "Loading" }
        if models.downloading == current.id { return models.verifying ? "Checking" : "Downloading \(models.total > 0 ? Int(Double(models.progress) / Double(models.total) * 100) : 0)%" }
        switch current.status {
        case .needsVerification: return "Not checked"
        case .damaged: return "Damaged"
        default: return "Get"
        }
    }
}
