import { useCallback, useEffect, useMemo, useRef, useState, type ButtonHTMLAttributes, type ReactNode } from "react";
import {
  ArrowUpRight,
  BrainCircuit,
  Bluetooth,
  Bug,
  Camera,
  Check,
  CheckCircle2,
  Copy,
  FileText,
  Github,
  Headphones,
  History,
  Home,
  Info,
  Library,
  Laptop,
  Mic2,
  Minus,
  Pause,
  Play,
  RefreshCw,
  Search,
  Settings,
  Smartphone,
  Trash2,
  Usb,
  Volume2,
  VolumeX,
  X,
  type LucideIcon,
} from "lucide-react";
import packageMetadata from "../package.json";
import { AppLogoMark } from "./components/icons";
import { audioRecordingService } from "./services/audioRecording";

type ViewId = "home" | "configuration" | "sound" | "models" | "history" | "about";
type WindowAction = "minimize" | "close";
type OverlayPlacement = "top" | "bottom";
type RecordingStatus = "idle" | "starting" | "recording" | "preparing-engine" | "transcribing" | "error";

interface OverlaySettings {
  placement: OverlayPlacement;
  customBounds: {
    displayId: number;
    x: number;
    y: number;
  } | null;
}

interface RuntimeInfo {
  isRecording: boolean;
  defaultModel?: string;
  defaultModelId?: string;
  dataDir?: string;
  overlaySettings?: OverlaySettings;
  engine?: EngineRuntimeState;
  models?: EngineModelInfo[];
  defaultTextEditor?: string;
  textEditors?: TextEditorOption[];
  shortcut?: string;
  shortcutRegistered?: boolean;
  capabilities?: {
    nativeWhisper?: boolean;
  };
}

interface AppInfo {
  name: string;
  version: string;
}

interface EngineRuntimeState {
  status: string;
  mode?: string;
  modelId?: string;
  model?: string;
  detail?: string;
  progress?: number | null;
  error?: string | null;
  updatedAt?: string;
}

interface EngineModelInfo {
  id: string;
  displayName: string;
  detail: string;
  sizeLabel: string;
  installed?: boolean;
}

interface TextEditorOption {
  id: string;
  label: string;
  detail: string;
}

interface NavItem {
  id: ViewId;
  label: string;
  icon: LucideIcon;
}

interface TranscriptHistoryRow {
  id: string;
  title: string;
  text: string;
  kind: "Dictation" | "File";
  model: string;
  durationSeconds: number;
  createdAt: number;
  status: "completed" | "failed";
  recordingUrl?: string;
  error?: string;
}

interface AudioInputDeviceOption {
  id: string;
  label: string;
}

type AudioInputDeviceIconType = "mic" | "laptop" | "phone" | "webcam" | "headphones" | "bluetooth" | "usb";

const audioInputDeviceIconByType: Record<AudioInputDeviceIconType, LucideIcon> = {
  mic: Mic2,
  laptop: Laptop,
  phone: Smartphone,
  webcam: Camera,
  headphones: Headphones,
  bluetooth: Bluetooth,
  usb: Usb,
};

function getAudioInputDeviceIconType(device: AudioInputDeviceOption): AudioInputDeviceIconType {
  const value = `${device.id} ${device.label}`.toLowerCase();

  if (device.id === "default" || value.includes("system default")) return "mic";
  if (/(iphone|ipad|android|mobile|\bphone\b)/.test(value)) return "phone";
  if (/(macbook|built-in|builtin|internal|laptop)/.test(value)) return "laptop";
  if (/(webcam|camera|facetime|logitech|brio|c920)/.test(value)) return "webcam";
  if (/(airpods|headphone|headset|earbud|earphone|buds)/.test(value)) return "headphones";
  if (/(bluetooth|\bbt\b)/.test(value)) return "bluetooth";
  if (/(usb|external|interface|focusrite|scarlett|yeti|rode|shure|elgato|studio)/.test(value)) return "usb";

  return "mic";
}

function AudioInputDeviceIcon({ device, className }: { device: AudioInputDeviceOption; className: string }) {
  const iconType = getAudioInputDeviceIconType(device);
  const Icon = audioInputDeviceIconByType[iconType];

  return <Icon aria-hidden="true" data-device-icon={iconType} className={className} />;
}

const navItems: NavItem[] = [
  { id: "home", label: "Home", icon: Home },
  { id: "configuration", label: "Configuration", icon: Settings },
  { id: "sound", label: "Sound", icon: Volume2 },
  { id: "models", label: "Models library", icon: Library },
  { id: "history", label: "History", icon: History },
  { id: "about", label: "About", icon: Info },
];

const sidebarIconTone: Record<ViewId, string> = {
  home: "bg-[#ff7a32] text-white",
  configuration: "bg-[#727272] text-white",
  sound: "bg-[#737373] text-white",
  models: "bg-[#8f8f8f] text-white",
  history: "bg-[#7167ff] text-white",
  about: "bg-[#727272] text-white",
};

const defaultModelName = "Whisper Base English";
const defaultAudioInputId = "default";
const defaultAudioInputLabel = "System default";
const defaultAudioInputOptions: AudioInputDeviceOption[] = [{ id: defaultAudioInputId, label: defaultAudioInputLabel }];
const defaultTextEditorId = "system";
const defaultTextEditorOptions: TextEditorOption[] = [
  { id: "system", label: "System default", detail: "Use the operating system default editor" },
  { id: "textedit", label: "TextEdit", detail: "Open transcript text in Apple TextEdit" },
  { id: "vscode", label: "Visual Studio Code", detail: "Open transcript text in VS Code" },
  { id: "cursor", label: "Cursor", detail: "Open transcript text in Cursor" },
];
const modelIdsByName: Record<string, string> = {
  "Whisper Tiny English": "whisper-tiny-en",
  "Whisper Base English": "whisper-base-en",
  "Whisper Small English": "whisper-small-en",
  "Whisper Base Multilingual": "whisper-base",
};
const transcriptHistoryStorageKey = "asrpro.transcriptHistory.v1";
const audioInputDeviceStorageKey = "asrpro.audioInputDevice.v1";
const seededScreenshotHistoryIdPrefix = "readme-history-";
const seededScreenshotHistoryRows = new Map([
  ["Product demo follow-up", "Summarize the product demo, send the follow-up notes, and schedule the model comparison review."],
  ["Roadmap voice note", "Keep the desktop release private first, tighten screenshot checks, and verify the packaged runtime before sharing."],
  ["Audio file transcript", "The imported audio sample should stay in history with model details and a replayable local recording."],
]);
const historyDateFormatter = new Intl.DateTimeFormat(undefined, {
  month: "short",
  day: "numeric",
});

const modelCards = [
  {
    name: "Whisper Tiny English",
    detail: "Fastest local model, lowest memory use",
    speed: "Fastest",
    status: "Active",
    disabled: false,
  },
  {
    name: "Whisper Base English",
    detail: "Default local model for English dictation",
    speed: "Default",
    status: "Active",
    disabled: false,
  },
  {
    name: "Whisper Small English",
    detail: "Higher accuracy with a larger local model",
    speed: "Accurate",
    status: "Active",
    disabled: false,
  },
  {
    name: "Whisper Base Multilingual",
    detail: "Small multilingual model with language detection",
    speed: "Multilingual",
    status: "Active",
    disabled: false,
  },
];

const appBuildVersion = typeof packageMetadata.version === "string" ? packageMetadata.version : "1.0.0";

const defaultAppInfo: AppInfo = {
  name: "ASR Pro",
  version: appBuildVersion,
};
const githubRepositoryUrl = "https://github.com/surajmandalcell/asrpro";
const githubIssueUrl = `${githubRepositoryUrl}/issues/new`;
const aboutActionLinks: Array<{ icon: LucideIcon; label: string; detail: string; href: string }> = [
  {
    icon: Github,
    label: "GitHub",
    detail: "View the project",
    href: githubRepositoryUrl,
  },
  {
    icon: Bug,
    label: "Report issue",
    detail: "Open a new issue",
    href: githubIssueUrl,
  },
];

const historyWaveformBars = Array.from({ length: 72 }, (_, index) => {
  const position = index / 71;
  const envelope = 0.42 + 0.58 * Math.sin(Math.PI * position);
  const shape = 0.42 + 0.2 * Math.sin(index * 1.7) + 0.16 * Math.sin(index * 0.53 + 1.1);
  return {
    id: `history-wave-${index}`,
    index,
    height: Math.round(clampNumber(6 + 18 * envelope * shape, 5, 24)),
  };
});

const sharedRadiusClass = "rounded-[12px]";
const insetControlRadiusClass = "rounded-[10px]";
const panelGlassClass = "rounded-[22px] border border-white/[0.095] bg-white/[0.055] backdrop-blur-2xl";
const panelSurfaceClass = `overflow-hidden ${panelGlassClass}`;
const panelDividerClass = "border-white/[0.08]";
const iconTileClass = `grid size-7 shrink-0 place-items-center ${sharedRadiusClass} bg-white/[0.07] text-[#d7d7d7]`;
const focusRingClass = "focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#9bcfff]";
const panelControlButtonClass = `inline-flex min-h-8 items-center justify-center gap-1.5 ${sharedRadiusClass} border border-[#5c5c5c] bg-[#303030] text-[12px] font-semibold text-[#eeeeee] transition hover:bg-[#3a3a3a] active:scale-[0.97] disabled:cursor-not-allowed disabled:text-[#8a8a8a] ${focusRingClass}`;
const historyActionButtonClass = `grid size-7 place-items-center ${sharedRadiusClass} text-[#bdbdbd] transition hover:bg-[#555] hover:text-white disabled:cursor-wait disabled:opacity-55`;
const dropdownSurfaceClass = `scrollbar-macos dropdown-options-scrollbar absolute z-50 max-h-64 overflow-x-hidden overflow-y-auto ${sharedRadiusClass} border border-[#5c5c5c] bg-[#303030] py-1 pl-1 pr-0.5 shadow-2xl shadow-black/40`;
const dropdownOptionButtonClass = `flex w-full min-w-0 items-start gap-2 ${insetControlRadiusClass} px-2.5 py-2 text-left text-[12px] font-semibold leading-4 transition`;
const segmentedControlClass = `inline-flex ${sharedRadiusClass} border border-white/[0.08] bg-[#2b2b2b] p-0.5`;
const segmentedItemClass = `h-7 ${insetControlRadiusClass} px-2.5 text-[12px] font-semibold transition ${focusRingClass}`;

const waveformBarCount = 76;
const waveformBaseBars = Array.from({ length: waveformBarCount }, (_, index) => {
  const position = index / Math.max(1, waveformBarCount - 1);
  const envelope = 0.36 + 0.64 * Math.sin(Math.PI * position);
  const voiceShape = 0.48
    + 0.26 * Math.sin(index * 1.39 + 0.4)
    + 0.18 * Math.sin(index * 0.47 + 1.7)
    + 0.12 * Math.sin(index * 2.13 + 0.9);
  const edgeDistance = Math.min(index, waveformBarCount - 1 - index);

  return {
    id: `wave-${index}`,
    baseHeight: Math.round(clampNumber(10 + 42 * envelope * voiceShape, 8, 46)),
    opacity: edgeDistance < 5 ? 0.34 + edgeDistance * 0.08 : 0.78,
  };
});
const idleWaveformFrame = waveformBaseBars.map((bar) => bar.baseHeight);

