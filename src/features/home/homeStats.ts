import { countWords } from "../../lib/format";
import type { TranscriptHistoryRow } from "../../types/history";

export function buildHomeStats(rows: TranscriptHistoryRow[]) {
  const completedRows = rows.filter((row) => row.status === "completed");
  const wordsThisWeek = completedRows.reduce((total, row) => total + countWords(row.text), 0);
  const spokenSeconds = completedRows.reduce((total, row) => total + row.durationSeconds, 0);
  const avgWpm = spokenSeconds > 0 ? Math.round(wordsThisWeek / (spokenSeconds / 60)) : 0;
  const savedMinutes = Math.max(0, Math.round(wordsThisWeek / 42));

  return [
    { value: `${avgWpm} WPM`, label: "Average speed" },
    { value: String(wordsThisWeek), label: "Words this week" },
    { value: String(rows.length), label: "Recordings" },
    { value: savedMinutes ? `${savedMinutes} minute${savedMinutes === 1 ? "" : "s"}` : "0 minutes", label: "Saved this week" },
  ];
}
