import type { TranscriptHistoryRow } from "../types/history";

export function buildHistoryTitle(text: string) {
  const compact = text.replace(/\s+/g, " ").trim();
  if (!compact) return "Untitled dictation";
  return compact.length > 92 ? `${compact.slice(0, 89)}...` : compact;
}

export function createTranscriptHistoryRow({
  text,
  model,
  durationSeconds,
  startedAt,
  recordingUrl,
}: {
  text: string;
  model: string;
  durationSeconds: number;
  startedAt: number;
  recordingUrl: string;
}): TranscriptHistoryRow {
  const normalizedText = text.replace(/\s+/g, " ").trim();

  return {
    id: `dictation-${startedAt}-${Math.random().toString(36).slice(2, 8)}`,
    title: buildHistoryTitle(normalizedText),
    text: normalizedText,
    kind: "Dictation",
    model,
    durationSeconds,
    createdAt: Date.now(),
    status: "completed",
    recordingUrl,
  };
}