function clampNumber(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

function buildReactiveWaveformFrame(frequencies: Uint8Array, voiceLevel: number, timestamp: number, previousFrame: number[]) {
  return waveformBaseBars.map((bar, index) => {
    const position = index / Math.max(1, waveformBaseBars.length - 1);
    const bin = Math.min(frequencies.length - 1, Math.floor(Math.pow(position, 1.34) * frequencies.length * 0.86));
    const spectralLevel = Math.max(
      frequencies[bin] / 255,
      (frequencies[Math.min(frequencies.length - 1, bin + 2)] || 0) / 255 * 0.82,
    );
    const unevenLift = clampNumber(
      0.64
        + 0.24 * Math.sin(index * 0.83 + timestamp * 0.009)
        + 0.18 * Math.sin(index * 1.71 + timestamp * 0.006),
      0.42,
      1.14,
    );
    const target = clampNumber(bar.baseHeight + voiceLevel * (9 + spectralLevel * 48) * unevenLift, 6, 64);
    const previous = previousFrame[index] ?? bar.baseHeight;

    return Math.round((previous * 0.5 + target * 0.5) * 10) / 10;
  });
}

function toOverlayWaveformSamples(frame: number[]) {
  const overlayCount = 55;
  return Array.from({ length: overlayCount }, (_, index) => {
    const sourceIndex = Math.round((index / Math.max(1, overlayCount - 1)) * (waveformBaseBars.length - 1));
    const baseHeight = waveformBaseBars[sourceIndex]?.baseHeight ?? 8;
    const height = frame[sourceIndex] ?? baseHeight;
    return clampNumber(((height - baseHeight) / (64 - baseHeight)) * 1.45, 0, 1);
  });
}

function sendOverlayWaveformFrame(frame: number[], hasVoice: boolean) {
  window.asrpro?.setWaveformFrame?.(hasVoice ? toOverlayWaveformSamples(frame) : []);
}

function scheduleWaveformFrame(callback: FrameRequestCallback) {
  if (typeof window.requestAnimationFrame === "function") {
    return window.requestAnimationFrame(callback);
  }

  return window.setTimeout(() => callback(performance.now()), 16);
}

function cancelWaveformFrame(id: number) {
  if (typeof window.cancelAnimationFrame === "function") {
    window.cancelAnimationFrame(id);
    return;
  }

  window.clearTimeout(id);
}

function loadSelectedAudioInputId() {
  try {
    const stored = window.localStorage.getItem(audioInputDeviceStorageKey);
    return stored && stored.trim() ? stored : defaultAudioInputId;
  } catch {
    return defaultAudioInputId;
  }
}

function saveSelectedAudioInputId(deviceId: string) {
  try {
    window.localStorage.setItem(audioInputDeviceStorageKey, deviceId);
  } catch {
    // Local storage failures should not block recording.
  }
}

function buildAudioInputDeviceOptions(devices: MediaDeviceInfo[]): AudioInputDeviceOption[] {
  const options: AudioInputDeviceOption[] = [...defaultAudioInputOptions];
  const seenDeviceIds = new Set([defaultAudioInputId]);
  let unnamedAudioInputCount = 0;

  for (const device of devices) {
    if (device.kind !== "audioinput" || !device.deviceId || seenDeviceIds.has(device.deviceId)) {
      continue;
    }

    seenDeviceIds.add(device.deviceId);
    unnamedAudioInputCount += 1;
    const label = device.label.trim() || `Microphone ${unnamedAudioInputCount}`;
    options.push({ id: device.deviceId, label });
  }

  return options;
}

function normalizeTextEditorId(editorId: unknown, options: TextEditorOption[] = defaultTextEditorOptions) {
  const normalized = typeof editorId === "string" ? editorId : defaultTextEditorId;
  return options.some((option) => option.id === normalized) ? normalized : defaultTextEditorId;
}

function normalizeTextEditorOptions(options: unknown): TextEditorOption[] {
  if (!Array.isArray(options)) return defaultTextEditorOptions;

  const normalized: TextEditorOption[] = [];
  for (const option of options) {
    if (!option || typeof option !== "object") continue;
    const candidate = option as Partial<TextEditorOption>;
    if (!candidate.id || !candidate.label) continue;
    normalized.push({
      id: String(candidate.id),
      label: String(candidate.label),
      detail: candidate.detail ? String(candidate.detail) : "",
    });
  }

  return normalized.length ? normalized : defaultTextEditorOptions;
}

function loadTranscriptHistory() {
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

function normalizeTranscriptHistoryRow(value: unknown): TranscriptHistoryRow | null {
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
    error: typeof row.error === "string" ? row.error : undefined,
  };
}

function saveTranscriptHistory(rows: TranscriptHistoryRow[]) {
  try {
    window.localStorage.setItem(transcriptHistoryStorageKey, JSON.stringify(rows.slice(0, 100)));
  } catch {
    // Local history should never break the recording flow.
  }
}

function buildHistoryTitle(text: string) {
  const compact = text.replace(/\s+/g, " ").trim();
  if (!compact) return "Untitled dictation";
  return compact.length > 92 ? `${compact.slice(0, 89)}...` : compact;
}

function formatDuration(seconds: number) {
  const rounded = Math.max(0, Math.round(seconds));
  const minutes = Math.floor(rounded / 60);
  const remainingSeconds = rounded % 60;
  return `${minutes}:${remainingSeconds.toString().padStart(2, "0")}`;
}

function formatHistoryGroupLabel(createdAt: number, now = Date.now()) {
  const elapsedDays = Math.max(0, Math.floor((startOfDay(now) - startOfDay(createdAt)) / 86_400_000));
  if (elapsedDays === 0) return "Today";
  if (elapsedDays === 1) return "Yesterday";
  if (elapsedDays < 30) return `${elapsedDays} days ago`;
  return historyDateFormatter.format(new Date(createdAt));
}

function countWords(text: string) {
  return text.trim().split(/\s+/).filter(Boolean).length;
}

function formatHomeRelativePath(filePath?: string) {
  if (!filePath) return undefined;
  return filePath.replace(/^\/(?:Users|home)\/[^/]+(?=\/|$)/, "~");
}

function buildAboutFactRows(appVersion: string, storagePath?: string): Array<{ label: string; value: string }> {
  return [
    { label: "Version", value: appVersion },
    { label: "Recognition", value: "Private dictation and file transcription" },
    { label: "Data folder", value: formatHomeRelativePath(storagePath) || "Waiting for local data folder" },
  ];
}

function buildHomeStats(rows: TranscriptHistoryRow[]) {
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

function startOfDay(timestamp: number) {
  const date = new Date(timestamp);
  date.setHours(0, 0, 0, 0);
  return date.getTime();
}

function formatShortcutParts(shortcut?: string) {
  const normalized = (shortcut || "CommandOrControl+`").split("+").flatMap((part) => {
    const trimmed = part.trim();
    return trimmed ? [trimmed] : [];
  });

  return normalized.map((part) => {
    if (part === "CommandOrControl" || part === "Command" || part === "Meta") return "⌘";
    if (part === "Control" || part === "Ctrl") return "⌃";
    if (part === "Alt" || part === "Option") return "⌥";
    if (part === "Shift") return "⇧";
    if (part === "Escape") return "esc";
    return part.replace("Backquote", "`");
  });
}

function getErrorMessage(error: unknown) {
  const message = error instanceof Error ? error.message : "Recording failed";

  if (/No handler registered|Error invoking remote method/i.test(message)) {
    return "Native Whisper engine needs restart. Restart ASR Pro, then try again.";
  }

  if (/Model download failed|checksum mismatch/i.test(message)) {
    return "Whisper model download failed. Check your connection and try again.";
  }

  if (/native Whisper addon|whisper\.node|libwhisper/i.test(message)) {
    return "Native Whisper engine could not load. Reinstall dependencies, then restart ASR Pro.";
  }

  if (/failed to fetch|load failed|networkerror|network request failed/i.test(message)) {
    return "Failed to load.";
  }

  return message;
}

function getRecordingErrorTitle(message: string) {
  if (/needs restart/i.test(message)) return "Engine needs restart";
  if (/Engine unavailable|Failed to load|download failed/i.test(message)) return "Engine unavailable";
  return "Recording failed";
}

function createTranscriptHistoryRow({
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

async function createTranscriptionAudioPayload(blob: Blob) {
  const wavBlob = await convertBlobToWav(blob).catch(() => blob);

  return {
    audioData: await wavBlob.arrayBuffer(),
    mimeType: wavBlob.type || blob.type || "audio/wav",
  };
}

function dataUrlToBlob(dataUrl: string) {
  if (!dataUrl.startsWith("data:")) {
    throw new Error("Saved source audio could not be loaded.");
  }

  const commaIndex = dataUrl.indexOf(",");
  if (commaIndex < 0) {
    throw new Error("Saved source audio could not be loaded.");
  }

  const header = dataUrl.slice(5, commaIndex);
  const payload = dataUrl.slice(commaIndex + 1);
  const headerParts = header.split(";").filter(Boolean);
  const mimeType = headerParts[0] || "audio/webm";
  const isBase64 = headerParts.includes("base64");

  try {
    const bytes = isBase64
      ? Uint8Array.from(window.atob(payload), (character) => character.charCodeAt(0))
      : new TextEncoder().encode(decodeURIComponent(payload));

    return new Blob([bytes], { type: mimeType });
  } catch {
    throw new Error("Saved source audio could not be loaded.");
  }
}

async function convertBlobToWav(blob: Blob) {
  if (blob.type.includes("wav")) return blob;

  const AudioContextCtor = window.AudioContext || (window as Window & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!AudioContextCtor) return blob;

  const audioContext = new AudioContextCtor();
  if (typeof audioContext.decodeAudioData !== "function") {
    await audioContext.close?.().catch(() => {});
    return blob;
  }

  const sourceData = await blob.arrayBuffer();
  const decoded = await audioContext.decodeAudioData(sourceData.slice(0));
  await audioContext.close?.().catch(() => {});
  const monoSamples = mixAudioBufferToMono(decoded);
  const samples = resamplePcm(monoSamples, decoded.sampleRate, 16000);
  const wavData = encodePcm16Wav(samples, 16000);

  return new Blob([wavData], { type: "audio/wav" });
}

function mixAudioBufferToMono(audioBuffer: AudioBuffer) {
  const samples = new Float32Array(audioBuffer.length);
  const channelCount = Math.max(1, audioBuffer.numberOfChannels);

  for (let channel = 0; channel < channelCount; channel += 1) {
    const channelData = audioBuffer.getChannelData(channel);
    for (let index = 0; index < samples.length; index += 1) {
      samples[index] += channelData[index] / channelCount;
    }
  }

  return samples;
}

function resamplePcm(samples: Float32Array, sourceRate: number, targetRate: number) {
  if (sourceRate === targetRate) return samples;

  const targetLength = Math.max(1, Math.round(samples.length * targetRate / sourceRate));
  const resampled = new Float32Array(targetLength);
  const ratio = (samples.length - 1) / Math.max(1, targetLength - 1);

  for (let index = 0; index < targetLength; index += 1) {
    const sourceIndex = index * ratio;
    const lower = Math.floor(sourceIndex);
    const upper = Math.min(samples.length - 1, lower + 1);
    const weight = sourceIndex - lower;
    resampled[index] = samples[lower] * (1 - weight) + samples[upper] * weight;
  }

  return resampled;
}

function encodePcm16Wav(samples: Float32Array, sampleRate: number) {
  const bytesPerSample = 2;
  const dataLength = samples.length * bytesPerSample;
  const buffer = new ArrayBuffer(44 + dataLength);
  const view = new DataView(buffer);

  writeAscii(view, 0, "RIFF");
  view.setUint32(4, 36 + dataLength, true);
  writeAscii(view, 8, "WAVE");
  writeAscii(view, 12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * bytesPerSample, true);
  view.setUint16(32, bytesPerSample, true);
  view.setUint16(34, 8 * bytesPerSample, true);
  writeAscii(view, 36, "data");
  view.setUint32(40, dataLength, true);

  let offset = 44;
  for (const sample of samples) {
    const clamped = Math.max(-1, Math.min(1, sample));
    view.setInt16(offset, Math.round(clamped < 0 ? clamped * 32768 : clamped * 32767), true);
    offset += bytesPerSample;
  }

  return buffer;
}

function writeAscii(view: DataView, offset: number, value: string) {
  for (let index = 0; index < value.length; index += 1) {
    view.setUint8(offset + index, value.charCodeAt(index));
  }
}

function readBlobAsDataUrl(blob: Blob) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error("Failed to save recording audio"));
    reader.onload = () => {
      if (typeof reader.result === "string") {
        resolve(reader.result);
        return;
      }

      reject(new Error("Failed to save recording audio"));
    };
    reader.readAsDataURL(blob);
  });
}

