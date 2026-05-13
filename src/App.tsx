import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties, type MutableRefObject, type ReactNode } from "react";
import {
  Activity,
  BrainCircuit,
  CheckCircle2,
  CircleStop,
  Database,
  FileAudio2,
  FolderUp,
  History,
  Layers2,
  Mic2,
  Settings,
  UploadCloud,
  type LucideIcon,
} from "lucide-react";
import { apiClient } from "./services/api";
import { audioRecordingService } from "./services/audioRecording";

type ViewId = "dashboard" | "files" | "models" | "history" | "settings";
type WindowAction = "minimize" | "maximize" | "close";
type OverlayPlacement = "top" | "bottom";
type RecordingStatus = "idle" | "starting" | "recording" | "transcribing" | "error";

interface AudioFile {
  fileName: string;
  path: string;
}

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
  error?: string;
}

const navItems: NavItem[] = [
  { id: "dashboard", label: "Dashboard", icon: Activity },
  { id: "files", label: "Files", icon: FileAudio2 },
  { id: "models", label: "Models", icon: BrainCircuit },
  { id: "history", label: "History", icon: History },
  { id: "settings", label: "Settings", icon: Settings },
];

const defaultModelName = "Parakeet-TDT-0.6B-v3";
const transcriptHistoryStorageKey = "asrpro.transcriptHistory.v1";

