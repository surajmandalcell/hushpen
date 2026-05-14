import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties, type MutableRefObject, type ReactNode } from "react";
import {
  Activity,
  BrainCircuit,
  CheckCircle2,
  CircleStop,
  Database,
  History,
  Home,
  Keyboard,
  Layers2,
  Library,
  Mic2,
  Pause,
  Play,
  Settings,
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
const modelIdsByName: Record<string, string> = {
  [defaultModelName]: "whisper-base",
  [parakeetModelName]: "parakeet-tdt-0.6b-v3",
  "Whisper Large V3 Turbo": "whisper-large",
};
const transcriptHistoryStorageKey = "asrpro.transcriptHistory.v1";
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

function formatRelativeTime(createdAt: number, now = Date.now()) {
  const elapsedSeconds = Math.max(0, Math.floor((now - createdAt) / 1000));
  if (elapsedSeconds < 45) return "Just now";
  if (elapsedSeconds < 90) return "1 min ago";

  const elapsedMinutes = Math.floor(elapsedSeconds / 60);
  if (elapsedMinutes < 60) return `${elapsedMinutes} min ago`;

  const elapsedHours = Math.floor(elapsedMinutes / 60);
  if (elapsedHours < 24) return `${elapsedHours} hr ago`;
  if (elapsedHours < 48) return "Yesterday";

  return historyDateFormatter.format(new Date(createdAt));
}

function countWords(text: string) {
  return text.trim().split(/\s+/).filter(Boolean).length;
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
    { value: "1", label: "Apps used" },
    { value: savedMinutes ? `${savedMinutes} minute${savedMinutes === 1 ? "" : "s"}` : "0 minutes", label: "Saved this week" },
  ];
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

function useMicrophoneWaveform(active: boolean, barsRef: MutableRefObject<Array<HTMLSpanElement | null>>) {
  const frameRef = useRef<number[]>(idleWaveformFrame);
  const lastOverlayFrameAtRef = useRef(0);
  const audioLevelRef = useRef(0);

  useEffect(() => {
    return audioRecordingService.subscribe((state) => {
      audioLevelRef.current = state.audioLevel;
    });
  }, []);

  useEffect(() => {
    const writeFrame = (frame: number[]) => {
      for (let index = 0; index < waveformBaseBars.length; index += 1) {
        const bar = barsRef.current[index];
        if (!bar) continue;

        const baseBar = waveformBaseBars[index];
        bar.style.setProperty("--wave-height", `${frame[index] ?? baseBar.baseHeight}px`);
        bar.style.setProperty("--wave-opacity", String(baseBar.opacity));
      }
    };

    if (!active) {
      frameRef.current = idleWaveformFrame;
      writeFrame(idleWaveformFrame);
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
          writeFrame(idleWaveformFrame);
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
        writeFrame(nextFrame);

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
      writeFrame(idleWaveformFrame);
      sendOverlayWaveformFrame(idleWaveformFrame, false);
    };
  }, [active, barsRef]);
}

function App() {
  const [activeView, setActiveView] = useState<ViewId>("home");
  const [isRecording, setIsRecording] = useState(false);
  const [recordingStatus, setRecordingStatus] = useState<RecordingStatus>("idle");
  const [recordingError, setRecordingError] = useState<string | null>(null);
  const [recordingDurationSeconds, setRecordingDurationSeconds] = useState(0);
  const [selectedModel, setSelectedModel] = useState(defaultModelName);
  const [runtimeInfo, setRuntimeInfo] = useState<RuntimeInfo | null>(null);
  const [overlayPlacement, setOverlayPlacement] = useState<OverlayPlacement>("top");
  const [historyFilter, setHistoryFilter] = useState("All");
  const [historyRows, setHistoryRows] = useState<TranscriptHistoryRow[]>(loadTranscriptHistory);
  const recordingStartedAtRef = useRef<number | null>(null);
  const recordingTransitionRef = useRef<"starting" | "stopping" | null>(null);

  const addHistoryRow = useCallback((row: TranscriptHistoryRow) => {
    setHistoryRows((current) => {
      const next = [row, ...current].slice(0, 100);
      saveTranscriptHistory(next);
      return next;
    });
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
  }, [syncRecordingBridge]);

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
      setActiveView("history");
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
      setActiveView("history");
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
        <section className="grid min-h-0 min-w-0 grid-rows-[48px_minmax(0,1fr)] bg-[#363636] sm:border-l sm:border-[#5b5b5b]">
          <Toolbar activeTitle={activeTitle} />
          <main className="min-h-0 min-w-0 overflow-y-auto px-4 pb-6 pt-5 sm:px-4 lg:px-4">
            {activeView === "home" && (
              <HomeView
                isRecording={isRecording}
                recordingStatus={recordingStatus}
                recordingError={recordingError}
                durationSeconds={recordingDurationSeconds}
                selectedModel={selectedModel}
                historyRows={historyRows}
                onToggleRecording={() => handleSetRecording(!isRecording)}
                onOpenHistory={() => setActiveView("history")}
              />
            )}
            {activeView === "sound" && <SoundView selectedModel={selectedModel} isRecording={isRecording} />}
            {activeView === "models" && <ModelsView selectedModel={selectedModel} onSelectModel={setSelectedModel} />}
            {activeView === "history" && <HistoryView rows={historyRows} activeFilter={historyFilter} onFilterChange={setHistoryFilter} />}
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
}

function Toolbar({ activeTitle }: ToolbarProps) {
  return (
    <header className="flex min-w-0 items-center justify-between border-b border-[#545454] bg-[#3f3f3f] px-4 [-webkit-app-region:drag]">
      <div className="flex min-w-0 items-center gap-3">
        <button
          type="button"
          aria-label="Split view"
          className="grid size-7 place-items-center rounded-md text-[#cfcfcf] transition hover:bg-[#505050] [-webkit-app-region:no-drag]"
        >
          <Layers2 className="size-4" />
        </button>
        <span className="truncate text-[13px] font-medium text-[#cfcfcf]">{activeTitle}</span>
      </div>
      <div className="inline-flex min-w-0 items-center gap-2 rounded-md px-2.5 py-1 text-[13px] font-medium text-[#d8d8d8]">
        <span className="hidden truncate sm:inline">MacBook Pro Microphone (Default)</span>
        <Mic2 className="size-4 shrink-0 text-[#cfcfcf]" />
      </div>
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
  onToggleRecording: () => void;
  onOpenHistory: () => void;
}

function HomeView({
  isRecording,
  recordingStatus,
  recordingError,
  durationSeconds,
  selectedModel,
  historyRows,
  onToggleRecording,
  onOpenHistory,
}: HomeViewProps) {
  const isBusy = recordingStatus === "starting" || recordingStatus === "transcribing";
  const statusDetail = recordingStatus === "starting"
    ? "Opening microphone..."
    : recordingStatus === "transcribing"
      ? "Transcribing captured audio..."
      : isRecording
        ? `Recording ${formatDuration(durationSeconds)}`
        : selectedModel;

  const stats = buildHomeStats(historyRows);

  return (
    <section className="mx-auto flex w-full max-w-[520px] flex-col gap-5">
      <div className="grid grid-cols-2 overflow-hidden rounded-[14px] bg-[#303030] sm:grid-cols-4">
        {stats.map((stat) => (
          <div key={stat.label} className="px-4 py-4">
            <p className="text-[15px] font-bold leading-none text-[#f3f3f3]">{stat.value}</p>
            <p className="mt-2 text-[11px] font-semibold leading-none text-[#a4a4a4]">{stat.label}</p>
          </div>
        ))}
      </div>

      <span
        aria-label={isRecording ? "Recording active" : "Recording inactive"}
        className="sr-only"
      />

      <section>
        <h2 className="mb-3 text-[13px] font-bold text-[#a9a9a9]">Get started</h2>
        <div className="space-y-1">
          <HomeActionRow
            icon={<Mic2 className="size-4" />}
            title={isRecording ? "Stop recording" : "Start recording"}
            detail={statusDetail}
            trailing={(
              <PrimaryButton onClick={onToggleRecording} disabled={isBusy}>
                {isRecording ? <CircleStop className="size-4" /> : <Mic2 className="size-4" />}
                {isRecording ? "Stop Recording" : recordingStatus === "transcribing" ? "Transcribing" : "Start Recording"}
              </PrimaryButton>
            )}
          />
          <HomeActionRow icon={<Keyboard className="size-4" />} title="Customize shortcuts" detail="Global recording is wired through the desktop bridge." />
        </div>
      </section>

      {recordingError ? (
        <p role="alert" className="px-1 text-[12px] font-medium text-[#ff9c8f]">
          {recordingError}
        </p>
      ) : null}

      <section>
        <div className="mb-2 flex items-center justify-between">
          <h2 className="text-[13px] font-bold text-[#a9a9a9]">What's new?</h2>
          <button type="button" className="text-[12px] font-semibold text-[#ececec] hover:text-white" onClick={onOpenHistory}>
            View history
          </button>
        </div>
        <div className="overflow-hidden rounded-[8px] border border-[#5c5c5c] bg-[#404040]">
          <UpdateRow date="May 14" title="Compact dark home" detail="Dark split window, compact navigation, stats, and action rows." />
          <UpdateRow date="May 13" title="Recording history playback" detail="Every dictation keeps its playable source audio, even when transcription fails." />
          <UpdateRow date="May 13" title="Local Whisper sidecar" detail="Desktop development now starts the local sidecar and uses the working Whisper Base model." />
        </div>
      </section>

      <Waveform active={isRecording} />
    </section>
  );
}

interface HomeActionRowProps {
  icon: ReactNode;
  title: string;
  detail: string;
  trailing?: ReactNode;
  onClick?: () => void;
}

function HomeActionRow({ icon, title, detail, trailing, onClick }: HomeActionRowProps) {
  const content = (
    <>
      <div className="grid size-8 shrink-0 place-items-center text-[#a8a8a8]">{icon}</div>
      <div className="min-w-0 flex-1">
        <p className="truncate text-[14px] font-bold leading-5 text-[#eeeeee]">{title}</p>
        <p className="truncate text-[13px] font-semibold leading-5 text-[#a8a8a8]">{detail}</p>
      </div>
      {trailing ? <div className="shrink-0">{trailing}</div> : null}
    </>
  );

  if (onClick) {
    return (
      <button type="button" className="flex w-full items-center gap-2 rounded-[8px] px-2 py-1.5 text-left transition hover:bg-[#424242]" onClick={onClick}>
        {content}
      </button>
    );
  }

  return <div className="flex w-full items-center gap-2 rounded-[8px] px-2 py-1.5">{content}</div>;
}

interface UpdateRowProps {
  date: string;
  title: string;
  detail: string;
}

function UpdateRow({ date, title, detail }: UpdateRowProps) {
  return (
    <div className="grid grid-cols-[56px_minmax(0,1fr)] gap-3 border-t border-[#5c5c5c] px-4 py-3 first:border-t-0">
      <p className="text-[12px] font-bold text-[#8d8d8d]">{date}</p>
      <div className="min-w-0">
        <p className="truncate text-[14px] font-bold text-[#eeeeee]">{title}</p>
        <p className="mt-1 text-[13px] font-semibold leading-5 text-[#b6b6b6]">{detail}</p>
      </div>
    </div>
  );
}

interface SoundViewProps {
  selectedModel: string;
  isRecording: boolean;
}

function SoundView({ selectedModel, isRecording }: SoundViewProps) {
  return (
    <ViewFrame title="Sound">
      <GroupedPanel title="Input">
        <PanelRow icon={<Mic2 className="size-4" />} title="MacBook Pro Microphone" detail={isRecording ? "Recording is active" : "Default input device"} trailing={<StatusPill tone={isRecording ? "red" : "green"}>{isRecording ? "Live" : "Default"}</StatusPill>} />
        <PanelRow icon={<BrainCircuit className="size-4" />} title="Recognition model" detail={selectedModel} trailing={<StatusPill tone="blue">Local</StatusPill>} />
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
    <ViewFrame title="Speech models">
      <GroupedPanel>
        {modelCards.map((model) => (
          <button
            key={model.name}
            type="button"
            className="flex w-full items-center gap-3 border-t border-[#5c5c5c] p-3 text-left first:border-t-0 hover:bg-[#4a4a4a]"
            onClick={() => onSelectModel(model.name)}
          >
            <div className="grid size-8 shrink-0 place-items-center rounded-md bg-[#303030] text-[#d7d7d7]">
              <BrainCircuit className="size-4" />
            </div>
            <div className="min-w-0 flex-1">
              <p className="truncate text-[13px] font-semibold text-[#eeeeee]">{model.name}</p>
              <p className="truncate text-[12px] text-[#b4b4b4]">{model.detail}</p>
            </div>
            <div className="hidden text-right text-[12px] text-[#a8a8a8] sm:block">
              <p>{model.speed}</p>
              <p>{model.status}</p>
            </div>
            {selectedModel === model.name ? <CheckCircle2 className="size-4 text-[#88c7ff]" /> : null}
          </button>
        ))}
      </GroupedPanel>
    </ViewFrame>
  );
}

interface HistoryViewProps {
  rows: TranscriptHistoryRow[];
  activeFilter: string;
  onFilterChange: (filter: string) => void;
}

function HistoryView({ rows, activeFilter, onFilterChange }: HistoryViewProps) {
  const filters = ["All", "Completed", "Failed"];
  const filteredRows = rows.filter((row) => {
    if (activeFilter === "All") return true;
    return row.status === activeFilter.toLowerCase();
  });

  return (
    <ViewFrame title="Transcript library">
      <div className="inline-flex rounded-lg border border-[#5c5c5c] bg-[#303030] p-0.5">
        {filters.map((filter) => (
          <button
            key={filter}
            type="button"
            className={`rounded-md px-3 py-1.5 text-[13px] font-medium transition ${
              activeFilter === filter ? "bg-[#686868] text-white" : "text-[#b6b6b6] hover:text-white"
            }`}
            onClick={() => onFilterChange(filter)}
          >
            {filter}
          </button>
        ))}
      </div>

      {filteredRows.length ? (
        <GroupedPanel>
          {filteredRows.map((row) => (
            <PanelRow
              key={row.id}
              icon={<History className="size-4" />}
              title={row.title}
              detail={row.status === "failed" ? `${row.kind} - ${formatDuration(row.durationSeconds)} - ${row.error ?? "Failed"}` : `${row.kind} - ${formatDuration(row.durationSeconds)} - ${row.model}`}
              trailing={<span className="text-[12px] font-medium text-[#77777f]">{formatRelativeTime(row.createdAt)}</span>}
              extra={row.recordingUrl ? <HistoryRecordingPlayer title={row.title} src={row.recordingUrl} /> : null}
            />
          ))}
        </GroupedPanel>
      ) : (
        <div className="rounded-[10px] border border-[#5c5c5c] bg-[#404040] p-6 text-center">
          <p className="text-[14px] font-semibold text-[#eeeeee]">No transcription history yet</p>
          <p className="mt-1 text-[12px] leading-5 text-[#b4b4b4]">Stop a recording after dictation and the transcript will appear here.</p>
        </div>
      )}
    </ViewFrame>
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
    <div className="flex max-w-[420px] items-center gap-2 rounded-[8px] border border-[#5c5c5c] bg-[#3a3a3a] px-2 py-2">
      <button
        type="button"
        aria-label={`${isPlaying ? "Pause" : "Play"} recording: ${title}`}
        className="grid size-7 shrink-0 place-items-center rounded-[8px] bg-[#0a84ff] text-white transition hover:bg-[#0877e8] active:scale-[0.97]"
        onClick={togglePlayback}
      >
        {isPlaying ? <Pause className="size-3.5" /> : <Play className="size-3.5" />}
      </button>
      <span className="min-w-0 flex-1 truncate text-[12px] font-semibold text-[#b6b6b6]">Saved recording</span>
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
  return (
    <ViewFrame title="Desktop behavior">
      <GroupedPanel title="Storage">
        <PanelRow icon={<Database className="size-4" />} title="Data folder" detail={runtimeInfo?.dataDir ?? "App-contained data directory"} />
        <PanelRow icon={<BrainCircuit className="size-4" />} title="Default model" detail={runtimeInfo?.defaultModelRepo ?? selectedModel} trailing={<StatusPill tone="green">Active</StatusPill>} />
      </GroupedPanel>

      <GroupedPanel title="Desktop integration">
        <PanelRow icon={<Layers2 className="size-4" />} title="Single instance" detail="Launching again focuses the running app" trailing={<StatusPill tone="green">On</StatusPill>} />
        <PanelRow icon={<Activity className="size-4" />} title="Tray and overlay" detail="Close hides to tray; hotkey recording shows a draggable floating pill" trailing={<StatusPill tone="green">On</StatusPill>} />
        <PanelRow
          icon={<Settings className="size-4" />}
          title="Recording overlay position"
          detail={runtimeInfo?.overlaySettings?.customBounds ? "Drag position remembered for that monitor" : "Choose the default edge for the hotkey overlay"}
          trailing={<OverlayPlacementControl placement={overlayPlacement} onChange={onOverlayPlacementChange} />}
        />
      </GroupedPanel>
    </ViewFrame>
  );
}

interface OverlayPlacementControlProps {
  placement: OverlayPlacement;
  onChange: (placement: OverlayPlacement) => void;
}

function OverlayPlacementControl({ placement, onChange }: OverlayPlacementControlProps) {
  return (
    <div className="inline-flex rounded-lg border border-[#5c5c5c] bg-[#303030] p-0.5">
      {(["top", "bottom"] as const).map((option) => {
        const active = placement === option;
        const label = option === "top" ? "Top" : "Bottom";

        return (
          <button
            key={option}
            type="button"
            aria-label={`${label} overlay position`}
            aria-pressed={active}
            className={`h-7 rounded-md px-2.5 text-[12px] font-semibold transition ${
              active ? "bg-[#686868] text-white" : "text-[#b6b6b6] hover:text-white"
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
    <section className="mx-auto w-full max-w-4xl space-y-4">
      <div>
        <h2 className="text-[22px] font-semibold tracking-normal text-[#f0f0f0] sm:text-[24px]">{title}</h2>
      </div>
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
    <section>
      {title ? <h3 className="mb-2 px-1 text-[11px] font-semibold uppercase tracking-[0.04em] text-[#a9a9a9]">{title}</h3> : null}
      <div className="overflow-hidden rounded-[10px] border border-[#5c5c5c] bg-[#404040]">{children}</div>
    </section>
  );
}

interface PanelRowProps {
  icon: ReactNode;
  title: string;
  detail: string;
  trailing?: ReactNode;
  extra?: ReactNode;
}

function PanelRow({ icon, title, detail, trailing, extra }: PanelRowProps) {
  return (
    <div className="border-t border-[#5c5c5c] p-3 first:border-t-0">
      <div className="flex min-w-0 items-center gap-3">
        <div className="grid size-8 shrink-0 place-items-center rounded-md bg-[#303030] text-[#d7d7d7]">{icon}</div>
        <div className="min-w-0 flex-1">
          <p className="truncate text-[13px] font-semibold text-[#eeeeee]">{title}</p>
          <p className="truncate text-[12px] text-[#b4b4b4]">{detail}</p>
        </div>
        {trailing ? <div className="shrink-0">{trailing}</div> : null}
      </div>
      {extra ? <div className="mt-2 pl-11">{extra}</div> : null}
    </div>
  );
}

interface WaveformProps {
  active: boolean;
}

function Waveform({ active }: WaveformProps) {
  const barsRef = useRef<Array<HTMLSpanElement | null>>([]);
  useMicrophoneWaveform(active, barsRef);

  return (
    <div className={`in-app-waveform ${active ? "is-active" : ""}`} role="img" aria-label="Live recording waveform">
      <div className="in-app-waveform__bars" aria-hidden="true">
        {waveformBaseBars.map((bar, index) => (
          <span
            key={bar.id}
            ref={(element) => {
              barsRef.current[index] = element;
            }}
            style={{
              "--wave-height": `${bar.baseHeight}px`,
              "--wave-opacity": bar.opacity,
            } as CSSProperties}
          />
        ))}
      </div>
    </div>
  );
}

interface StatusPillProps {
  children: ReactNode;
  tone?: "blue" | "green" | "gray" | "red";
}

function StatusPill({ children, tone = "gray" }: StatusPillProps) {
  const tones = {
    blue: "bg-[#284862] text-[#9fd2ff] border-[#3b6e94]",
    green: "bg-[#244735] text-[#9ee0b6] border-[#3b7654]",
    gray: "bg-[#303030] text-[#d0d0d0] border-[#5c5c5c]",
    red: "bg-[#5a2c29] text-[#ffb3aa] border-[#8c4942]",
  };

  return <span className={`inline-flex rounded-full border px-2 py-0.5 text-[11px] font-semibold ${tones[tone]}`}>{children}</span>;
}

interface PrimaryButtonProps {
  children: ReactNode;
  onClick: () => void;
  disabled?: boolean;
}

function PrimaryButton({ children, onClick, disabled = false }: PrimaryButtonProps) {
  return (
    <button
      type="button"
      disabled={disabled}
      className="inline-flex h-8 items-center justify-center gap-2 rounded-md border border-[#6b6b6b] bg-[#5a5a5a] px-3 text-[12px] font-bold text-white shadow-none transition hover:bg-[#686868] active:scale-[0.97] disabled:cursor-not-allowed disabled:border-[#555] disabled:bg-[#484848] disabled:text-[#9b9b9b] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#9bcfff]"
      onClick={onClick}
    >
      {children}
    </button>
  );
}

export default App;