function useMicrophoneWaveform(active: boolean) {
  const frameRef = useRef<number[]>(idleWaveformFrame);
  const lastOverlayFrameAtRef = useRef(0);
  const audioLevelRef = useRef(0);

  useEffect(() => {
    return audioRecordingService.subscribe((state) => {
      audioLevelRef.current = state.audioLevel;
    });
  }, []);

  useEffect(() => {
    if (!active) {
      frameRef.current = idleWaveformFrame;
      sendOverlayWaveformFrame(idleWaveformFrame, false);
      return undefined;
    }

    let stopped = false;
    let animationFrame = 0;
    const frequencySamples = new Uint8Array(64);

    const tick = (timestamp: number) => {
      if (stopped) return;

      const voiceLevel = clampNumber(audioLevelRef.current, 0, 1);

      if (voiceLevel <= 0.012) {
        if (frameRef.current !== idleWaveformFrame) {
          frameRef.current = idleWaveformFrame;
        }

        if (timestamp - lastOverlayFrameAtRef.current > 120) {
          sendOverlayWaveformFrame(idleWaveformFrame, false);
          lastOverlayFrameAtRef.current = timestamp;
        }
      } else {
        for (let index = 0; index < frequencySamples.length; index += 1) {
          frequencySamples[index] = Math.round(clampNumber(voiceLevel * 210 + Math.sin(timestamp * 0.008 + index * 0.4) * 26, 0, 255));
        }

        const nextFrame = buildReactiveWaveformFrame(frequencySamples, voiceLevel, timestamp, frameRef.current);

        frameRef.current = nextFrame;

        if (timestamp - lastOverlayFrameAtRef.current > 16) {
          sendOverlayWaveformFrame(nextFrame, true);
          lastOverlayFrameAtRef.current = timestamp;
        }
      }

      animationFrame = scheduleWaveformFrame(tick);
    };

    animationFrame = scheduleWaveformFrame(tick);

    return () => {
      stopped = true;
      if (animationFrame) cancelWaveformFrame(animationFrame);
      frameRef.current = idleWaveformFrame;
      sendOverlayWaveformFrame(idleWaveformFrame, false);
    };
  }, [active]);
}

