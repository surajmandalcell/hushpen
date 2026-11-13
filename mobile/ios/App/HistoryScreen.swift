import SwiftUI
import UIKit

/// Every dictation this phone kept: one card each, grouped by day, with search.
struct HistoryScreen: View {
    let history: HistoryModel
    let models: ModelsModel

    var body: some View {
        @Bindable var history = history
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 8) {
                if !history.notice.isEmpty { NoticeRow(text: history.notice).card() }
                ForEach(days(history.entries), id: \.day) { group in
                    Text(dayTitle(group.day))
                        .font(.subheadline.weight(.semibold))
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 4)
                        .padding(.top, 10)
                        .accessibilityAddTraits(.isHeader)
                    ForEach(group.entries) { HistoryCard(entry: $0, style: .list) }
                }
                if history.hasMore {
                    ProgressView().frame(maxWidth: .infinity).padding(.vertical, 12)
                        .task { await history.loadMore() }
                }
            }
            .padding(.horizontal, 16)
        }
        .background(Color(.systemGroupedBackground))
        .overlay {
            if history.loaded && history.entries.isEmpty {
                if history.query.isEmpty {
                    ContentUnavailableView("No dictations yet", systemImage: "clock", description: Text("Dictations you make with the Hushpen keyboard appear here. They stay on this \(deviceKind)."))
                } else {
                    ContentUnavailableView.search(text: history.query)
                }
            }
        }
        .navigationTitle("History")
        .toolbarTitleDisplayMode(.large)
        .searchable(text: $history.query, placement: .navigationBarDrawer(displayMode: .automatic), prompt: "Search")
        .navigationDestination(for: HistoryEntry.self) { HistoryDetail(entry: $0, history: history, models: models) }
        .task(id: history.query) {
            if !history.query.isEmpty { try? await Task.sleep(for: .milliseconds(250)) }
            if !Task.isCancelled { await history.refresh() }
        }
    }

    private func days(_ entries: [HistoryEntry]) -> [(day: Date, entries: [HistoryEntry])] {
        var groups: [(day: Date, entries: [HistoryEntry])] = []
        for entry in entries {
            let day = Calendar.current.startOfDay(for: entry.date)
            if groups.last?.day == day { groups[groups.count - 1].entries.append(entry) } else { groups.append((day, [entry])) }
        }
        return groups
    }

    private func dayTitle(_ day: Date) -> String {
        if Calendar.current.isDateInToday(day) { return "Today" }
        if Calendar.current.isDateInYesterday(day) { return "Yesterday" }
        return day.formatted(.dateTime.weekday(.wide).day().month(.wide))
    }
}

/// One dictation. The list style has one row under the text: the time, the stats, and a plain copy icon.
/// The time and the copy icon never shrink; the stats drop "Took" first, then end in an ellipsis.
struct HistoryCard: View {
    enum Style { case home, list }

    let entry: HistoryEntry
    let style: Style
    @State private var copied = false

