import Foundation
import Observation

/// The dictations this phone kept, newest first, read from the phone's own history database.
@Observable @MainActor
final class HistoryModel {
    nonisolated static let pageSize: UInt32 = 50
    private let repository: PhoneRepository
    private var next: HistoryCursor?
    private var generation = 0
    private(set) var entries: [HistoryEntry] = []
    private(set) var activity: [HistoryActivity] = []
    private(set) var loaded = false
    private(set) var hasMore = false
    var query = ""
    var notice = ""

    init(repository: PhoneRepository) { self.repository = repository }

    /// Reloads the first page for the current search and the numbers behind Home and Stats.
    func refresh() async {
        generation += 1
        let token = generation
        let query = query.trimmingCharacters(in: .whitespacesAndNewlines)
        do {
            let (page, activity) = try await Task.detached { [repository] in
                (try repository.history.page(query: query, after: nil, limit: Self.pageSize), try repository.history.activity(sinceMs: 0))
            }.value
            guard token == generation else { return }
            entries = page.entries
            next = page.next
            hasMore = page.next != nil
            self.activity = activity
            loaded = true
            notice = ""
        } catch { notice = phoneError(error) }
    }

    func loadMore() async {
        guard let cursor = next else { return }
        let token = generation
        let query = query.trimmingCharacters(in: .whitespacesAndNewlines)
        do {
            let page = try await Task.detached { [repository] in
                try repository.history.page(query: query, after: cursor, limit: Self.pageSize)
            }.value
            guard token == generation, next == cursor else { return }
            entries += page.entries
            next = page.next
            hasMore = page.next != nil
        } catch { notice = phoneError(error) }
    }

    func delete(_ id: String) async {
        do {
            try await Task.detached { [repository] in try repository.history.delete(id: id) }.value
            notice = ""
        } catch { notice = phoneError(error) }
        await refresh()
    }

    func clear() async {
        do {
            try await Task.detached { [repository] in try repository.history.clear() }.value
            notice = ""
        } catch { notice = phoneError(error) }
        await refresh()
    }

    /// "Keep History": removes what is older than `days`. Zero keeps everything.
    func prune(keepingDays days: Int, now: Date = Date()) async {
        guard days > 0, let cutoff = Calendar.current.date(byAdding: .day, value: -days, to: now) else { return }
        let before = Int64(cutoff.timeIntervalSince1970 * 1000)
        do { try await Task.detached { [repository] in try repository.history.prune(beforeMs: before) }.value }
        catch { notice = phoneError(error) }
        await refresh()
    }
}

extension HistoryEntry: Identifiable {
    var date: Date { Date(timeIntervalSince1970: Double(createdMs) / 1000) }
}