function App() {
  const [activeView, setActiveView] = useState<ViewId>("home");
  const [isRecording, setIsRecording] = useState(false);
  const [recordingStatus, setRecordingStatus] = useState<RecordingStatus>("idle");
  const [recordingError, setRecordingError] = useState<string | null>(null);
  const [recordingDurationSeconds, setRecordingDurationSeconds] = useState(0);
  const [selectedModel, setSelectedModel] = useState(defaultModelName);
  const [audioInputDevices, setAudioInputDevices] = useState<AudioInputDeviceOption[]>(defaultAudioInputOptions);
  const [selectedAudioInputId, setSelectedAudioInputId] = useState(loadSelectedAudioInputId);
  const [audioInputDevicesLoading, setAudioInputDevicesLoading] = useState(false);
  const [audioInputDevicesError, setAudioInputDevicesError] = useState<string | null>(null);
  const [runtimeInfo, setRuntimeInfo] = useState<RuntimeInfo | null>(null);
  const [appInfo, setAppInfo] = useState<AppInfo>(defaultAppInfo);
  const [overlayPlacement, setOverlayPlacement] = useState<OverlayPlacement>("top");
  const [textEditorOptions, setTextEditorOptions] = useState<TextEditorOption[]>(defaultTextEditorOptions);
  const [selectedTextEditorId, setSelectedTextEditorId] = useState(defaultTextEditorId);
  const [historyRows, setHistoryRows] = useState<TranscriptHistoryRow[]>(loadTranscriptHistory);
  const [reprocessingHistoryRowId, setReprocessingHistoryRowId] = useState<string | null>(null);
  const [openingTranscriptRowId, setOpeningTranscriptRowId] = useState<string | null>(null);
  const [isScrollbarVisible, setIsScrollbarVisible] = useState(true);
  const recordingStartedAtRef = useRef<number | null>(null);
  const recordingTransitionRef = useRef<"starting" | "stopping" | null>(null);
  const scrollbarTimerRef = useRef<number | null>(null);
  const overlayPlacementTouchedRef = useRef(false);
  useMicrophoneWaveform(isRecording);

  const addHistoryRow = useCallback((row: TranscriptHistoryRow) => {
    setHistoryRows((current) => {
      const next = [row, ...current].slice(0, 100);
      saveTranscriptHistory(next);
      return next;
    });
  }, []);

  const updateHistoryRow = useCallback((rowId: string, updater: (row: TranscriptHistoryRow) => TranscriptHistoryRow) => {
    setHistoryRows((current) => {
      const next = current.map((row) => (row.id === rowId ? updater(row) : row));
      saveTranscriptHistory(next);
      return next;
    });
  }, []);

  const deleteHistoryRow = useCallback((rowId: string) => {
    setHistoryRows((current) => {
      const next = current.filter((row) => row.id !== rowId);
      saveTranscriptHistory(next);
      return next;
    });
  }, []);

  const copyHistoryText = useCallback((text: string) => {
    void navigator.clipboard?.writeText(text).catch(() => {});
  }, []);

  const showScrollbarTemporarily = useCallback((durationMs = 1200) => {
    setIsScrollbarVisible(true);
    if (scrollbarTimerRef.current) {
      window.clearTimeout(scrollbarTimerRef.current);
    }

    scrollbarTimerRef.current = window.setTimeout(() => {
      setIsScrollbarVisible(false);
      scrollbarTimerRef.current = null;
    }, durationMs);
  }, []);

  const syncRecordingBridge = useCallback(async (active: boolean) => {
    const api = window.asrpro;
    if (!api?.setRecording) return;

    try {
      const state = await api.setRecording(active);
      setRuntimeInfo((current) => (current ? { ...current, isRecording: state.isRecording } : current));
    } catch {
      setRuntimeInfo((current) => (current ? { ...current, isRecording: active } : current));
    }
  }, []);

  const transcribeRecording = useCallback(async (audioBlob: Blob) => {
    const transcribeAudio = window.asrpro?.transcribeAudio;
    if (!transcribeAudio) {
      throw new Error("Native Whisper engine is not available.");
    }

    const payload = await createTranscriptionAudioPayload(audioBlob);
    return transcribeAudio({
      ...payload,
      modelId: modelIdsByName[selectedModel] ?? "whisper-base-en",
    });
  }, [selectedModel]);

  const reprocessHistoryRow = useCallback(async (row: TranscriptHistoryRow) => {
    if (!row.recordingUrl || reprocessingHistoryRowId) return;

    setReprocessingHistoryRowId(row.id);

    try {
      const result = await transcribeRecording(dataUrlToBlob(row.recordingUrl));
      const text = typeof result === "string" ? result : result?.text;
      if (!text || !text.trim()) {
        throw new Error("No transcription text returned");
      }

      const normalizedText = text.replace(/\s+/g, " ").trim();
      updateHistoryRow(row.id, (current) => ({
        ...current,
        title: buildHistoryTitle(normalizedText),
        text: normalizedText,
        model: selectedModel,
        status: "completed",
        error: undefined,
      }));
    } catch (error) {
      const message = getErrorMessage(error);
      updateHistoryRow(row.id, (current) => ({
        ...current,
        text: current.status === "failed" || !current.text.trim() ? message : current.text,
        title: current.status === "failed" || !current.title.trim() ? getRecordingErrorTitle(message) : current.title,
        model: selectedModel,
        status: "failed",
        error: message,
      }));
    } finally {
      setReprocessingHistoryRowId(null);
    }
  }, [reprocessingHistoryRowId, selectedModel, transcribeRecording, updateHistoryRow]);

  const openHistoryTranscriptText = useCallback(async (row: TranscriptHistoryRow) => {
    if (!row.text.trim() || openingTranscriptRowId) return;

    setOpeningTranscriptRowId(row.id);

    try {
      const request = {
        title: row.title,
        text: row.text,
      };

      if (window.asrpro?.openTranscriptText) {
        await window.asrpro.openTranscriptText(request);
        return;
      }

      const blobUrl = URL.createObjectURL(new Blob([`${row.text.trim()}\n`], { type: "text/plain;charset=utf-8" }));
      window.open(blobUrl, "_blank", "noopener,noreferrer");
      window.setTimeout(() => URL.revokeObjectURL(blobUrl), 60_000);
    } finally {
      setOpeningTranscriptRowId(null);
    }
  }, [openingTranscriptRowId]);

  const refreshAudioInputDevices = useCallback(async () => {
    const mediaDevices = navigator.mediaDevices;

    if (!mediaDevices?.enumerateDevices) {
      setAudioInputDevices(defaultAudioInputOptions);
      setAudioInputDevicesError("Microphone list is not available.");
      setSelectedAudioInputId(defaultAudioInputId);
      saveSelectedAudioInputId(defaultAudioInputId);
      return;
    }

    setAudioInputDevicesLoading(true);
    setAudioInputDevicesError(null);

    try {
      const devices = await mediaDevices.enumerateDevices();
      const nextOptions = buildAudioInputDeviceOptions(devices);

      setAudioInputDevices(nextOptions);
      setSelectedAudioInputId((current) => {
        const nextDeviceId = nextOptions.some((device) => device.id === current) ? current : defaultAudioInputId;
        if (nextDeviceId !== current) {
          saveSelectedAudioInputId(nextDeviceId);
        }
        return nextDeviceId;
      });
    } catch {
      setAudioInputDevices(defaultAudioInputOptions);
      setAudioInputDevicesError("Microphone list could not be loaded.");
      setSelectedAudioInputId(defaultAudioInputId);
      saveSelectedAudioInputId(defaultAudioInputId);
    } finally {
      setAudioInputDevicesLoading(false);
    }
  }, []);

  const handleAudioInputChange = useCallback((deviceId: string) => {
    setSelectedAudioInputId(deviceId);
    saveSelectedAudioInputId(deviceId);
  }, []);

  const handleTextEditorChange = useCallback((editorId: string) => {
    const normalizedEditorId = normalizeTextEditorId(editorId, textEditorOptions);
    setSelectedTextEditorId(normalizedEditorId);
    setRuntimeInfo((current) => (current ? { ...current, defaultTextEditor: normalizedEditorId } : current));

    const saveTextEditor = window.asrpro?.setDefaultTextEditor?.(normalizedEditorId);
    if (!saveTextEditor) return;

    saveTextEditor.then((settings) => {
      const nextEditorId = normalizeTextEditorId(settings.defaultTextEditor, textEditorOptions);
      setSelectedTextEditorId(nextEditorId);
      setRuntimeInfo((current) => (current ? { ...current, defaultTextEditor: nextEditorId } : current));
    }).catch(() => {});
  }, [textEditorOptions]);

  const selectedAudioInputLabel = useMemo(() => (
    audioInputDevices.find((device) => device.id === selectedAudioInputId)?.label ?? defaultAudioInputLabel
  ), [audioInputDevices, selectedAudioInputId]);

  const selectedTextEditorLabel = useMemo(() => (
    textEditorOptions.find((editor) => editor.id === selectedTextEditorId)?.label ?? defaultTextEditorOptions[0].label
  ), [selectedTextEditorId, textEditorOptions]);

  const startRecordingFlow = useCallback(async (syncBridge = true) => {
    if (recordingTransitionRef.current || audioRecordingService.isRecording()) {
      return;
    }

    recordingTransitionRef.current = "starting";
    setRecordingStatus("starting");
    setRecordingError(null);

    try {
      await audioRecordingService.startRecording({
        sampleRate: 16000,
        channelCount: 1,
        deviceId: selectedAudioInputId === defaultAudioInputId ? undefined : selectedAudioInputId,
        echoCancellation: true,
        noiseSuppression: true,
      });
      const startedAt = Date.now();
      recordingStartedAtRef.current = startedAt;
      setRecordingDurationSeconds(0);
      setIsRecording(true);
      setRecordingStatus("recording");
      setRuntimeInfo((current) => (current ? { ...current, isRecording: true } : current));
      if (syncBridge) {
        await syncRecordingBridge(true);
      }
    } catch (error) {
      const message = getErrorMessage(error);
      setIsRecording(false);
      setRecordingStatus("error");
      setRecordingError(message);
      setRuntimeInfo((current) => (current ? { ...current, isRecording: false } : current));
      if (syncBridge) {
        await syncRecordingBridge(false);
      }
    } finally {
      recordingTransitionRef.current = null;
    }
  }, [selectedAudioInputId, syncRecordingBridge]);

  const stopRecordingFlow = useCallback(async (syncBridge = true) => {
    if (recordingTransitionRef.current === "stopping") {
      return;
    }

    const wasRecording = audioRecordingService.isRecording();
    const startedAt = recordingStartedAtRef.current ?? Date.now();
    const durationSeconds = Math.max(0, Math.round((Date.now() - startedAt) / 1000));

    recordingTransitionRef.current = "stopping";
    setIsRecording(false);
    setRecordingDurationSeconds(durationSeconds);
    setRecordingStatus(wasRecording ? "preparing-engine" : "idle");
    setRuntimeInfo((current) => (current ? { ...current, isRecording: false } : current));

    let recordingUrl: string | undefined;

    try {
      if (syncBridge) {
        await syncRecordingBridge(false);
      }

      if (!wasRecording) {
        return;
      }

      const audioBlob = await audioRecordingService.stopRecording();
      if (!audioBlob || audioBlob.size === 0) {
        throw new Error("No audio was captured");
      }

      recordingUrl = await readBlobAsDataUrl(audioBlob);
      setRecordingStatus("preparing-engine");
      setRecordingStatus("transcribing");
      const result = await transcribeRecording(audioBlob);
      const text = typeof result === "string" ? result : result?.text;
      if (!text || !text.trim()) {
        throw new Error("No transcription text returned");
      }

      addHistoryRow(createTranscriptHistoryRow({
        text,
        model: selectedModel,
        durationSeconds,
        startedAt,
        recordingUrl,
      }));
      setRecordingStatus("idle");
      setRecordingError(null);
    } catch (error) {
      const message = getErrorMessage(error);
      setRecordingStatus("error");
      setRecordingError(message);
      addHistoryRow({
        id: `dictation-error-${startedAt}`,
        title: "Recording failed to transcribe",
        text: message,
        kind: "Dictation",
        model: selectedModel,
        durationSeconds,
        createdAt: Date.now(),
        status: "failed",
        error: message,
        recordingUrl,
      });
    } finally {
      recordingStartedAtRef.current = null;
      recordingTransitionRef.current = null;
    }
  }, [addHistoryRow, selectedModel, syncRecordingBridge, transcribeRecording]);

  useEffect(() => {
    const api = window.asrpro;
    if (!api) return undefined;

    if (api.getAppInfo) {
      Promise.resolve(api.getAppInfo()).then((info) => {
        if (!info) return;
        setAppInfo((current) => ({
          name: info.name || current.name,
          version: info.version || current.version,
        }));
      }).catch(() => {});
    }

    if (api.getRuntimeState) {
      Promise.resolve(api.getRuntimeState()).then((state) => {
        if (!state) return;
        setRuntimeInfo(state);
        setSelectedModel(state.defaultModel || defaultModelName);
        const nextTextEditorOptions = normalizeTextEditorOptions(state.textEditors);
        setTextEditorOptions(nextTextEditorOptions);
        setSelectedTextEditorId(normalizeTextEditorId(state.defaultTextEditor, nextTextEditorOptions));
        if (!overlayPlacementTouchedRef.current) {
          setOverlayPlacement(normalizeOverlayPlacement(state.overlaySettings?.placement));
        }
        if (state.isRecording) {
          void startRecordingFlow(false);
        } else if (!recordingTransitionRef.current && !audioRecordingService.isRecording()) {
          setIsRecording(false);
        }
      }).catch(() => {});
    }

    const unsubscribeRecording = api.onRecordingState?.((state) => {
      setRuntimeInfo((current) => (current ? { ...current, isRecording: state.isRecording } : current));
      if (recordingTransitionRef.current) {
        setIsRecording(state.isRecording);
        return;
      }

      if (state.isRecording) {
        void startRecordingFlow(false);
      } else {
        void stopRecordingFlow(false);
      }
    });

    const unsubscribeEngine = api.onEngineState?.((engineState) => {
      setRuntimeInfo((current) => (current ? { ...current, engine: engineState } : { isRecording: false, engine: engineState }));
    });

    return () => {
      unsubscribeRecording?.();
      unsubscribeEngine?.();
    };
  }, [startRecordingFlow, stopRecordingFlow]);

  useEffect(() => {
    void refreshAudioInputDevices();

    const mediaDevices = navigator.mediaDevices;
    if (!mediaDevices?.addEventListener) {
      return undefined;
    }

    const handleDeviceChange = () => {
      void refreshAudioInputDevices();
    };

    mediaDevices.addEventListener("devicechange", handleDeviceChange);
    return () => mediaDevices.removeEventListener("devicechange", handleDeviceChange);
  }, [refreshAudioInputDevices]);

  useEffect(() => {
    if (!isRecording || recordingStatus !== "recording") {
      return undefined;
    }

    const updateDuration = () => {
      const startedAt = recordingStartedAtRef.current;
      if (!startedAt) return;
      setRecordingDurationSeconds(Math.max(0, Math.floor((Date.now() - startedAt) / 1000)));
    };

    updateDuration();
    const interval = window.setInterval(updateDuration, 1000);
    return () => window.clearInterval(interval);
  }, [isRecording, recordingStatus]);

  useEffect(() => () => {
    if (audioRecordingService.isRecording()) {
      void audioRecordingService.stopRecording();
    }
  }, []);

  useEffect(() => {
    showScrollbarTemporarily(1600);
  }, [activeView, showScrollbarTemporarily]);

  useEffect(() => () => {
    if (scrollbarTimerRef.current) {
      window.clearTimeout(scrollbarTimerRef.current);
    }
  }, []);

  const activeTitle = useMemo(() => navItems.find((item) => item.id === activeView)?.label ?? "Home", [activeView]);

  const handleWindowAction = (action: WindowAction) => {
    void window.asrpro?.windowControl(action);
  };

  const handleSetRecording = useCallback((active: boolean) => {
    if (active) {
      void startRecordingFlow(true);
    } else {
      void stopRecordingFlow(true);
    }
  }, [startRecordingFlow, stopRecordingFlow]);

  const handleScrollActivity = useCallback(() => {
    showScrollbarTemporarily(1100);
  }, [showScrollbarTemporarily]);

  const handleOverlayPlacementChange = useCallback((placement: OverlayPlacement) => {
    overlayPlacementTouchedRef.current = true;
    setOverlayPlacement(placement);
    setRuntimeInfo((current) => mergeOverlaySettings(current, { placement, customBounds: null }));

    window.asrpro?.setOverlaySettings?.({ placement }).then((settings) => {
      const nextPlacement = normalizeOverlayPlacement(settings.placement);
      setOverlayPlacement(nextPlacement);
      setRuntimeInfo((current) => mergeOverlaySettings(current, { ...settings, placement: nextPlacement }));
    }).catch(() => {});
  }, []);

  return (
    <div className="app-chrome h-screen w-screen overflow-hidden bg-[#2f2f2f] font-[Inter,-apple-system,BlinkMacSystemFont,'SF_Pro_Text','Segoe_UI',sans-serif] text-[#ededed] antialiased">
      <div className="grid h-full grid-cols-1 grid-rows-[auto_minmax(0,1fr)] sm:grid-cols-[208px_minmax(0,1fr)] sm:grid-rows-1">
        <Sidebar activeView={activeView} onChange={setActiveView} onWindowAction={handleWindowAction} />
        <section className="grid min-h-0 min-w-0 grid-rows-[34px_minmax(0,1fr)] bg-[radial-gradient(circle_at_68%_10%,rgba(57,89,62,0.16),transparent_38%),#333333] sm:border-l sm:border-[#3f3f3f]">
          <Toolbar
            activeTitle={activeTitle}
            audioInputDevices={audioInputDevices}
            selectedAudioInputId={selectedAudioInputId}
            selectedAudioInputLabel={selectedAudioInputLabel}
            audioInputDevicesLoading={audioInputDevicesLoading}
            onSelectAudioInput={handleAudioInputChange}
          />
          <main
            tabIndex={-1}
            className={`scrollbar-macos scrollbar-autohide min-h-0 min-w-0 overflow-y-auto px-3 pb-5 pt-3 outline-none focus:outline-none focus-visible:outline-none sm:px-4 ${isScrollbarVisible ? "is-scrollbar-visible" : ""}`}
            onScroll={handleScrollActivity}
            onTouchMove={handleScrollActivity}
            onWheel={handleScrollActivity}
          >
            {activeView === "home" && (
              <HomeView
                isRecording={isRecording}
                recordingStatus={recordingStatus}
                recordingError={recordingError}
                durationSeconds={recordingDurationSeconds}
                selectedModel={selectedModel}
                historyRows={historyRows}
                shortcut={runtimeInfo?.shortcut}
                onToggleRecording={() => handleSetRecording(!isRecording)}
                onOpenHistory={() => setActiveView("history")}
                onOpenModels={() => setActiveView("models")}
              />
            )}
            {activeView === "sound" && (
              <SoundView
                selectedModel={selectedModel}
                isRecording={isRecording}
                audioInputDevices={audioInputDevices}
                selectedAudioInputId={selectedAudioInputId}
                selectedAudioInputLabel={selectedAudioInputLabel}
                audioInputDevicesLoading={audioInputDevicesLoading}
                audioInputDevicesError={audioInputDevicesError}
                onSelectAudioInput={handleAudioInputChange}
                onRefreshAudioInputs={refreshAudioInputDevices}
                onOpenModels={() => setActiveView("models")}
              />
            )}
            {activeView === "models" && <ModelsView selectedModel={selectedModel} onSelectModel={setSelectedModel} />}
            {activeView === "history" && (
              <HistoryView
                rows={historyRows}
                onCopyRow={copyHistoryText}
                onReprocessRow={reprocessHistoryRow}
                onOpenTranscriptRow={openHistoryTranscriptText}
                onDeleteRow={deleteHistoryRow}
                reprocessingRowId={reprocessingHistoryRowId}
                openingTranscriptRowId={openingTranscriptRowId}
              />
            )}
            {activeView === "about" && <AboutView appInfo={appInfo} storagePath={runtimeInfo?.dataDir} />}
            {activeView === "configuration" && (
              <SettingsView
                runtimeInfo={runtimeInfo}
                selectedModel={selectedModel}
                selectedAudioInputLabel={selectedAudioInputLabel}
                overlayPlacement={overlayPlacement}
                textEditorOptions={textEditorOptions}
                selectedTextEditorId={selectedTextEditorId}
                selectedTextEditorLabel={selectedTextEditorLabel}
                onOverlayPlacementChange={handleOverlayPlacementChange}
                onTextEditorChange={handleTextEditorChange}
                onOpenModels={() => setActiveView("models")}
                onOpenSound={() => setActiveView("sound")}
              />
            )}
          </main>
        </section>
      </div>
    </div>
  );
}