const modelCards = [
  {
    name: defaultModelName,
    detail: "NVIDIA NeMo, multilingual, punctuation and timestamps",
    speed: "Default",
    status: "Active",
  },
  {
    name: "Local Whisper",
    detail: "Whisper Base, offline, private",
    speed: "Ready",
    status: "Installed",
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

    return parsed.map(normalizeTranscriptHistoryRow).filter((row): row is TranscriptHistoryRow => Boolean(row));
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

  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
  }).format(new Date(createdAt));
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
}: {
  text: string;
  model: string;
  durationSeconds: number;
  startedAt: number;
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
  };
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
  const [activeView, setActiveView] = useState<ViewId>("dashboard");
  const [isRecording, setIsRecording] = useState(false);
  const [recordingStatus, setRecordingStatus] = useState<RecordingStatus>("idle");
  const [recordingError, setRecordingError] = useState<string | null>(null);
  const [recordingDurationSeconds, setRecordingDurationSeconds] = useState(0);
  const [selectedModel, setSelectedModel] = useState(defaultModelName);
  const [runtimeInfo, setRuntimeInfo] = useState<RuntimeInfo | null>(null);
  const [overlayPlacement, setOverlayPlacement] = useState<OverlayPlacement>("top");
  const [queuedFiles, setQueuedFiles] = useState<AudioFile[]>([
    { fileName: "board-meeting.wav", path: "demo://board-meeting.wav" },
    { fileName: "voice-note.m4a", path: "demo://voice-note.m4a" },
  ]);
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

      const result = await apiClient.transcribeFile(createRecordingFile(audioBlob));
      const text = typeof result === "string" ? result : result?.text;
      if (!text || !text.trim()) {
        throw new Error("No transcription text returned");
      }

      addHistoryRow(createTranscriptHistoryRow({
        text,
        model: selectedModel,
        durationSeconds,
        startedAt,
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

  const activeTitle = useMemo(() => navItems.find((item) => item.id === activeView)?.label ?? "Dashboard", [activeView]);

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

  const handleSelectFiles = useCallback(async () => {
    const files = await window.asrpro?.selectAudioFiles();
    if (files?.length) {
      setQueuedFiles((current) => [...files, ...current]);
      setActiveView("files");
      return;
    }

    setQueuedFiles((current) => [
      { fileName: "sample-interview.mp3", path: "demo://sample-interview.mp3" },
      ...current,
    ]);
    setActiveView("files");
  }, []);

  const handleOverlayPlacementChange = useCallback((placement: OverlayPlacement) => {
    setOverlayPlacement(placement);
    setRuntimeInfo((current) => mergeOverlaySettings(current, { placement, customBounds: null }));

    window.asrpro?.setOverlaySettings?.({ placement }).then((settings) => {
      const nextPlacement = normalizeOverlayPlacement(settings.placement);
      setOverlayPlacement(nextPlacement);
      setRuntimeInfo((current) => mergeOverlaySettings(current, { ...settings, placement: nextPlacement }));
    }).catch(() => {});
  }, []);

  useEffect(() => window.asrpro?.onAddFiles?.(() => {
    void handleSelectFiles();
  }), [handleSelectFiles]);

  return (
    <div className="h-screen w-screen overflow-hidden bg-[#f5f5f7] font-[Inter,-apple-system,BlinkMacSystemFont,'SF_Pro_Text','Segoe_UI',sans-serif] text-[#1d1d1f] antialiased">
      <div className="grid h-full grid-cols-1 grid-rows-[auto_minmax(0,1fr)] md:grid-cols-[220px_minmax(0,1fr)] md:grid-rows-1">
        <Sidebar activeView={activeView} onChange={setActiveView} onWindowAction={handleWindowAction} />
        <section className="grid min-h-0 min-w-0 grid-rows-[48px_minmax(0,1fr)] bg-[#f5f5f7] md:border-l md:border-[#d8d8de]">
          <Toolbar activeTitle={activeTitle} isRecording={isRecording} recordingStatus={recordingStatus} onToggleRecording={() => handleSetRecording(!isRecording)} />
          <main className="min-h-0 min-w-0 overflow-y-auto px-4 pb-6 pt-3 sm:px-6 lg:px-8">
            {activeView === "dashboard" && (
              <DashboardView
                isRecording={isRecording}
                recordingStatus={recordingStatus}
                recordingError={recordingError}
                durationSeconds={recordingDurationSeconds}
                selectedModel={selectedModel}
                onToggleRecording={() => handleSetRecording(!isRecording)}
                onSelectFiles={handleSelectFiles}
              />
            )}
            {activeView === "files" && <FilesView queuedFiles={queuedFiles} onSelectFiles={handleSelectFiles} />}
            {activeView === "models" && <ModelsView selectedModel={selectedModel} onSelectModel={setSelectedModel} />}
            {activeView === "history" && <HistoryView rows={historyRows} activeFilter={historyFilter} onFilterChange={setHistoryFilter} />}
            {activeView === "settings" && (
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
    <aside className="flex min-h-0 flex-col border-b border-[rgba(0,0,0,0.08)] bg-[#f5f5f7]/72 backdrop-blur-2xl md:border-b-0">
      <div className="flex h-12 items-center gap-3 px-4 [-webkit-app-region:drag]">
        <WindowDots onWindowAction={onWindowAction} />
        <p className="truncate text-[13px] font-semibold leading-tight text-[#242429]">ASR Pro</p>
      </div>

      <nav className="flex gap-1 overflow-x-auto px-2.5 pb-3 pt-1 md:block md:min-h-0 md:overflow-y-auto" aria-label="Primary">
        {navItems.map((item) => {
          const Icon = item.icon;
          const isActive = activeView === item.id;

          return (
            <button
              key={item.id}
              type="button"
              aria-label={item.label}
              aria-current={isActive ? "page" : undefined}
              className={`mb-1 flex h-8 shrink-0 items-center gap-2 rounded-md px-2.5 text-left text-[13px] font-medium transition md:w-full ${
                isActive
                  ? "bg-[#d9d9e1] text-[#1d1d1f] shadow-[inset_0_0_0_1px_rgba(0,0,0,0.06)]"
                  : "text-[#55555c] hover:bg-[#e8e8ee]"
              }`}
              onClick={() => onChange(item.id)}
            >
              <Icon className="size-4 shrink-0" />
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
  isRecording: boolean;
  recordingStatus: RecordingStatus;
  onToggleRecording: () => void;
}

function Toolbar({ activeTitle, isRecording, recordingStatus, onToggleRecording }: ToolbarProps) {
  const disabled = recordingStatus === "starting" || recordingStatus === "transcribing";
  const label = isRecording ? "Stop Recording" : recordingStatus === "transcribing" ? "Transcribing" : "Start Recording";

  return (
    <header className="flex min-w-0 items-center justify-between bg-[#f5f5f7]/62 px-4 backdrop-blur-2xl [-webkit-app-region:drag] sm:px-6 lg:px-8">
      <h1 className="truncate text-[14px] font-semibold leading-none text-[#2c2c31]">{activeTitle}</h1>
      <button
        type="button"
        aria-label={isRecording ? "Stop Recording from toolbar" : "Start Recording from toolbar"}
        disabled={disabled}
        onClick={onToggleRecording}
        className="inline-flex h-8 items-center gap-2 rounded-full border border-[rgba(0,0,0,0.08)] bg-white/58 px-3.5 text-[13px] font-normal tracking-[-0.12px] text-[#0066cc] backdrop-blur-2xl transition active:scale-[0.95] disabled:cursor-not-allowed disabled:text-[#86868b] [-webkit-app-region:no-drag]"
      >
        {isRecording ? <CircleStop className="size-4" /> : <Mic2 className="size-4" />}
        <span className="hidden sm:inline">{label}</span>
      </button>
    </header>
  );
}

interface DashboardViewProps {
  isRecording: boolean;
  recordingStatus: RecordingStatus;
  recordingError: string | null;
  durationSeconds: number;
  selectedModel: string;
  onToggleRecording: () => void;
  onSelectFiles: () => void;
}

function DashboardView({ isRecording, recordingStatus, recordingError, durationSeconds, selectedModel, onToggleRecording, onSelectFiles }: DashboardViewProps) {
  const isBusy = recordingStatus === "starting" || recordingStatus === "transcribing";
  const statusDetail = recordingStatus === "starting"
    ? "Opening microphone..."
    : recordingStatus === "transcribing"
      ? "Transcribing captured audio..."
      : isRecording
        ? `Recording ${formatDuration(durationSeconds)}`
        : selectedModel;

  return (
    <section className="mx-auto flex w-full max-w-[760px] flex-col gap-4 pt-1">
      <div className="flex min-w-0 items-center justify-between gap-4 px-1">
        <div className="flex min-w-0 items-center gap-3">
          <span
            className={`grid size-9 shrink-0 place-items-center rounded-full border ${
              isRecording ? "border-[rgba(255,69,58,0.18)] bg-white/58 text-[#ff453a]" : "border-[rgba(0,0,0,0.08)] bg-white/58 text-[#5f6067]"
            }`}
            aria-label={isRecording ? "Recording active" : "Recording inactive"}
          >
            <Mic2 className="size-4" />
          </span>
          <div className="min-w-0">
            <p className="truncate text-[13px] font-semibold leading-5 text-[#242428]">Dictation</p>
            <p className="truncate text-[12px] leading-4 text-[#6e6e73]">{statusDetail}</p>
          </div>
        </div>
        <span
          className={`size-2.5 shrink-0 rounded-full ${isRecording ? "bg-[#ff453a] shadow-[0_0_0_4px_rgba(255,69,58,0.12)]" : "bg-[#b8b8bf]"}`}
          aria-hidden="true"
        />
      </div>

      <Waveform active={isRecording} />

      <div className="flex flex-col items-start gap-2 sm:flex-row">
        <PrimaryButton onClick={onToggleRecording} disabled={isBusy}>
          {isRecording ? <CircleStop className="size-4" /> : <Mic2 className="size-4" />}
          {isRecording ? "Stop Recording" : recordingStatus === "transcribing" ? "Transcribing" : "Start Recording"}
        </PrimaryButton>
        <SecondaryButton onClick={onSelectFiles}>
          <FolderUp className="size-4" />
          Add Files
        </SecondaryButton>
      </div>
      {recordingError ? (
        <p role="alert" className="px-1 text-[12px] font-medium text-[#b3261e]">
          {recordingError}
        </p>
      ) : null}
    </section>
  );
}

interface FilesViewProps {
  queuedFiles: AudioFile[];
  onSelectFiles: () => void;
}

function FilesView({ queuedFiles, onSelectFiles }: FilesViewProps) {
  return (
    <ViewFrame title="Drop audio or video">
      <div className="rounded-[12px] border border-dashed border-[#bfc0c7] bg-[#fbfbfd]/76 p-6 text-center">
        <div className="mx-auto grid size-12 place-items-center rounded-full bg-white/58 text-[#5f6067] backdrop-blur-xl">
          <UploadCloud className="size-6" />
        </div>
        <p className="mt-3 text-[18px] font-semibold text-[#202024]">Drop files here</p>
        <p className="mx-auto mt-1 max-w-md text-[13px] leading-5 text-[#686870]">Choose files from the desktop or drag them onto this window.</p>
        <div className="mt-4 flex justify-center">
          <PrimaryButton onClick={onSelectFiles}>
            <FolderUp className="size-4" />
            Choose Files
          </PrimaryButton>
        </div>
      </div>

      <GroupedPanel title="Queued transcription jobs">
        {queuedFiles.map((file, index) => (
          <PanelRow
            key={file.path}
            icon={<FileAudio2 className="size-4" />}
            title={file.fileName}
            detail={file.path}
            trailing={<StatusPill tone={index === 0 ? "blue" : "gray"}>{index === 0 ? "Next" : "Queued"}</StatusPill>}
          />
        ))}
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
            className="flex w-full items-center gap-3 border-t border-[#e3e3e8] p-3 text-left first:border-t-0 hover:bg-[#f7f7f9]"
            onClick={() => onSelectModel(model.name)}
          >
            <div className="grid size-8 shrink-0 place-items-center rounded-md bg-[#eeeeef] text-[#5f6067]">
              <BrainCircuit className="size-4" />
            </div>
            <div className="min-w-0 flex-1">
              <p className="truncate text-[13px] font-semibold text-[#25252a]">{model.name}</p>
              <p className="truncate text-[12px] text-[#74747b]">{model.detail}</p>
            </div>
            <div className="hidden text-right text-[12px] text-[#77777f] sm:block">
              <p>{model.speed}</p>
              <p>{model.status}</p>
            </div>
            {selectedModel === model.name ? <CheckCircle2 className="size-4 text-[#0066cc]" /> : null}
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
  const filters = ["All", "Dictation", "Files", "Exports"];
  const filteredRows = rows.filter((row) => {
    if (activeFilter === "All") return true;
    if (activeFilter === "Files") return row.kind === "File";
    if (activeFilter === "Exports") return row.status === "completed";
    return row.kind === activeFilter;
  });

  return (
    <ViewFrame title="Transcript library">
      <div className="inline-flex rounded-lg border border-[#d4d4da] bg-[#e9e9ed] p-0.5">
        {filters.map((filter) => (
          <button
            key={filter}
            type="button"
            className={`rounded-md px-3 py-1.5 text-[13px] font-medium transition ${
              activeFilter === filter ? "bg-[#fbfbfd] text-[#1d1d1f] shadow-[0_1px_2px_rgba(0,0,0,0.08)]" : "text-[#696970] hover:text-[#1d1d1f]"
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
            />
          ))}
        </GroupedPanel>
      ) : (
        <div className="rounded-[10px] border border-[#d9d9df] bg-[#fbfbfd]/78 p-6 text-center">
          <p className="text-[14px] font-semibold text-[#25252a]">No transcription history yet</p>
          <p className="mt-1 text-[12px] leading-5 text-[#74747b]">Stop a recording after dictation and the transcript will appear here.</p>
        </div>
      )}
    </ViewFrame>
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
    <div className="inline-flex rounded-lg border border-[#d4d4da] bg-[#e9e9ed] p-0.5">
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
              active ? "bg-[#fbfbfd] text-[#1d1d1f] shadow-[0_1px_2px_rgba(0,0,0,0.12)]" : "text-[#67676f] hover:text-[#1d1d1f]"
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
        <h2 className="text-[22px] font-semibold tracking-normal text-[#1d1d1f] sm:text-[24px]">{title}</h2>
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
      {title ? <h3 className="mb-2 px-1 text-[11px] font-semibold uppercase tracking-[0.04em] text-[#77777f]">{title}</h3> : null}
      <div className="overflow-hidden rounded-[10px] border border-[#d9d9df] bg-[#fbfbfd]/78">{children}</div>
    </section>
  );
}

interface PanelRowProps {
  icon: ReactNode;
  title: string;
  detail: string;
  trailing?: ReactNode;
}

function PanelRow({ icon, title, detail, trailing }: PanelRowProps) {
  return (
    <div className="flex min-w-0 items-center gap-3 border-t border-[#e3e3e8] p-3 first:border-t-0">
      <div className="grid size-8 shrink-0 place-items-center rounded-md bg-[#eeeeef] text-[#5f6067]">{icon}</div>
      <div className="min-w-0 flex-1">
        <p className="truncate text-[13px] font-semibold text-[#25252a]">{title}</p>
        <p className="truncate text-[12px] text-[#74747b]">{detail}</p>
      </div>
      {trailing ? <div className="shrink-0">{trailing}</div> : null}
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
    blue: "bg-white/58 text-[#0066cc] border-[rgba(0,102,204,0.24)]",
    green: "bg-[#e8f6ee] text-[#247a48] border-[#c8ead5]",
    gray: "bg-[#f1f1f4] text-[#65656d] border-[#dadbe2]",
    red: "bg-[#fff0ef] text-[#b3261e] border-[#ffd1ce]",
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
      className="inline-flex h-10 items-center justify-center gap-2 rounded-full border border-[#0066cc] bg-[#0066cc] px-[22px] text-[15px] font-normal tracking-[-0.224px] text-white shadow-none transition active:scale-[0.95] disabled:cursor-not-allowed disabled:border-[#b8b8bf] disabled:bg-[#d2d2d7] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#0071e3]"
      onClick={onClick}
    >
      {children}
    </button>
  );
}

interface SecondaryButtonProps {
  children: ReactNode;
  onClick: () => void;
}

function SecondaryButton({ children, onClick }: SecondaryButtonProps) {
  return (
    <button
      type="button"
      className="inline-flex h-10 items-center justify-center gap-2 rounded-full border border-[#0066cc] bg-transparent px-[22px] text-[15px] font-normal tracking-[-0.224px] text-[#0066cc] shadow-none transition active:scale-[0.95] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#0071e3]"
      onClick={onClick}
    >
      {children}
    </button>
  );
}

export default App;
