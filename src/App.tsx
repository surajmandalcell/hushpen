import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  BrainCircuit,
  Bluetooth,
  Camera,
  Check,
  CheckCircle2,
  ChevronDown,
  Copy,
  Headphones,
  History,
  Home,
  Library,
  Laptop,
  Mic2,
  Pause,
  Play,
  RefreshCw,
  Search,
  Settings,
  Smartphone,
  Trash2,
  Usb,
  Volume2,
  type LucideIcon,
} from "lucide-react";
import { apiClient } from "./services/api";
import { audioRecordingService } from "./services/audioRecording";

type ViewId = "home" | "configuration" | "sound" | "models" | "history";
type WindowAction = "minimize" | "maximize" | "close";
type OverlayPlacement = "top" | "bottom";
type RecordingStatus = "idle" | "starting" | "recording" | "transcribing" | "error";

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
  defaultModelRepo?: string;
  dataDir?: string;
  overlaySettings?: OverlaySettings;
  shortcut?: string;
  shortcutRegistered?: boolean;
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
];

const sidebarIconTone: Record<ViewId, string> = {
  home: "bg-[#ff7a32] text-white",
  configuration: "bg-[#727272] text-white",
  sound: "bg-[#737373] text-white",
  models: "bg-[#8f8f8f] text-white",
  history: "bg-[#7167ff] text-white",
};

const defaultModelName = "Local Whisper";
const parakeetModelName = "Parakeet-TDT-0.6B-v3";
const defaultAudioInputId = "default";
const defaultAudioInputLabel = "System default";
const defaultAudioInputOptions: AudioInputDeviceOption[] = [{ id: defaultAudioInputId, label: defaultAudioInputLabel }];
const modelIdsByName: Record<string, string> = {
  [defaultModelName]: "whisper-base",
  [parakeetModelName]: "parakeet-tdt-0.6b-v3",
  "Whisper Large V3 Turbo": "whisper-large",
};
const transcriptHistoryStorageKey = "asrpro.transcriptHistory.v1";
const audioInputDeviceStorageKey = "asrpro.audioInputDevice.v1";
const historyDateFormatter = new Intl.DateTimeFormat(undefined, {
  month: "short",
  day: "numeric",
});

const modelCards = [
  {
    name: "Local Whisper",
    detail: "Whisper Base, offline, private",
    speed: "Default",
    status: "Active",
  },
  {
    name: parakeetModelName,
    detail: "NVIDIA NeMo, multilingual, punctuation and timestamps",
    speed: "Optional",
    status: "Requires NeMo",
  },
  {
    name: "Whisper Large V3 Turbo",
    detail: "Higher accuracy for noisy media",
    speed: "Optional",
    status: "Not installed",
  },
];

const historyWaveformBars = Array.from({ length: 72 }, (_, index) => {
  const position = index / 71;
  const envelope = 0.42 + 0.58 * Math.sin(Math.PI * position);
  const shape = 0.42 + 0.2 * Math.sin(index * 1.7) + 0.16 * Math.sin(index * 0.53 + 1.1);
  return Math.round(clampNumber(6 + 18 * envelope * shape, 5, 24));
});

const sharedRadiusClass = "rounded-[9px]";
const panelSurfaceClass = "overflow-hidden rounded-[18px] border border-[#505050] bg-[linear-gradient(135deg,rgba(70,70,70,0.86),rgba(54,54,54,0.9))] shadow-[inset_0_1px_0_rgba(255,255,255,0.045)]";
const panelDividerClass = "border-[#505050]";
const iconTileClass = `grid size-8 shrink-0 place-items-center ${sharedRadiusClass} bg-[#303030] text-[#d7d7d7]`;

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

    return rows;
  } catch {
    return [];
  }
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

function startOfDay(timestamp: number) {
  const date = new Date(timestamp);
  date.setHours(0, 0, 0, 0);
  return date.getTime();
}