    var body: some View {
        ZStack(alignment: .bottomTrailing) {
            NavigationLink(value: entry) {
                VStack(alignment: .leading, spacing: 0) {
                    if style == .home {
                        HStack {
                            Text(time)
                            Spacer(minLength: 8)
                            Text(wordCount(entry.words))
                        }
                        .font(.subheadline).foregroundStyle(.secondary)
                        .padding(.bottom, 4)
                    }
                    Text(entry.text).font(.body).lineLimit(3).multilineTextAlignment(.leading)
                        .padding(.trailing, 2)
                    if style == .list { statsRow.padding(.top, 6) }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("history.card")
            if style == .list {
                GlyphButton(symbol: copied ? "checkmark" : "doc.on.doc", label: "Copy", size: 16, action: copy)
                    .padding(.trailing, -14)
            }
        }
        .padding(.leading, 16).padding(.trailing, 16).padding(.top, 14)
        .padding(.bottom, style == .list ? 0 : 14)
        .background(Color(.secondarySystemGroupedBackground), in: RoundedRectangle(cornerRadius: 20, style: .continuous))
        .sensoryFeedback(.success, trigger: copied) { _, now in now }
    }

    private var time: String { entry.date.formatted(date: .omitted, time: .shortened) }

    private var statsRow: some View {
        HStack(spacing: 8) {
            Text(time).font(.footnote.weight(.semibold)).fixedSize()
            ViewThatFits(in: .horizontal) {
                Text(stats(took: true)).fixedSize()
                Text(stats(took: false)).fixedSize()
                Text(stats(took: false)).lineLimit(1)
            }
            .font(.footnote).foregroundStyle(.secondary)
            Spacer(minLength: 0)
            Color.clear.frame(width: 44, height: 44)
        }
        .accessibilityElement(children: .combine)
    }

    private func stats(took: Bool) -> String {
        var parts = ["Spoke \(spokenLabel(entry.spokenMs))"]
        if took && entry.tookMs > 0 { parts.append("Took \(tookLabel(entry.tookMs))") }
        parts.append(wordCount(entry.words))
        return parts.joined(separator: " \u{B7} ")
    }

    private func copy() {
        UIPasteboard.general.string = entry.text
        copied = true
        Task { try? await Task.sleep(for: .seconds(1.5)); copied = false }
    }
}

/// History detail: the text in a card, then the facts about it, with Copy within thumb reach.
struct HistoryDetail: View {
    let entry: HistoryEntry
    let history: HistoryModel
    let models: ModelsModel
    @Environment(\.dismiss) private var dismiss
    @State private var showsOriginal = false
    @State private var deleting = false
    @State private var copied = false

    private var shown: String { showsOriginal && !entry.rawText.isEmpty ? entry.rawText : entry.text }

    var body: some View {
        List {
            if !entry.rawText.isEmpty {
                Section {
                    Picker("Version", selection: $showsOriginal) {
                        Text("Cleaned up").tag(false)
                        Text("Original").tag(true)
                    }
                    .pickerStyle(.segmented)
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets())
                }
            }
            Section {
                VStack(alignment: .leading, spacing: 16) {
                    Text(shown).font(.system(.title3)).textSelection(.enabled).accessibilityIdentifier("history.text")
                    Divider()
                    Text(stats).font(.subheadline).foregroundStyle(.secondary)
                }
                .padding(.vertical, 8)
            } footer: {
                Text(showsOriginal && !entry.rawText.isEmpty ? "Exactly what the speech model heard." : cleanupNote)
            }
            Section("Details") {
                LabeledContent("Speech model", value: modelName)
                LabeledContent("Cleanup", value: entry.cleanup == "used" ? "Model" : "Rules")
                LabeledContent("Language", value: languageName(entry.language))
            }
        }
        .navigationTitle(entry.date.formatted(date: .omitted, time: .shortened))
        .navigationSubtitle(entry.date.formatted(.dateTime.weekday(.wide).day().month(.wide)))
        .navigationBarTitleDisplayMode(.inline)
        .toolbarVisibility(.hidden, for: .tabBar)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    Button("Copy cleaned up text", systemImage: "doc.on.doc") { copy(entry.text) }
                    if !entry.rawText.isEmpty { Button("Copy original", systemImage: "doc.on.doc") { copy(entry.rawText) } }
                    Button("Delete", systemImage: "trash", role: .destructive) { deleting = true }
                } label: { Image(systemName: "ellipsis") }
                    .accessibilityLabel("More")
            }
        }
        .safeAreaInset(edge: .bottom) { actions }
        .confirmationDialog("Delete this dictation?", isPresented: $deleting, titleVisibility: .visible) {
            Button("Delete", role: .destructive) {
                Task { await history.delete(entry.id); dismiss() }
            }
        } message: { Text("It is removed from this \(deviceKind).") }
        .sensoryFeedback(.success, trigger: copied) { _, now in now }
    }

    private var actions: some View {
        HStack(spacing: 12) {
            ShareLink(item: shown) { Image(systemName: "square.and.arrow.up").font(.title3).frame(width: 44, height: 44) }
                .buttonStyle(.glass).buttonBorderShape(.circle)
                .accessibilityLabel("Share")
            Button { copy(shown) } label: {
                Label(copied ? "Copied" : "Copy", systemImage: copied ? "checkmark" : "doc.on.doc")
                    .font(.headline).frame(maxWidth: .infinity, minHeight: 44)
            }
            .buttonStyle(.glassProminent).buttonBorderShape(.capsule)
            .accessibilityIdentifier("history.copy")
            Button(role: .destructive) { deleting = true } label: { Image(systemName: "trash").font(.title3).frame(width: 44, height: 44) }
                .buttonStyle(.glass).buttonBorderShape(.circle).tint(.red)
                .accessibilityLabel("Delete")
        }
        .padding(.horizontal, 16).padding(.vertical, 8)
    }

    private var stats: String {
        var parts = ["Spoke \(spokenLabel(entry.spokenMs))"]
        if entry.tookMs > 0 { parts.append("Took \(tookLabel(entry.tookMs))") }
        parts.append(wordCount(entry.words))
        return parts.joined(separator: " \u{B7} ")
    }

    private var cleanupNote: String { entry.cleanup == "used" ? "Cleaned up with a model." : "Cleaned up with rules." }

    private var modelName: String { models.models.first { $0.id == entry.modelId }?.name ?? (entry.modelId.isEmpty ? "Unknown" : entry.modelId) }

    private func copy(_ text: String) {
        UIPasteboard.general.string = text
        copied = true
        Task { try? await Task.sleep(for: .seconds(1.5)); copied = false }
    }
}