function normalizeOverlayPlacement(value: unknown): OverlayPlacement {
  return value === "bottom" ? "bottom" : "top";
}

function mergeOverlaySettings(runtimeInfo: RuntimeInfo | null, overlaySettings: OverlaySettings): RuntimeInfo | null {
  if (!runtimeInfo) return runtimeInfo;
  return {
    ...runtimeInfo,
    overlaySettings,
  };
}

interface SidebarProps {
  activeView: ViewId;
  onChange: (view: ViewId) => void;
  onWindowAction: (action: WindowAction) => void;
}

function Sidebar({ activeView, onChange, onWindowAction }: SidebarProps) {
  return (
    <aside className="flex min-h-0 flex-col border-b border-[#545454] bg-[#3c3c3c] text-[#d8d8d8] sm:border-b-0">
      <div className="flex h-12 items-center gap-3 px-4 [-webkit-app-region:drag]">
        <WindowDots onWindowAction={onWindowAction} />
      </div>

      <nav className="scrollbar-macos flex gap-1 overflow-x-auto px-2.5 pb-3 pt-1 sm:block sm:min-h-0 sm:overflow-y-auto" aria-label="Primary">
        {navItems.map((item) => {
          const Icon = item.icon;
          const isActive = activeView === item.id;

          return (
            <button
              key={item.id}
              type="button"
              aria-label={item.label}
              aria-current={isActive ? "page" : undefined}
              className={`mb-1 flex h-9 shrink-0 items-center gap-2 rounded-[9px] px-2.5 text-left text-[13px] font-semibold transition outline-none focus:outline-none focus-visible:ring-2 focus-visible:ring-[#9bcfff]/70 focus-visible:ring-offset-1 focus-visible:ring-offset-[#3c3c3c] sm:w-full ${
                isActive
                  ? "bg-[#686868] text-white"
                  : "text-[#d0d0d0] hover:bg-[#505050]"
              }`}
              onClick={() => onChange(item.id)}
            >
              <span className={`grid size-5 shrink-0 place-items-center rounded-md ${sidebarIconTone[item.id]}`}>
                <Icon className="size-3.5" />
              </span>
              <span>{item.label}</span>
            </button>
          );
        })}
      </nav>
    </aside>
  );
}

interface WindowDotsProps {
  onWindowAction: (action: WindowAction) => void;
}

function WindowDots({ onWindowAction }: WindowDotsProps) {
  const dotButtonClass =
    "grid size-[13px] place-items-center rounded-full border-0 p-0 shadow-none outline-none transition-transform duration-150 [appearance:none] hover:scale-105 focus:outline-none focus-visible:outline-none focus-visible:ring-0 active:outline-none";
  const dotIconClass =
    "size-[9px] opacity-0 transition-opacity duration-100 group-hover/window-dots:opacity-75";

  return (
    <div className="group/window-dots flex shrink-0 items-center gap-[7px] [-webkit-app-region:no-drag]">
      <button
        aria-label="Close window"
        className={`${dotButtonClass} bg-[#ff5f57]`}
        type="button"
        onClick={() => onWindowAction("close")}
      >
        <X
          aria-hidden="true"
          data-window-dot-icon="close"
          strokeWidth={2.6}
          className={`${dotIconClass} text-[#6e140f]`}
        />
      </button>
      <button
        aria-label="Minimize window"
        className={`${dotButtonClass} bg-[#febc2e]`}
        type="button"
        onClick={() => onWindowAction("minimize")}
      >
        <Minus
          aria-hidden="true"
          data-window-dot-icon="minimize"
          strokeWidth={3}
          className={`${dotIconClass} text-[#8f5b00]`}
        />
      </button>
    </div>
  );
}

interface ToolbarProps {
  activeTitle: string;
  audioInputDevices: AudioInputDeviceOption[];
  selectedAudioInputId: string;
  selectedAudioInputLabel: string;
  audioInputDevicesLoading: boolean;
  onSelectAudioInput: (deviceId: string) => void;
}

function Toolbar({
  activeTitle,
  audioInputDevices,
  selectedAudioInputId,
  selectedAudioInputLabel,
  audioInputDevicesLoading,
  onSelectAudioInput,
}: ToolbarProps) {
  return (
    <header className="flex min-w-0 items-center justify-between border-b border-[#3c3c3c]/70 bg-transparent px-4 [-webkit-app-region:drag]">
      <div className="flex min-w-0 items-center">
        <span className="truncate text-[12px] font-semibold text-[#bdbdbd]">{activeTitle}</span>
      </div>
      <MicrophoneSelector
        ariaLabel="Toolbar microphone selector"
        devices={audioInputDevices}
        disabled={audioInputDevicesLoading}
        selectedDeviceId={selectedAudioInputId}
        selectedLabel={selectedAudioInputLabel}
        variant="toolbar"
        onSelect={onSelectAudioInput}
      />
    </header>
  );
}

interface HomeViewProps {
  isRecording: boolean;
  recordingStatus: RecordingStatus;
  recordingError: string | null;
  durationSeconds: number;
  selectedModel: string;
  historyRows: TranscriptHistoryRow[];
  shortcut?: string;
  onToggleRecording: () => void;
  onOpenHistory: () => void;
  onOpenModels: () => void;
}

function HomeView({
  isRecording,
  recordingStatus,
  recordingError,
  durationSeconds,
  selectedModel,
  historyRows,
  shortcut,
  onToggleRecording,
  onOpenHistory,
  onOpenModels,
}: HomeViewProps) {
  const isBusy = recordingStatus === "starting" || recordingStatus === "preparing-engine" || recordingStatus === "transcribing";
  const statusDetail = recordingStatus === "starting"
    ? "Opening microphone..."
    : recordingStatus === "preparing-engine"
      ? "Preparing Whisper engine..."
    : recordingStatus === "transcribing"
      ? "Loading Whisper model and transcribing..."
      : isRecording
        ? `Recording ${formatDuration(durationSeconds)}`
        : "Turn your voice to text with a single click.";
  const shortcutParts = formatShortcutParts(shortcut);
  const recordingTitle = isRecording ? "Stop recording" : recordingStatus === "preparing-engine" ? "Preparing engine" : recordingStatus === "transcribing" ? "Transcribing" : "Start recording";
  const recordingActionLabel = isRecording ? "Stop Recording" : recordingStatus === "preparing-engine" ? "Preparing Engine" : recordingStatus === "transcribing" ? "Transcribing" : "Start Recording";
  const stats = buildHomeStats(historyRows);

  return (
    <section className="mx-auto flex w-full max-w-[520px] flex-col gap-5 pt-1">
      <div className={`${panelSurfaceClass} grid grid-cols-2 sm:grid-cols-4`}>
        {stats.map((stat) => (
          <div key={stat.label} className={`border-t ${panelDividerClass} p-4 first:border-t-0 sm:border-l sm:border-t-0 sm:first:border-l-0`}>
            <p className="text-[15px] font-semibold leading-none text-[#f3f3f3]">{stat.value}</p>
            <p className="mt-2 text-[11px] font-semibold leading-none text-[#a4a4a4]">{stat.label}</p>
          </div>
        ))}
      </div>

      <span
        aria-label={isRecording ? "Recording active" : "Recording inactive"}
        className="sr-only"
      />

      <section>
        <h2 className="mb-3 text-[13px] font-semibold text-[#a9a9a9]">Get started</h2>
        <div className="space-y-2">
          <HomeActionRow
            icon={<Mic2 className="size-3.5" />}
            title={recordingTitle}
            detail={statusDetail}
            disabled={isBusy}
            trailing={<ShortcutCluster parts={shortcutParts} />}
            ariaLabel={recordingActionLabel}
            onClick={onToggleRecording}
          />
          <HomeActionRow icon={<History className="size-3.5" />} title="Review history" detail="Replay saved recordings and transcripts." onClick={onOpenHistory} />
          <HomeActionRow icon={<Library className="size-3.5" />} title="Choose speech model" detail={selectedModel} onClick={onOpenModels} />
        </div>
      </section>

      {recordingError ? (
        <div
          role="alert"
          className="selectable-text flex items-start gap-2 rounded-[12px] border border-[#ff7a66]/25 bg-[#ff6b4a]/10 px-3 py-2 text-left"
        >
          <span className="mt-0.5 grid size-5 shrink-0 place-items-center rounded-full bg-[#ff7a66]/15 text-[#ffad9f]">
            <Info className="size-3" aria-hidden="true" />
          </span>
          <span className="min-w-0">
            <span className="block text-[12px] font-semibold leading-4 text-[#ffd2ca]">
              {getRecordingErrorTitle(recordingError)}
            </span>
            <span className="block break-words text-[12px] font-medium leading-5 text-[#ffad9f]">
              {recordingError}
            </span>
          </span>
        </div>
      ) : null}

      <section>
        <div className="mb-2 flex items-center justify-between">
          <h2 className="text-[13px] font-semibold text-[#a9a9a9]">What's new?</h2>
          <button type="button" className={`inline-flex items-center gap-1.5 ${sharedRadiusClass} px-2 py-1 text-[12px] font-semibold text-[#ececec] transition hover:bg-white/[0.07] hover:text-white`} onClick={onOpenHistory}>
            <span>View history</span>
            <ArrowUpRight className="size-3" />
          </button>
        </div>
        <div className={panelSurfaceClass}>
          <UpdateRow date="May 14" title="Recording history playback" detail="Saved dictations keep playable source audio with their transcripts." />
          <UpdateRow date="May 14" title="Microphone picker" detail="Choose the input device from the toolbar or Sound settings." />
          <UpdateRow date="May 13" title="Native Whisper engine" detail="Desktop transcription now runs through local Whisper models in Electron." />
        </div>
      </section>
    </section>
  );
}

