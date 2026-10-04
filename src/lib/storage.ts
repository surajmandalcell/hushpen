import type { EngineModelInfo } from "../types/engine";
import type { TranscriptHistoryRow } from "../types/history";
import { defaultAudioInputId, defaultModelName, fallbackModelCards } from "./defaults";
import { buildHistoryTitle } from "./history";

const transcriptHistoryStorageKey = "asrpro.transcriptHistory.v1";
const audioInputDeviceStorageKey = "asrpro.audioInputDevice.v1";
const selectedModelStorageKey = "asrpro.selectedModel.v1";
const seededScreenshotHistoryIdPrefix = "readme-history-";
const seededScreenshotHistoryRows = new Map([
  ["Product demo follow-up", "Summarize the product demo, send the follow-up notes, and schedule the model comparison review."],
  ["Roadmap voice note", "Keep the desktop release private first, tighten screenshot checks, and verify the packaged runtime before sharing."],
  ["Audio file transcript", "The imported audio sample should stay in history with model details and a replayable local recording."],
]);

export function loadSelectedAudioInputId() {
  try {
    const stored = window.localStorage.getItem(audioInputDeviceStorageKey);
    return stored && stored.trim() ? stored : defaultAudioInputId;
  } catch {
    return defaultAudioInputId;
  }
}

export function saveSelectedAudioInputId(deviceId: string) {
  try {
    window.localStorage.setItem(audioInputDeviceStorageKey, deviceId);
  } catch {
    // Local storage failures should not block recording.
  }
}

export function normalizeSelectedModelName(value: unknown, models: EngineModelInfo[] = fallbackModelCards) {
  if (typeof value !== "string" || !value.trim()) return undefined;

  const normalized = value.trim();
  const byName = models.find((model) => model.displayName === normalized);
  if (byName) return byName.displayName;

  const byId = models.find((model) => model.id === normalized);
  return byId?.displayName;
}

export function loadSelectedModelName(models: EngineModelInfo[] = fallbackModelCards) {
  try {
    return normalizeSelectedModelName(window.localStorage.getItem(selectedModelStorageKey), models);
  } catch {
    return undefined;
  }
}

export function saveSelectedModelName(modelName: string) {
  try {
    window.localStorage.setItem(selectedModelStorageKey, modelName);
  } catch {
    // Local storage failures should not block recognition.
  }
}

export function loadTranscriptHistory() {
  try {
    const raw = window.localStorage.getItem(transcriptHistoryStorageKey);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];

    const rows: TranscriptHistoryRow[] = [];
    for (const item of parsed) {
      const row = normalizeTranscriptHistoryRow(item);
      if (row) rows.push(row);
    }

    const sanitized = sanitizeTranscriptHistoryRows(rows);
    if (sanitized.removedSeededRows) {
      saveTranscriptHistory(sanitized.rows);
    }

    return sanitized.rows;
  } catch {
    return [];
  }
}

function sanitizeTranscriptHistoryRows(rows: TranscriptHistoryRow[]) {
  if (window.asrpro?.isScreenshotMode) {
    return { rows, removedSeededRows: false };
  }

  const sanitizedRows = rows.filter((row) => !isSeededScreenshotHistoryRow(row));
  return {
    rows: sanitizedRows,
    removedSeededRows: sanitizedRows.length !== rows.length,
  };
}

function isSeededScreenshotHistoryRow(row: TranscriptHistoryRow) {
  if (row.recordingUrl) return false;
  if (row.id.startsWith(seededScreenshotHistoryIdPrefix)) return true;

  return seededScreenshotHistoryRows.get(row.title) === row.text;
}

export function normalizeTranscriptHistoryRow(value: unknown): TranscriptHistoryRow | null {
  if (!value || typeof value !== "object") return null;

  const row = value as Partial<TranscriptHistoryRow>;
  const text = typeof row.text === "string" ? row.text : "";
  const title = typeof row.title === "string" && row.title.trim() ? row.title : buildHistoryTitle(text);
  const kind = row.kind === "File" ? "File" : "Dictation";
  const status = row.status === "failed" ? "failed" : "completed";

  return {
    id: typeof row.id === "string" && row.id ? row.id : `history-${Date.now()}`,
    title,
    text,
    kind,
    model: typeof row.model === "string" && row.model ? row.model : defaultModelName,
    durationSeconds: Number.isFinite(row.durationSeconds) ? Math.max(0, Math.round(Number(row.durationSeconds))) : 0,
    createdAt: Number.isFinite(row.createdAt) ? Number(row.createdAt) : Date.now(),
    status,
    recordingUrl: typeof row.recordingUrl === "string" && row.recordingUrl ? row.recordingUrl : undefined,
    transcriptFilePath: typeof row.transcriptFilePath === "string" && row.transcriptFilePath ? row.transcriptFilePath : undefined,
    error: typeof row.error === "string" ? row.error : undefined,
  };
}

export function saveTranscriptHistory(rows: TranscriptHistoryRow[]) {
  try {
    window.localStorage.setItem(transcriptHistoryStorageKey, JSON.stringify(rows.slice(0, 100)));
  } catch {
    // Local history should never break the recording flow.
  }
}
