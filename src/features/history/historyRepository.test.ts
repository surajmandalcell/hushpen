import { afterEach, describe, expect, it } from "vitest";
import type { TranscriptHistoryRow } from "../../types/history";
import { localStorageHistoryRepository } from "./historyRepository";

const storageKey = "asrpro.transcriptHistory.v1";

function buildRow(id: string, overrides: Partial<TranscriptHistoryRow> = {}): TranscriptHistoryRow {
  return {
    id,
    title: `Title ${id}`,
    text: `Text ${id}`,
    kind: "Dictation",
    model: "Whisper Base English",
    durationSeconds: 4,
    createdAt: 1000,
    status: "completed",
    ...overrides,
  };
}

afterEach(() => {
  window.localStorage.clear();
});

describe("localStorageHistoryRepository", () => {
  it("lists nothing when no history is stored", () => {
    expect(localStorageHistoryRepository.list()).toEqual([]);
  });

  it("round-trips saved rows under the existing storage key", () => {
    const rows = [buildRow("a"), buildRow("b", { status: "failed", error: "Engine failed" })];

    localStorageHistoryRepository.save(rows);

    expect(JSON.parse(window.localStorage.getItem(storageKey) ?? "[]")).toHaveLength(2);
    expect(localStorageHistoryRepository.list()).toEqual([
      { ...rows[0], recordingUrl: undefined, transcriptFilePath: undefined, error: undefined },
      { ...rows[1], recordingUrl: undefined, transcriptFilePath: undefined },
    ]);
  });

  it("keeps only the newest 100 rows when saving", () => {
    const rows = Array.from({ length: 130 }, (_, index) => buildRow(`row-${index}`));

    localStorageHistoryRepository.save(rows);

    const stored = JSON.parse(window.localStorage.getItem(storageKey) ?? "[]") as TranscriptHistoryRow[];
    expect(stored).toHaveLength(100);
    expect(stored[0].id).toBe("row-0");
    expect(stored[99].id).toBe("row-99");
  });
});