interface HomeActionRowProps {
  icon: ReactNode;
  title: string;
  detail?: string;
  trailing?: ReactNode;
  ariaLabel?: string;
  disabled?: boolean;
  onClick?: () => void;
}

function HomeActionRow({ icon, title, detail, trailing, ariaLabel, disabled = false, onClick }: HomeActionRowProps) {
  const content = (
    <>
      <div className="grid size-7 shrink-0 place-items-center text-[#a8a8a8]">{icon}</div>
      <div className="min-w-0 flex-1">
        <p className="truncate text-[14px] font-semibold leading-5 text-[#eeeeee]">{title}</p>
        {detail ? <p className="selectable-text truncate text-[13px] font-semibold leading-5 text-[#aaa]">{detail}</p> : null}
      </div>
      {trailing ? <div className="shrink-0">{trailing}</div> : null}
    </>
  );

  if (onClick) {
    return (
      <button
        type="button"
        aria-label={ariaLabel}
        className={`flex w-full items-center gap-3 ${sharedRadiusClass} px-2.5 py-2 text-left transition hover:bg-[#424242]/80 disabled:cursor-not-allowed disabled:opacity-70`}
        disabled={disabled}
        onClick={onClick}
      >
        {content}
      </button>
    );
  }

  return <div className={`flex w-full items-center gap-3 ${sharedRadiusClass} px-2.5 py-2`}>{content}</div>;
}

interface UpdateRowProps {
  date: string;
  title: string;
  detail: string;
}

function UpdateRow({ date, title, detail }: UpdateRowProps) {
  return (
    <div className={`grid grid-cols-[56px_minmax(0,1fr)] gap-3 border-t ${panelDividerClass} px-4 py-3 first:border-t-0`}>
      <p className="text-[12px] font-semibold text-[#8d8d8d]">{date}</p>
      <div className="min-w-0">
        <p className="truncate text-[13px] font-semibold text-[#eeeeee]">{title}</p>
        <p className="selectable-text mt-1 text-[12px] font-medium leading-5 text-[#b6b6b6]">{detail}</p>
      </div>
    </div>
  );
}

interface SoundViewProps {
  selectedModel: string;
  isRecording: boolean;
  audioInputDevices: AudioInputDeviceOption[];
  selectedAudioInputId: string;
  selectedAudioInputLabel: string;
  audioInputDevicesLoading: boolean;
  audioInputDevicesError: string | null;
  onSelectAudioInput: (deviceId: string) => void;
  onRefreshAudioInputs: () => void;
  onOpenModels: () => void;
}

function PanelControlButton({ className = "", children, ...props }: ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button {...props} className={`${panelControlButtonClass} ${className}`}>
      {children}
    </button>
  );
}

interface DropdownSurfaceProps {
  id: string;
  ariaLabel: string;
  alignClassName: string;
  children: ReactNode;
}

function DropdownSurface({ id, ariaLabel, alignClassName, children }: DropdownSurfaceProps) {
  return (
    <div id={id} role="listbox" aria-label={ariaLabel} className={`${alignClassName} ${dropdownSurfaceClass}`}>
      {children}
    </div>
  );
}

interface DropdownOptionButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  selected: boolean;
}

function DropdownOptionButton({ selected, className = "", children, ...props }: DropdownOptionButtonProps) {
  return (
    <button
      type="button"
      role="option"
      aria-selected={selected}
      {...props}
      className={`${dropdownOptionButtonClass} ${selected ? "bg-[#5a5a5a] text-white" : "text-[#dddddd] hover:bg-[#454545]"} ${className}`}
    >
      {children}
    </button>
  );
}

interface SegmentedControlOption<TValue extends string> {
  value: TValue;
  label: string;
  ariaLabel: string;
}

const overlayPlacementControlOptions: readonly SegmentedControlOption<OverlayPlacement>[] = [
  { value: "top", label: "Top", ariaLabel: "Top overlay position" },
  { value: "bottom", label: "Bottom", ariaLabel: "Bottom overlay position" },
];

interface SegmentedControlProps<TValue extends string> {
  value: TValue;
  options: readonly SegmentedControlOption<TValue>[];
  onChange: (value: TValue) => void;
}