function formatShortcutParts(shortcut?: string) {
  const normalized = (shortcut || "CommandOrControl+`").split("+").map((part) => part.trim()).filter(Boolean);
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
  return error instanceof Error ? error.message : "Recording failed";
}

function createRecordingFile(blob: Blob) {
  const extension = blob.type.includes("mp4") ? "m4a" : blob.type.includes("mpeg") ? "mp3" : blob.type.includes("wav") ? "wav" : "webm";
  return new File([blob], `dictation-${new Date().toISOString().replace(/[:.]/g, "-")}.${extension}`, {
    type: blob.type || "audio/webm",
  });
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
  const [overlayPlacement, setOverlayPlacement] = useState<OverlayPlacement>("top");
  const [historyRows, setHistoryRows] = useState<TranscriptHistoryRow[]>(loadTranscriptHistory);
  const [isScrollbarVisible, setIsScrollbarVisible] = useState(true);
  const recordingStartedAtRef = useRef<number | null>(null);
  const recordingTransitionRef = useRef<"starting" | "stopping" | null>(null);
  const scrollbarTimerRef = useRef<number | null>(null);
  useMicrophoneWaveform(isRecording);

  const addHistoryRow = useCallback((row: TranscriptHistoryRow) => {
    setHistoryRows((current) => {
      const next = [row, ...current].slice(0, 100);
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

  const selectedAudioInputLabel = useMemo(() => (
    audioInputDevices.find((device) => device.id === selectedAudioInputId)?.label ?? defaultAudioInputLabel
  ), [audioInputDevices, selectedAudioInputId]);

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
    setRecordingStatus(wasRecording ? "transcribing" : "idle");
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
      const result = await apiClient.transcribeFile(createRecordingFile(audioBlob), modelIdsByName[selectedModel] ?? selectedModel);
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
  }, [addHistoryRow, selectedModel, syncRecordingBridge]);

  useEffect(() => {
    const api = window.asrpro;
    if (!api) return undefined;

    if (api.getRuntimeState) {
      Promise.resolve(api.getRuntimeState()).then((state) => {
        if (!state) return;
        setRuntimeInfo(state);
        setSelectedModel(state.defaultModel || defaultModelName);
        setOverlayPlacement(normalizeOverlayPlacement(state.overlaySettings?.placement));
        if (state.isRecording) {
          void startRecordingFlow(false);
        } else {
          setIsRecording(false);
        }
      }).catch(() => {});
    }

    return api.onRecordingState?.((state) => {
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
    setOverlayPlacement(placement);
    setRuntimeInfo((current) => mergeOverlaySettings(current, { placement, customBounds: null }));

    window.asrpro?.setOverlaySettings?.({ placement }).then((settings) => {
      const nextPlacement = normalizeOverlayPlacement(settings.placement);
      setOverlayPlacement(nextPlacement);
      setRuntimeInfo((current) => mergeOverlaySettings(current, { ...settings, placement: nextPlacement }));
    }).catch(() => {});
  }, []);

  return (
    <div className="h-screen w-screen overflow-hidden bg-[#2f2f2f] font-[Inter,-apple-system,BlinkMacSystemFont,'SF_Pro_Text','Segoe_UI',sans-serif] text-[#ededed] antialiased">
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
            className={`scrollbar-macos min-h-0 min-w-0 overflow-y-auto px-3 pb-5 pt-3 sm:px-4 ${isScrollbarVisible ? "is-scrollbar-visible" : ""}`}
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
              />
            )}
            {activeView === "models" && <ModelsView selectedModel={selectedModel} onSelectModel={setSelectedModel} />}
            {activeView === "history" && (
              <HistoryView
                rows={historyRows}
                onCopyRow={copyHistoryText}
                onDeleteRow={deleteHistoryRow}
              />
            )}
            {activeView === "configuration" && (
              <SettingsView
                runtimeInfo={runtimeInfo}
                selectedModel={selectedModel}
                overlayPlacement={overlayPlacement}
                onOverlayPlacementChange={handleOverlayPlacementChange}
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

      <nav className="flex gap-1 overflow-x-auto px-2.5 pb-3 pt-1 sm:block sm:min-h-0 sm:overflow-y-auto" aria-label="Primary">
        {navItems.map((item) => {
          const Icon = item.icon;
          const isActive = activeView === item.id;

          return (
            <button
              key={item.id}
              type="button"
              aria-label={item.label}
              aria-current={isActive ? "page" : undefined}
              className={`mb-1 flex h-9 shrink-0 items-center gap-2 rounded-[9px] px-2.5 text-left text-[13px] font-semibold transition sm:w-full ${
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
  return (
    <div className="flex shrink-0 items-center gap-2 [-webkit-app-region:no-drag]">
      <button
        aria-label="Close window"
        className="size-3 rounded-full border border-[#e14640] bg-[#ff5f57]"
        type="button"
        onClick={() => onWindowAction("close")}
      />
      <button
        aria-label="Minimize window"
        className="size-3 rounded-full border border-[#dfa023] bg-[#febc2e]"
        type="button"
        onClick={() => onWindowAction("minimize")}
      />
      <button
        aria-label="Maximize window"
        className="size-3 rounded-full border border-[#18a433] bg-[#28c840]"
        type="button"
        onClick={() => onWindowAction("maximize")}
      />
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
  shortcut,
  onToggleRecording,
  onOpenHistory,
  onOpenModels,
}: HomeViewProps) {
  const isBusy = recordingStatus === "starting" || recordingStatus === "transcribing";
  const statusDetail = recordingStatus === "starting"
    ? "Opening microphone..."
    : recordingStatus === "transcribing"
      ? "Transcribing captured audio..."
      : isRecording
        ? `Recording ${formatDuration(durationSeconds)}`
        : "Turn your voice to text with a single click.";
  const shortcutParts = formatShortcutParts(shortcut);
  const recordingActionLabel = isRecording ? "Stop Recording" : recordingStatus === "transcribing" ? "Transcribing" : "Start Recording";

  return (
    <section className="mx-auto flex w-full max-w-[520px] flex-col gap-4 pt-1">
      <span
        aria-label={isRecording ? "Recording active" : "Recording inactive"}
        className="sr-only"
      />

      <section>
        <h2 className="mb-3 text-[13px] font-semibold text-[#a9a9a9]">Get started</h2>
        <div className="space-y-2">
          <HomeActionRow
            icon={<Mic2 className="size-3.5" />}
            title={isRecording ? "Stop recording" : "Start recording"}
            detail={statusDetail}
            disabled={isBusy}
            trailing={<ShortcutCluster parts={shortcutParts} />}
            ariaLabel={recordingActionLabel}
            onClick={onToggleRecording}
          />
          <HomeActionRow icon={<History className="size-3.5" />} title="Review history" detail="Replay saved recordings and transcripts." onClick={onOpenHistory} />
          <HomeActionRow icon={<Library className="size-3.5" />} title="Choose speech model" detail="Pick the recognizer for new dictations." onClick={onOpenModels} />
        </div>
      </section>

      {recordingError ? (
        <p role="alert" className="px-1 text-[12px] font-medium text-[#ff9c8f]">
          {recordingError}
        </p>
      ) : null}

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
        {detail ? <p className="truncate text-[13px] font-semibold leading-5 text-[#aaa]">{detail}</p> : null}
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
      <button
        type="button"
        aria-controls={isOpen ? listboxId.current : undefined}
        aria-expanded={isOpen}
        aria-haspopup="listbox"
        aria-label={ariaLabel}
        disabled={disabled}
        className={isToolbar
          ? "inline-flex h-7 max-w-[260px] min-w-0 items-center gap-1.5 rounded-[7px] px-1.5 text-[12px] font-medium text-[#bdbdbd] transition hover:bg-[#434343] hover:text-[#eeeeee] disabled:cursor-not-allowed disabled:text-[#7d7d7d] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#9bcfff]"
          : "flex min-h-8 w-full min-w-0 items-center gap-2 rounded-md border border-[#5c5c5c] bg-[#303030] px-2 py-1.5 text-[12px] font-semibold text-[#eeeeee] transition hover:bg-[#3a3a3a] disabled:cursor-not-allowed disabled:text-[#8a8a8a] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#9bcfff]"}
        onClick={() => setIsOpen((current) => !current)}
      >
        <AudioInputDeviceIcon device={selectedDevice} className={isToolbar ? "size-3 shrink-0 text-current" : "size-3 shrink-0 text-[#bdbdbd]"} />
        <span className={isToolbar ? "hidden min-w-0 truncate sm:inline" : "min-w-0 flex-1 whitespace-normal break-words text-left leading-4"}>
          {selectedLabel}
        </span>
      </button>

      {isOpen ? (
        <div
          id={listboxId.current}
          role="listbox"
          aria-label="Microphone options"
          className={`${isToolbar ? "right-0 top-full mt-1 w-[320px] max-w-[calc(100vw-1rem)]" : "left-0 top-full mt-1 w-full min-w-[260px]"} absolute z-50 max-h-64 overflow-y-auto rounded-[9px] border border-[#5c5c5c] bg-[#303030] p-1 shadow-2xl shadow-black/40`}
        >
          {devices.map((device) => {
            const selected = device.id === selectedDeviceId;

            return (
              <button
                key={device.id}
                type="button"
                role="option"
                aria-selected={selected}
                className={`flex w-full min-w-0 items-start gap-2 rounded-[7px] px-2.5 py-2 text-left text-[12px] font-semibold leading-4 transition ${
                  selected ? "bg-[#5a5a5a] text-white" : "text-[#dddddd] hover:bg-[#454545]"
                }`}
                onClick={() => handleSelect(device.id)}
              >
                <AudioInputDeviceIcon device={device} className="mt-0.5 size-3 shrink-0 text-[#bdbdbd]" />
                <span className="min-w-0 flex-1 whitespace-normal break-words">{device.label}</span>
                {selected ? <Check className="mt-0.5 size-3 shrink-0 text-[#9bcfff]" /> : null}
              </button>
            );
          })}
        </div>
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
}: SoundViewProps) {
  return (
    <ViewFrame title="Sound">
      <GroupedPanel title="Input">
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
                <button
                  type="button"
                  aria-label="Refresh microphones"
                  className="inline-flex h-8 items-center justify-center gap-1.5 rounded-md border border-[#5c5c5c] bg-[#303030] px-2.5 text-[12px] font-semibold text-[#eeeeee] transition hover:bg-[#4a4a4a] active:scale-[0.97] disabled:cursor-not-allowed disabled:text-[#8a8a8a] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#9bcfff]"
                  disabled={audioInputDevicesLoading}
                  onClick={onRefreshAudioInputs}
                >
                  <RefreshCw className={`size-3 ${audioInputDevicesLoading ? "animate-spin" : ""}`} />
                  <span>Refresh</span>
                </button>
              </div>
              {audioInputDevicesError ? (
                <p role="status" className="text-[12px] font-medium text-[#ffb3aa]">
                  {audioInputDevicesError}
                </p>
              ) : null}
            </div>
          )}
        />
        <PanelRow icon={<BrainCircuit className="size-3.5" />} title="Recognition model" detail={selectedModel} trailing={<StatusLabel>Local</StatusLabel>} />
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
            aria-pressed={selectedModel === model.name}
            className={`flex w-full items-center gap-3 border-t ${panelDividerClass} p-4 text-left transition first:border-t-0 hover:bg-[#4b4b4b]/70 ${
              selectedModel === model.name ? "bg-[#4b4b4b]/70" : ""
            }`}
            onClick={() => onSelectModel(model.name)}
          >
            <div className={iconTileClass}>
              <BrainCircuit className="size-3.5" />
            </div>
            <div className="min-w-0 flex-1">
              <p className="truncate text-[14px] font-semibold text-[#f2f2f2]">{model.name}</p>
              <p className="mt-0.5 truncate text-[12px] font-medium text-[#aaa]">{model.detail}</p>
              <div className="mt-2 flex flex-wrap gap-1.5">
                <ModelTag>{model.speed}</ModelTag>
                <ModelTag>{model.status}</ModelTag>
              </div>
            </div>
            {selectedModel === model.name ? (
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

interface ModelTagProps {
  children: ReactNode;
}

function ModelTag({ children }: ModelTagProps) {
  return <span className="rounded-[7px] bg-[#303030] px-2 py-1 text-[11px] font-semibold text-[#c8c8c8]">{children}</span>;
}

interface HistoryViewProps {
  rows: TranscriptHistoryRow[];
  onCopyRow: (text: string) => void;
  onDeleteRow: (rowId: string) => void;
}

function HistoryView({ rows, onCopyRow, onDeleteRow }: HistoryViewProps) {
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
      <div className="flex h-10 items-center gap-2 rounded-full border border-[#464646] bg-[#282828]/85 px-3 text-[#8e8e8e] shadow-[inset_0_1px_0_rgba(255,255,255,0.035)]">
        <Search className="size-3.5 shrink-0" />
        <input
          type="search"
          aria-label="Search history"
          className="min-w-0 flex-1 bg-transparent text-[13px] font-semibold text-[#e8e8e8] outline-none placeholder:text-[#777]"
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
                    onDelete={() => onDeleteRow(row.id)}
                    onToggle={() => setExpandedRowId((current) => (current === row.id ? null : row.id))}
                  />
                ))}
              </div>
            </section>
          ))}
        </div>
      ) : (
        <div className={`${panelSurfaceClass} p-6 text-center`}>
          <p className="text-[14px] font-semibold text-[#eeeeee]">No transcription history yet</p>
          <p className="mt-1 text-[12px] leading-5 text-[#b4b4b4]">Stop a recording after dictation and the transcript will appear here.</p>
        </div>
      )}
    </section>
  );
}

interface HistoryCardProps {
  row: TranscriptHistoryRow;
  expanded: boolean;
  onCopy: () => void;
  onDelete: () => void;
  onToggle: () => void;
}

function HistoryCard({ row, expanded, onCopy, onDelete, onToggle }: HistoryCardProps) {
  return (
    <article className={`${panelSurfaceClass} transition ${expanded ? "p-4" : "p-0"}`}>
      <button
        type="button"
        className={`block w-full text-left ${expanded ? "" : "px-4 py-4"}`}
        onClick={onToggle}
      >
        <p className={`${expanded ? "line-clamp-3" : "truncate"} text-[13px] font-semibold leading-5 text-[#f1f1f1]`}>
          {row.title}
        </p>
        {expanded && row.status !== "failed" && row.text && row.text !== row.title ? (
          <p className="mt-2 text-[12px] font-medium leading-5 text-[#c7c7c7]">{row.text}</p>
        ) : null}
      </button>

      {expanded ? (
        <div className="mt-3 space-y-3">
          {row.recordingUrl ? <HistoryRecordingPlayer title={row.title} src={row.recordingUrl} durationSeconds={row.durationSeconds} /> : null}
          {row.status === "failed" ? (
            <p className="rounded-[8px] border border-[#5c5c5c] bg-[#303030] px-3 py-2 text-[12px] font-medium text-[#ffb3aa]">
              {row.error ?? "Transcription failed"}
            </p>
          ) : null}
          <div className="flex items-center justify-between">
            <div className="inline-flex rounded-[8px] bg-[#5a5a5a]/45 p-0.5">
              <span className="rounded-[7px] bg-[#777] px-2 py-1 text-[12px] font-semibold text-white">Original</span>
              <span className="px-2 py-1 text-[12px] font-semibold text-[#9d9d9d]">Segmented</span>
            </div>
            <div className="flex items-center gap-1 text-[#bdbdbd]">
              <button type="button" aria-label={`Copy transcript: ${row.title}`} className={`grid size-7 place-items-center ${sharedRadiusClass} transition hover:bg-[#555]`} onClick={onCopy}>
                <Copy className="size-3" />
              </button>
              <button type="button" aria-label={`Delete transcript: ${row.title}`} className={`grid size-7 place-items-center ${sharedRadiusClass} transition hover:bg-[#555]`} onClick={onDelete}>
                <Trash2 className="size-3" />
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </article>
  );
}

interface HistoryRecordingPlayerProps {
  title: string;
  src: string;
  durationSeconds: number;
}

function HistoryRecordingPlayer({ title, src, durationSeconds }: HistoryRecordingPlayerProps) {
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
    <div className={`flex items-center gap-3 ${sharedRadiusClass} border border-[#626262]/70 bg-[#606060]/45 px-3 py-2`}>
      <button
        type="button"
        aria-label={`${isPlaying ? "Pause" : "Play"} recording: ${title}`}
        className={`grid size-7 shrink-0 place-items-center ${sharedRadiusClass} text-white transition active:scale-[0.97] ${isPlaying ? "bg-[#0a84ff]" : "bg-[#6e6e6e] hover:bg-[#777]"}`}
        onClick={togglePlayback}
      >
        {isPlaying ? <Pause className="size-3" /> : <Play className="size-3" />}
      </button>
      <div className="flex min-w-0 flex-1 items-center gap-px overflow-hidden" aria-hidden="true">
        {historyWaveformBars.map((height, index) => (
          <span
            key={`history-wave-${index}`}
            className="w-px shrink-0 rounded-full bg-[#a9a9a9]"
            style={{ height: `${height}px`, opacity: index % 4 === 0 ? 0.35 : 0.62 }}
          />
        ))}
      </div>
      <span className="shrink-0 text-[12px] font-semibold text-[#e0e0e0]">{formatDuration(durationSeconds)}</span>
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
  overlayPlacement: OverlayPlacement;
  onOverlayPlacementChange: (placement: OverlayPlacement) => void;
}

function SettingsView({ runtimeInfo, selectedModel, overlayPlacement, onOverlayPlacementChange }: SettingsViewProps) {
  const shortcutParts = formatShortcutParts(runtimeInfo?.shortcut);

  return (
    <ViewFrame title="Configuration">
      <GroupedPanel title="Recording window">
        <div className="p-4">
          <div className="mb-2 flex items-center justify-between">
            <span className="text-[13px] font-semibold text-[#f0f0f0]">Style</span>
            <StatusLabel>Mini</StatusLabel>
          </div>
          <div className="grid grid-cols-3 gap-3">
            <RecordingWindowPreview label="Classic" variant="classic" />
            <RecordingWindowPreview label="Mini" variant="mini" selected />
            <RecordingWindowPreview label="Hidden" variant="hidden" />
          </div>
          <div className={`mt-4 flex min-w-0 items-center justify-between border-t ${panelDividerClass} pt-3`}>
            <div>
              <p className="text-[13px] font-semibold text-[#eeeeee]">Position</p>
            </div>
            <OverlayPlacementControl placement={overlayPlacement} onChange={onOverlayPlacementChange} />
          </div>
        </div>
      </GroupedPanel>

      <GroupedPanel title="Keyboard shortcuts">
        <PanelRow title="Toggle recording" detail="Start or stop recording" trailing={<ShortcutCluster parts={shortcutParts} />} />
        <PanelRow title="Cancel recording" detail="Discard active recording" trailing={<ShortcutBadge>esc</ShortcutBadge>} />
      </GroupedPanel>

      <GroupedPanel title="Application">
        <PanelRow title="Default model" detail={runtimeInfo?.defaultModelRepo ?? selectedModel} trailing={<StatusLabel>Active</StatusLabel>} />
        <PanelRow title="Data folder" detail={runtimeInfo?.dataDir ?? "App-contained data directory"} />
        <PanelRow title="Single instance" trailing={<StatusLabel>On</StatusLabel>} />
        <PanelRow title="Tray and overlay" trailing={<StatusLabel>On</StatusLabel>} />
      </GroupedPanel>

      <div className={panelSurfaceClass}>
        <button type="button" className="flex h-11 w-full items-center justify-between px-4 text-left text-[13px] font-semibold text-[#eeeeee] transition hover:bg-[#484848]/70">
          <span>Advanced settings</span>
          <ChevronDown className="-rotate-90 size-4 text-[#d2d2d2]" />
        </button>
      </div>
    </ViewFrame>
  );
}

interface RecordingWindowPreviewProps {
  label: string;
  selected?: boolean;
  variant: "classic" | "mini" | "hidden";
}

function RecordingWindowPreview({ label, selected = false, variant }: RecordingWindowPreviewProps) {
  return (
    <div className={`${sharedRadiusClass} border p-1.5 text-center ${selected ? "border-[#0a84ff] bg-[#28343a]" : "border-[#5b5b5b] bg-[#262626]/70"}`}>
      <div className="grid h-12 place-items-center rounded-[7px] bg-[#111] text-[#d8d8d8]">
        {variant === "hidden" ? (
          <span className="text-[15px] text-[#6f6f6f]">⊘</span>
        ) : (
          <span className={`flex items-center gap-px ${variant === "mini" ? "w-10" : "w-16"}`} aria-hidden="true">
            {historyWaveformBars.slice(0, variant === "mini" ? 18 : 30).map((height, index) => (
              <span
                key={`${label}-${index}`}
                className="w-px rounded-full bg-[#d7d7d7]"
                style={{ height: `${Math.max(4, Math.round(height * 0.45))}px`, opacity: index % 3 === 0 ? 0.55 : 0.9 }}
              />
            ))}
          </span>
        )}
      </div>
      <p className={`mt-1 text-[11px] font-semibold ${selected ? "text-white" : "text-[#b6b6b6]"}`}>{label}</p>
    </div>
  );
}

interface OverlayPlacementControlProps {
  placement: OverlayPlacement;
  onChange: (placement: OverlayPlacement) => void;
}

function OverlayPlacementControl({ placement, onChange }: OverlayPlacementControlProps) {
  return (
    <div className={`inline-flex ${sharedRadiusClass} bg-[#2b2b2b] p-0.5`}>
      {(["top", "bottom"] as const).map((option) => {
        const active = placement === option;
        const label = option === "top" ? "Top" : "Bottom";

        return (
          <button
            key={option}
            type="button"
            aria-label={`${label} overlay position`}
            aria-pressed={active}
            className={`h-7 rounded-[7px] px-2.5 text-[12px] font-semibold transition ${
              active ? "bg-[#686868] text-white" : "text-[#aaa] hover:text-white"
            }`}
            onClick={() => onChange(option)}
          >
            {label}
          </button>
        );
      })}
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
  children: ReactNode;
}

function GroupedPanel({ title, children }: GroupedPanelProps) {
  return (
    <section className="space-y-2">
      {title ? <h3 className="px-1 text-[13px] font-semibold text-[#a8a8a8]">{title}</h3> : null}
      <div className={panelSurfaceClass}>{children}</div>
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
          {detail ? <p className="mt-0.5 truncate text-[12px] font-medium text-[#aaa]">{detail}</p> : null}
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
      {parts.map((part, index) => (
        <ShortcutBadge key={`${part}-${index}`} muted={muted}>{part}</ShortcutBadge>
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