function SegmentedControl<TValue extends string>({ value, options, onChange }: SegmentedControlProps<TValue>) {
  return (
    <div className={segmentedControlClass}>
      {options.map((option) => {
        const active = value === option.value;

        return (
          <button
            key={option.value}
            type="button"
            aria-label={option.ariaLabel}
            aria-pressed={active}
            className={`${segmentedItemClass} ${active ? "bg-[#686868] text-white" : "text-[#aaa] hover:text-white"}`}
            onClick={() => onChange(option.value)}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

interface MicrophoneSelectorProps {
  ariaLabel: string;
  devices: AudioInputDeviceOption[];
  disabled?: boolean;
  selectedDeviceId: string;
  selectedLabel: string;
  variant: "toolbar" | "panel";
  onSelect: (deviceId: string) => void;
}

function MicrophoneSelector({
  ariaLabel,
  devices,
  disabled = false,
  selectedDeviceId,
  selectedLabel,
  variant,
  onSelect,
}: MicrophoneSelectorProps) {
  const [isOpen, setIsOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const listboxId = useRef(`mic-options-${Math.random().toString(36).slice(2)}`);
  const isToolbar = variant === "toolbar";
  const selectedDevice = devices.find((device) => device.id === selectedDeviceId) ?? {
    id: selectedDeviceId,
    label: selectedLabel,
  };
  const triggerContent = (
    <>
      <AudioInputDeviceIcon device={selectedDevice} className={isToolbar ? "size-3 shrink-0 text-current" : "size-3 shrink-0 text-[#bdbdbd]"} />
      <span className={isToolbar ? "hidden min-w-0 truncate sm:inline" : "min-w-0 flex-1 whitespace-normal break-words text-left leading-4"}>
        {selectedLabel}
      </span>
    </>
  );

  useEffect(() => {
    if (!isOpen) return undefined;

    const handlePointerDown = (event: PointerEvent) => {
      if (rootRef.current?.contains(event.target as Node)) return;
      setIsOpen(false);
    };

    const handleKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        setIsOpen(false);
      }
    };

    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [isOpen]);

  const handleSelect = (deviceId: string) => {
    onSelect(deviceId);
    setIsOpen(false);
  };

  return (
    <div ref={rootRef} className={`relative min-w-0 ${isToolbar ? "[-webkit-app-region:no-drag]" : "w-full"}`}>
      {isToolbar ? (
        <button
          type="button"
          aria-controls={isOpen ? listboxId.current : undefined}
          aria-expanded={isOpen}
          aria-haspopup="listbox"
          aria-label={ariaLabel}
          disabled={disabled}
          className={`toolbar-mic-trigger inline-flex h-7 max-w-[260px] min-w-0 items-center gap-1.5 ${sharedRadiusClass} px-1.5 text-[12px] font-medium text-[#bdbdbd] disabled:cursor-not-allowed disabled:text-[#7d7d7d] ${focusRingClass}`}
          onClick={() => setIsOpen((current) => !current)}
        >
          {triggerContent}
        </button>
      ) : (
        <PanelControlButton
          type="button"
          aria-controls={isOpen ? listboxId.current : undefined}
          aria-expanded={isOpen}
          aria-haspopup="listbox"
          aria-label={ariaLabel}
          disabled={disabled}
          className="w-full min-w-0 justify-start px-2 py-1.5 text-left"
          onClick={() => setIsOpen((current) => !current)}
        >
          {triggerContent}
        </PanelControlButton>
      )}

      {isOpen ? (
        <DropdownSurface
          id={listboxId.current}
          ariaLabel="Microphone options"
          alignClassName={isToolbar ? "right-0 top-full mt-1 w-[320px] max-w-[calc(100vw-1rem)]" : "left-0 top-full mt-1 w-full min-w-[260px]"}
        >
          {devices.map((device) => {
            const selected = device.id === selectedDeviceId;

            return (
              <DropdownOptionButton
                key={device.id}
                selected={selected}
                onClick={() => handleSelect(device.id)}
              >
                <AudioInputDeviceIcon device={device} className="mt-0.5 size-3 shrink-0 text-[#bdbdbd]" />
                <span className="min-w-0 flex-1 whitespace-normal break-words">{device.label}</span>
                {selected ? <Check className="mt-0.5 size-3 shrink-0 text-[#9bcfff]" /> : null}
              </DropdownOptionButton>
            );
          })}
        </DropdownSurface>
      ) : null}
    </div>
  );
}

function SoundView({
  selectedModel,
  isRecording,
  audioInputDevices,
  selectedAudioInputId,
  selectedAudioInputLabel,
  audioInputDevicesLoading,
  audioInputDevicesError,
  onSelectAudioInput,
  onRefreshAudioInputs,
  onOpenModels,
}: SoundViewProps) {
  return (
    <ViewFrame title="Sound">
      <GroupedPanel title="Input" allowOverflow>
        <PanelRow
          icon={<Mic2 className="size-3.5" />}
          title="Microphone"
          detail={isRecording ? `Recording with ${selectedAudioInputLabel}` : selectedAudioInputLabel}
          trailing={<StatusLabel>{isRecording ? "Live" : selectedAudioInputId === defaultAudioInputId ? "Default" : "Ready"}</StatusLabel>}
          extra={(
            <div className="space-y-2">
              <div className="flex min-w-0 flex-col gap-2 sm:flex-row sm:items-center">
                <MicrophoneSelector
                  ariaLabel="Microphone selector"
                  devices={audioInputDevices}
                  disabled={isRecording || audioInputDevicesLoading}
                  selectedDeviceId={selectedAudioInputId}
                  selectedLabel={selectedAudioInputLabel}
                  variant="panel"
                  onSelect={onSelectAudioInput}
                />
                <PanelControlButton
                  type="button"
                  aria-label="Refresh microphones"
                  className="h-8 px-2.5 hover:bg-[#4a4a4a]"
                  disabled={audioInputDevicesLoading}
                  onClick={onRefreshAudioInputs}
                >
                  <RefreshCw className={`size-3 ${audioInputDevicesLoading ? "animate-spin" : ""}`} />
                  <span>Refresh</span>
                </PanelControlButton>
              </div>
              {audioInputDevicesError ? (
                <p role="status" className="selectable-text text-[12px] font-medium text-[#ffb3aa]">
                  {audioInputDevicesError}
                </p>
              ) : null}
            </div>
          )}
        />
        <PanelRow
          icon={<BrainCircuit className="size-3.5" />}
          title="Recognition model"
          detail={selectedModel}
          trailing={<NavigateButton label="Change" onClick={onOpenModels} />}
        />
      </GroupedPanel>
    </ViewFrame>
  );
}

interface ModelsViewProps {
  selectedModel: string;
  onSelectModel: (model: string) => void;
}

function ModelsView({ selectedModel, onSelectModel }: ModelsViewProps) {
  return (
    <ViewFrame title="Models library">
      <GroupedPanel title="Recognition models">
        {modelCards.map((model) => (
          <button
            key={model.name}
            type="button"
            disabled={model.disabled}
            aria-pressed={selectedModel === model.name}
            className={`flex min-h-14 w-full items-center gap-3 border-t ${panelDividerClass} px-3 py-2 text-left transition first:border-t-0 ${
              model.disabled ? "cursor-not-allowed opacity-55" : "hover:bg-white/[0.065]"
            } ${
              selectedModel === model.name ? "bg-white/[0.075]" : ""
            }`}
            onClick={() => {
              if (!model.disabled) {
                onSelectModel(model.name);
              }
            }}
          >
            <div className={iconTileClass}>
              <BrainCircuit className="size-3" />
            </div>
            <div className="min-w-0 flex-1">
              <p className="truncate text-[13px] font-semibold text-[#f2f2f2]">{model.name}</p>
              <p className="selectable-text mt-0.5 truncate text-[12px] font-medium text-[#aaa]">{model.detail}</p>
            </div>
            {model.disabled ? (
              <span className="shrink-0 text-[12px] font-semibold text-[#a8a8a8]">{model.status}</span>
            ) : selectedModel === model.name ? (
              <span className="inline-flex shrink-0 items-center gap-1.5 text-[12px] font-semibold text-[#e8e8e8]">
                <CheckCircle2 className="size-3.5 text-[#0a84ff]" />
                Selected
              </span>
            ) : (
              <span className="shrink-0 text-[12px] font-semibold text-[#a8a8a8]">Use model</span>
            )}
          </button>
        ))}
      </GroupedPanel>
    </ViewFrame>
  );
}

interface HistoryViewProps {
  rows: TranscriptHistoryRow[];
  onCopyRow: (text: string) => void;
  onReprocessRow: (row: TranscriptHistoryRow) => void;
  onOpenTranscriptRow: (row: TranscriptHistoryRow) => void;
  onDeleteRow: (rowId: string) => void;
  reprocessingRowId: string | null;
  openingTranscriptRowId: string | null;
}

function HistoryView({ rows, onCopyRow, onReprocessRow, onOpenTranscriptRow, onDeleteRow, reprocessingRowId, openingTranscriptRowId }: HistoryViewProps) {
  const [query, setQuery] = useState("");
  const [expandedRowId, setExpandedRowId] = useState<string | null>(rows[0]?.id ?? null);
  const normalizedQuery = query.trim().toLowerCase();
  const filteredRows = rows.filter((row) => (
    !normalizedQuery
    || row.title.toLowerCase().includes(normalizedQuery)
    || row.text.toLowerCase().includes(normalizedQuery)
    || row.model.toLowerCase().includes(normalizedQuery)
  ));
  const groupedRows = filteredRows.reduce<Array<{ label: string; rows: TranscriptHistoryRow[] }>>((groups, row) => {
    const label = formatHistoryGroupLabel(row.createdAt);
    const group = groups.find((item) => item.label === label);
    if (group) {
      group.rows.push(row);
    } else {
      groups.push({ label, rows: [row] });
    }
    return groups;
  }, []);

  useEffect(() => {
    setExpandedRowId((current) => {
      if (current && rows.some((row) => row.id === current)) return current;
      return rows[0]?.id ?? null;
    });
  }, [rows]);

  return (
    <section className="mx-auto w-full max-w-[520px] space-y-4">
      <h2 className="sr-only">History</h2>
      <div className="flex h-10 items-center gap-2 rounded-full border border-white/[0.09] bg-white/[0.035] px-3 text-[#8e8e8e] backdrop-blur-2xl">
        <Search className="size-3.5 shrink-0" />
        <input
          type="search"
          aria-label="Search history"
          className="liquid-search-input min-w-0 flex-1 appearance-none border-0 bg-transparent p-0 text-[13px] font-semibold text-[#e8e8e8] outline-none placeholder:text-[#777]"
          placeholder="Find..."
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
        <ShortcutCluster parts={["⌘", "F"]} muted />
      </div>

      {groupedRows.length ? (
        <div className="space-y-5">
          {groupedRows.map((group) => (
            <section key={group.label} className="space-y-2">
              <h3 className="px-2 text-[12px] font-semibold text-[#858585]">{group.label}</h3>
              <div className="space-y-2">
                {group.rows.map((row) => (
                  <HistoryCard
                    key={row.id}
                    row={row}
                    expanded={expandedRowId === row.id}
                    onCopy={() => onCopyRow(row.text)}
                    onReprocess={() => onReprocessRow(row)}
                    onOpenTranscript={() => onOpenTranscriptRow(row)}
                    onDelete={() => onDeleteRow(row.id)}
                    onToggle={() => setExpandedRowId((current) => (current === row.id ? null : row.id))}
                    reprocessing={reprocessingRowId === row.id}
                    openingTranscript={openingTranscriptRowId === row.id}
                  />
                ))}
              </div>
            </section>
          ))}
        </div>
      ) : (
        <div className={`${panelSurfaceClass} p-6 text-center`}>
          <p className="text-[14px] font-semibold text-[#eeeeee]">No transcription history yet</p>
          <p className="selectable-text mt-1 text-[12px] leading-5 text-[#b4b4b4]">Stop a recording after dictation and the transcript will appear here.</p>
        </div>
      )}
    </section>
  );
}

interface HistoryCardProps {
  row: TranscriptHistoryRow;
  expanded: boolean;
  onCopy: () => void;
  onReprocess: () => void;
  onOpenTranscript: () => void;
  onDelete: () => void;
  onToggle: () => void;
  reprocessing: boolean;
  openingTranscript: boolean;
}

function HistoryCard({ row, expanded, onCopy, onReprocess, onOpenTranscript, onDelete, onToggle, reprocessing, openingTranscript }: HistoryCardProps) {
  const canReprocess = Boolean(row.recordingUrl);
  const canOpenTranscript = Boolean(row.text.trim());

  return (
    <article className={`${panelSurfaceClass} p-4 transition-colors duration-200 ${expanded ? "bg-white/[0.07]" : ""}`}>
      <button
        type="button"
        className="block w-full text-left"
        onClick={onToggle}
      >
        <p className={`selectable-text ${expanded ? "line-clamp-3" : "truncate"} text-[13px] font-semibold leading-5 text-[#f1f1f1]`}>
          {row.title}
        </p>
        {expanded && row.status !== "failed" && row.text && row.text !== row.title ? (
          <p className="selectable-text mt-2 text-[12px] font-medium leading-5 text-[#c7c7c7]">{row.text}</p>
        ) : null}
      </button>

      {expanded ? (
          <div className="history-card-details mt-3 space-y-3">
          {row.recordingUrl ? (
            <HistoryRecordingPlayer title={row.title} src={row.recordingUrl} />
          ) : (
            <MissingHistoryAudioNotice />
          )}
          {row.status === "failed" ? (
            <p className={`selectable-text ${sharedRadiusClass} border border-white/[0.08] bg-white/[0.05] px-3 py-2 text-[12px] font-medium text-[#ffb3aa]`}>
              {row.error ?? "Transcription failed"}
            </p>
          ) : null}
          <div className="flex items-center justify-between">
            <div className={`inline-flex ${sharedRadiusClass} bg-white/[0.07] p-0.5`}>
              <span className="rounded-[7px] bg-[#777] px-2 py-1 text-[12px] font-semibold text-white">Original</span>
              <span className="px-2 py-1 text-[12px] font-semibold text-[#9d9d9d]">Segmented</span>
            </div>
            <div className="flex items-center gap-1 text-[#bdbdbd]">
              {canReprocess ? (
                <button
                  type="button"
                  aria-busy={reprocessing || undefined}
                  aria-label={`Reprocess clip: ${row.title}`}
                  className={historyActionButtonClass}
                  disabled={reprocessing}
                  onClick={onReprocess}
                >
                  <RefreshCw className={`size-3 ${reprocessing ? "animate-spin" : ""}`} />
                </button>
              ) : null}
              {canOpenTranscript ? (
                <button
                  type="button"
                  aria-busy={openingTranscript || undefined}
                  aria-label={`Open transcript text: ${row.title}`}
                  className={historyActionButtonClass}
                  disabled={openingTranscript}
                  onClick={onOpenTranscript}
                >
                  <FileText className="size-3" />
                </button>
              ) : null}
              <button type="button" aria-label={`Copy transcript: ${row.title}`} className={historyActionButtonClass} onClick={onCopy}>
                <Copy className="size-3" />
              </button>
              <button type="button" aria-label={`Delete transcript: ${row.title}`} className={historyActionButtonClass} onClick={onDelete}>
                <Trash2 className="size-3" />
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </article>
  );
}

function MissingHistoryAudioNotice() {
  return (
    <div className={`flex items-center gap-2 ${sharedRadiusClass} border border-white/[0.08] bg-white/[0.05] px-3 py-2 text-[#bdbdbd]`}>
      <VolumeX className="size-3 shrink-0" aria-hidden="true" />
      <span className="text-[12px] font-semibold">No source audio saved</span>
    </div>
  );
}

interface HistoryRecordingPlayerProps {
  title: string;
  src: string;
}

function HistoryRecordingPlayer({ title, src }: HistoryRecordingPlayerProps) {
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const [isPlaying, setIsPlaying] = useState(false);

  const togglePlayback = async () => {
    const audio = audioRef.current;
    if (!audio) return;

    if (isPlaying) {
      audio.pause();
      setIsPlaying(false);
      return;
    }

    try {
      await audio.play();
      setIsPlaying(true);
    } catch {
      setIsPlaying(false);
    }
  };

  return (
    <div className={`flex items-center gap-3 ${sharedRadiusClass} border border-white/[0.08] bg-white/[0.085] px-3 py-2`}>
      <button
        type="button"
        aria-label={`${isPlaying ? "Pause" : "Play"} recording: ${title}`}
        className={`grid size-7 shrink-0 place-items-center ${sharedRadiusClass} text-white transition active:scale-[0.97] ${isPlaying ? "bg-[#0a84ff]" : "bg-white/[0.12] hover:bg-white/[0.18]"}`}
        onClick={togglePlayback}
      >
        {isPlaying ? <Pause className="size-3" /> : <Play className="size-3" />}
      </button>
      <div className="flex min-w-0 flex-1 items-center gap-px overflow-hidden" aria-hidden="true">
        {historyWaveformBars.map((bar) => (
          <span
            key={bar.id}
            className="w-px shrink-0 rounded-full bg-[#a9a9a9]"
            style={{ height: `${bar.height}px`, opacity: bar.index % 4 === 0 ? 0.35 : 0.62 }}
          />
        ))}
      </div>
      <audio
        ref={audioRef}
        aria-label={`Recording audio: ${title}`}
        preload="metadata"
        src={src}
        onEnded={() => setIsPlaying(false)}
        onPause={() => setIsPlaying(false)}
        onPlay={() => setIsPlaying(true)}
      />
    </div>
  );
}

interface SettingsViewProps {
  runtimeInfo: RuntimeInfo | null;
  selectedModel: string;
  selectedAudioInputLabel: string;
  overlayPlacement: OverlayPlacement;
  textEditorOptions: TextEditorOption[];
  selectedTextEditorId: string;
  selectedTextEditorLabel: string;
  onOverlayPlacementChange: (placement: OverlayPlacement) => void;
  onTextEditorChange: (editorId: string) => void;
  onOpenModels: () => void;
  onOpenSound: () => void;
}

function SettingsView({
  runtimeInfo,
  selectedModel,
  selectedAudioInputLabel,
  overlayPlacement,
  textEditorOptions,
  selectedTextEditorId,
  selectedTextEditorLabel,
  onOverlayPlacementChange,
  onTextEditorChange,
  onOpenModels,
  onOpenSound,
}: SettingsViewProps) {
  const shortcutParts = formatShortcutParts(runtimeInfo?.shortcut);
  const engine = runtimeInfo?.engine;
  const engineStatus = formatEngineStatus(engine?.status);
  const engineDetail = engine?.error || engine?.detail || (engine?.status === "idle" ? "Loads the selected Whisper model when needed" : engine?.model || engine?.mode || "Waiting for desktop runtime");

  return (
    <ViewFrame title="Configuration">
      <GroupedPanel title="Recording overlay">
        <PanelRow
          title="Position"
          detail="Floating waveform location"
          trailing={<OverlayPlacementControl placement={overlayPlacement} onChange={onOverlayPlacementChange} />}
        />
      </GroupedPanel>

      <GroupedPanel title="Keyboard shortcuts">
        <PanelRow title="Toggle recording" detail="Registered by the desktop app" trailing={<ShortcutCluster parts={shortcutParts} />} />
      </GroupedPanel>

      <GroupedPanel title="Application" allowOverflow>
        <PanelRow title="Default model" detail={runtimeInfo?.defaultModel ?? selectedModel} trailing={<NavigateButton label="Change" onClick={onOpenModels} />} />
        <PanelRow title="Microphone input" detail={selectedAudioInputLabel} trailing={<NavigateButton label="Change" onClick={onOpenSound} />} />
        <PanelRow
          title="Transcript editor"
          detail="Used for history text files"
          trailing={(
            <TextEditorSelector
              options={textEditorOptions}
              selectedEditorId={selectedTextEditorId}
              selectedLabel={selectedTextEditorLabel}
              onSelect={onTextEditorChange}
            />
          )}
        />
        <PanelRow title="Engine" detail={engineDetail} trailing={<StatusLabel>{engineStatus}</StatusLabel>} />
        <PanelRow title="Data folder" detail={runtimeInfo?.dataDir ?? "App-contained data directory"} trailing={<StatusLabel>Read only</StatusLabel>} />
      </GroupedPanel>
    </ViewFrame>
  );
}

function formatEngineStatus(status?: string) {
  if (status === "ready") return "Ready";
  if (status === "starting") return "Starting";
  if (status === "downloading") return "Downloading";
  if (status === "transcribing") return "Transcribing";
  if (status === "idle") return "Idle";
  if (status === "failed") return "Failed";
  if (status === "stopped") return "Stopped";
  return "Unknown";
}

interface AboutViewProps {
  appInfo: AppInfo;
  storagePath?: string;
}

function AboutView({ appInfo, storagePath }: AboutViewProps) {
  const facts = buildAboutFactRows(appInfo.version, storagePath);

  return (
    <ViewFrame title="About ASR Pro">
      <section aria-label="About product summary" className={panelSurfaceClass}>
        <div className="flex flex-col gap-4 p-5 sm:flex-row sm:items-start">
          <div className="grid size-[72px] shrink-0 place-items-center rounded-[16px] bg-[#f6f4ef] text-[#26343b]">
            <AppLogoMark className="size-16" title="ASR Pro" />
          </div>
          <div className="min-w-0">
            <h3 className="text-[24px] font-semibold leading-7 tracking-normal text-[#f4f4f4]">{appInfo.name}</h3>
            <p className="selectable-text mt-1 text-[12px] font-semibold text-[#a8a8a8]">Version {appInfo.version}</p>
            <p className="selectable-text mt-4 max-w-[420px] text-[13px] leading-5 text-[#cfcfcf]">
              A quiet desktop workspace for private dictation, file transcription, and local speech model testing.
            </p>
          </div>
        </div>

        <dl aria-label="Product facts" className={`border-t ${panelDividerClass}`}>
          {facts.map((fact) => (
            <div key={fact.label} className={`grid gap-1 border-t ${panelDividerClass} px-5 py-3 first:border-t-0 sm:grid-cols-[120px_minmax(0,1fr)] sm:gap-4`}>
              <dt className="text-[11px] font-semibold uppercase leading-5 text-[#8e8e8e]">{fact.label}</dt>
              <dd className="selectable-text text-[13px] font-semibold leading-5 text-[#e4e4e4]">{fact.value}</dd>
            </div>
          ))}
        </dl>

        <div aria-label="GitHub links" className={`grid border-t ${panelDividerClass} sm:grid-cols-2`}>
          {aboutActionLinks.map((link) => {
            const Icon = link.icon;

            return (
              <a
                key={link.label}
                href={link.href}
                target="_blank"
                rel="noreferrer"
                className={`group/link flex min-w-0 items-center gap-3 border-t ${panelDividerClass} px-5 py-3 text-left no-underline transition-colors first:border-t-0 hover:bg-white/[0.045] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#9bcfff]/70 sm:border-l sm:border-t-0 sm:first:border-l-0`}
                aria-label={`${link.label}: ${link.detail}`}
              >
                <span className={iconTileClass}>
                  <Icon className="size-3.5" />
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block text-[13px] font-semibold leading-5 text-[#eeeeee]">{link.label}</span>
                  <span className="block truncate text-[12px] font-medium leading-5 text-[#aaa]">{link.detail}</span>
                </span>
                <ArrowUpRight className="size-3.5 shrink-0 text-[#9f9f9f] transition-colors group-hover/link:text-[#eeeeee]" />
              </a>
            );
          })}
        </div>
      </section>
    </ViewFrame>
  );
}

interface OverlayPlacementControlProps {
  placement: OverlayPlacement;
  onChange: (placement: OverlayPlacement) => void;
}

function OverlayPlacementControl({ placement, onChange }: OverlayPlacementControlProps) {
  return (
    <SegmentedControl value={placement} options={overlayPlacementControlOptions} onChange={onChange} />
  );
}

interface TextEditorSelectorProps {
  options: TextEditorOption[];
  selectedEditorId: string;
  selectedLabel: string;
  onSelect: (editorId: string) => void;
}

function TextEditorSelector({ options, selectedEditorId, selectedLabel, onSelect }: TextEditorSelectorProps) {
  const [isOpen, setIsOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const listboxId = useRef(`text-editor-options-${Math.random().toString(36).slice(2)}`);

  useEffect(() => {
    if (!isOpen) return undefined;

    const handlePointerDown = (event: PointerEvent) => {
      if (rootRef.current?.contains(event.target as Node)) return;
      setIsOpen(false);
    };

    const handleKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        setIsOpen(false);
      }
    };

    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [isOpen]);

  const handleSelect = (editorId: string) => {
    onSelect(editorId);
    setIsOpen(false);
  };

  return (
    <div ref={rootRef} className="relative min-w-[180px]">
      <PanelControlButton
        type="button"
        aria-controls={isOpen ? listboxId.current : undefined}
        aria-expanded={isOpen}
        aria-haspopup="listbox"
        aria-label="Text editor selector"
        className="w-full min-w-0 justify-start px-2 py-1.5 text-left"
        onClick={() => setIsOpen((current) => !current)}
      >
        <FileText className="size-3 shrink-0 text-[#bdbdbd]" />
        <span className="min-w-0 flex-1 truncate">{selectedLabel}</span>
      </PanelControlButton>

      {isOpen ? (
        <DropdownSurface
          id={listboxId.current}
          ariaLabel="Text editor options"
          alignClassName="right-0 top-full mt-1 w-[260px] max-w-[calc(100vw-1rem)]"
        >
          {options.map((editor) => {
            const selected = editor.id === selectedEditorId;

            return (
              <DropdownOptionButton
                key={editor.id}
                selected={selected}
                aria-label={`${editor.label}${editor.detail ? `, ${editor.detail}` : ""}`}
                onClick={() => handleSelect(editor.id)}
              >
                <FileText className="mt-0.5 size-3 shrink-0 text-[#bdbdbd]" />
                <span className="min-w-0 flex-1">
                  <span className="block whitespace-normal break-words">{editor.label}</span>
                  {editor.detail ? <span className="block whitespace-normal break-words text-[11px] font-medium text-[#9f9f9f]">{editor.detail}</span> : null}
                </span>
                {selected ? <Check className="mt-0.5 size-3 shrink-0 text-[#9bcfff]" /> : null}
              </DropdownOptionButton>
            );
          })}
        </DropdownSurface>
      ) : null}
    </div>
  );
}

interface ViewFrameProps {
  title: string;
  children: ReactNode;
}

function ViewFrame({ title, children }: ViewFrameProps) {
  return (
    <section className="mx-auto w-full max-w-[520px] space-y-4">
      <h2 className="sr-only">{title}</h2>
      {children}
    </section>
  );
}

interface GroupedPanelProps {
  title?: string;
  allowOverflow?: boolean;
  children: ReactNode;
}

function GroupedPanel({ title, allowOverflow = false, children }: GroupedPanelProps) {
  return (
    <section className="space-y-2">
      {title ? <h3 className="px-1 text-[13px] font-semibold text-[#a8a8a8]">{title}</h3> : null}
      <div className={allowOverflow ? panelGlassClass : panelSurfaceClass}>{children}</div>
    </section>
  );
}

interface PanelRowProps {
  icon?: ReactNode;
  title: string;
  detail?: string;
  trailing?: ReactNode;
  extra?: ReactNode;
}

function PanelRow({ icon, title, detail, trailing, extra }: PanelRowProps) {
  return (
    <div className={`border-t ${panelDividerClass} p-4 first:border-t-0`}>
      <div className="flex min-w-0 items-center gap-3">
        {icon ? <div className={iconTileClass}>{icon}</div> : null}
        <div className="min-w-0 flex-1">
          <p className="truncate text-[13px] font-semibold text-[#eeeeee]">{title}</p>
          {detail ? <p className="selectable-text mt-0.5 truncate text-[12px] font-medium text-[#aaa]">{detail}</p> : null}
        </div>
        {trailing ? <div className="shrink-0">{trailing}</div> : null}
      </div>
      {extra ? <div className={icon ? "mt-3 pl-11" : "mt-3"}>{extra}</div> : null}
    </div>
  );
}

interface StatusLabelProps {
  children: ReactNode;
}

interface NavigateButtonProps {
  label: string;
  onClick: () => void;
}

function NavigateButton({ label, onClick }: NavigateButtonProps) {
  return (
    <button
      type="button"
      className={`inline-flex h-7 items-center gap-1.5 ${sharedRadiusClass} bg-white/[0.08] px-2.5 text-[12px] font-semibold text-[#eeeeee] transition hover:bg-white/[0.12] active:scale-[0.97]`}
      onClick={onClick}
    >
      <span>{label}</span>
      <ArrowUpRight className="size-3" />
    </button>
  );
}

function StatusLabel({ children }: StatusLabelProps) {
  return <span className="text-[12px] font-semibold text-[#cfcfcf]">{children}</span>;
}

interface ShortcutClusterProps {
  parts: string[];
  muted?: boolean;
}

function ShortcutCluster({ parts, muted = false }: ShortcutClusterProps) {
  return (
    <span className="inline-flex shrink-0 items-center gap-1">
      {parts.map((part) => (
        <ShortcutBadge key={part} muted={muted}>{part}</ShortcutBadge>
      ))}
    </span>
  );
}

interface ShortcutBadgeProps {
  children: ReactNode;
  muted?: boolean;
}

function ShortcutBadge({ children, muted = false }: ShortcutBadgeProps) {
  return <span className={`grid min-w-5 place-items-center rounded-[5px] px-1.5 py-1 text-[11px] font-semibold leading-none ${muted ? "bg-[#3a3a3a] text-[#858585]" : "bg-[#646464] text-[#f2f2f2]"}`}>{children}</span>;
}

export default App;
