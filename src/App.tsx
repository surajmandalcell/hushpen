import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from "react";
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

type ViewId = "dashboard" | "files" | "models" | "history" | "settings";
type WindowAction = "minimize" | "maximize" | "close";
type OverlayPlacement = "top" | "bottom";

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

const navItems: NavItem[] = [
  { id: "dashboard", label: "Dashboard", icon: Activity },
  { id: "files", label: "Files", icon: FileAudio2 },
  { id: "models", label: "Models", icon: BrainCircuit },
  { id: "history", label: "History", icon: History },
  { id: "settings", label: "Settings", icon: Settings },
];

const defaultModelName = "Parakeet-TDT-0.6B-v3";

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

const transcriptRows = [
  {
    title: "Design review notes",
    kind: "Dictation",
    model: defaultModelName,
    duration: "06:12",
    time: "4 min ago",
  },
  {
    title: "Product demo call",
    kind: "File",
    model: defaultModelName,
    duration: "28:44",
    time: "Yesterday",
  },
  {
    title: "Lecture excerpt",
    kind: "SRT export",
    model: "Whisper Base",
    duration: "14:09",
    time: "May 10",
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

function calculateVoiceLevel(samples: Uint8Array) {
  let sum = 0;
  for (let index = 0; index < samples.length; index += 1) {
    const centered = (samples[index] - 128) / 128;
    sum += centered * centered;
  }

  const rms = Math.sqrt(sum / samples.length);
  return clampNumber((rms - 0.016) / 0.13, 0, 1);
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

function useMicrophoneWaveform(active: boolean) {
  const [frame, setFrame] = useState<number[]>(idleWaveformFrame);
  const frameRef = useRef<number[]>(idleWaveformFrame);
  const lastOverlayFrameAtRef = useRef(0);

  useEffect(() => {
    if (!active) {
      frameRef.current = idleWaveformFrame;
      setFrame(idleWaveformFrame);
      sendOverlayWaveformFrame(idleWaveformFrame, false);
      return undefined;
    }

    let stopped = false;
    let animationFrame = 0;
    let stream: MediaStream | null = null;
    let audioContext: AudioContext | null = null;

    const startAnalyser = async () => {
      try {
        if (!navigator.mediaDevices?.getUserMedia || !window.AudioContext) {
          frameRef.current = idleWaveformFrame;
          setFrame(idleWaveformFrame);
          sendOverlayWaveformFrame(idleWaveformFrame, false);
          return;
        }

        stream = await navigator.mediaDevices.getUserMedia({
          audio: {
            channelCount: 1,
            echoCancellation: true,
            noiseSuppression: true,
          },
        });

        if (stopped) {
          stream.getTracks().forEach((track) => track.stop());
          return;
        }

        audioContext = new AudioContext();
        await audioContext.resume().catch(() => {});
        if (stopped) return;

        const analyser = audioContext.createAnalyser();
        analyser.fftSize = 1024;
        analyser.smoothingTimeConstant = 0.72;

        const source = audioContext.createMediaStreamSource(stream);
        source.connect(analyser);

        const timeDomainSamples = new Uint8Array(analyser.fftSize);
        const frequencySamples = new Uint8Array(analyser.frequencyBinCount);

        const tick = (timestamp: number) => {
          if (stopped) return;

          analyser.getByteTimeDomainData(timeDomainSamples);
          const voiceLevel = calculateVoiceLevel(timeDomainSamples);

          if (voiceLevel <= 0.025) {
            if (frameRef.current !== idleWaveformFrame) {
              frameRef.current = idleWaveformFrame;
              setFrame(idleWaveformFrame);
            }

            if (timestamp - lastOverlayFrameAtRef.current > 120) {
              sendOverlayWaveformFrame(idleWaveformFrame, false);
              lastOverlayFrameAtRef.current = timestamp;
            }
          } else {
            analyser.getByteFrequencyData(frequencySamples);
            const nextFrame = buildReactiveWaveformFrame(frequencySamples, voiceLevel, timestamp, frameRef.current);

            frameRef.current = nextFrame;
            setFrame(nextFrame);

            if (timestamp - lastOverlayFrameAtRef.current > 16) {
              sendOverlayWaveformFrame(nextFrame, true);
              lastOverlayFrameAtRef.current = timestamp;
            }
          }

          animationFrame = requestAnimationFrame(tick);
        };

        animationFrame = requestAnimationFrame(tick);
      } catch {
        if (stopped) return;
        frameRef.current = idleWaveformFrame;
        setFrame(idleWaveformFrame);
        sendOverlayWaveformFrame(idleWaveformFrame, false);
      }
    };

    void startAnalyser();

    return () => {
      stopped = true;
      if (animationFrame) cancelAnimationFrame(animationFrame);
      stream?.getTracks().forEach((track) => track.stop());
      void audioContext?.close();
      frameRef.current = idleWaveformFrame;
      sendOverlayWaveformFrame(idleWaveformFrame, false);
    };
  }, [active]);

  return frame;
}

function App() {
  const [activeView, setActiveView] = useState<ViewId>("dashboard");
  const [isRecording, setIsRecording] = useState(false);
  const [selectedModel, setSelectedModel] = useState(defaultModelName);
  const [runtimeInfo, setRuntimeInfo] = useState<RuntimeInfo | null>(null);
  const [overlayPlacement, setOverlayPlacement] = useState<OverlayPlacement>("top");
  const [queuedFiles, setQueuedFiles] = useState<AudioFile[]>([
    { fileName: "board-meeting.wav", path: "demo://board-meeting.wav" },
    { fileName: "voice-note.m4a", path: "demo://voice-note.m4a" },
  ]);
  const [historyFilter, setHistoryFilter] = useState("All");
  const waveformFrame = useMicrophoneWaveform(isRecording);

  useEffect(() => {
    const api = window.asrpro;
    if (!api) return undefined;

    if (api.getRuntimeState) {
      Promise.resolve(api.getRuntimeState()).then((state) => {
        if (!state) return;
        setRuntimeInfo(state);
        setIsRecording(state.isRecording);
        setSelectedModel(state.defaultModel || defaultModelName);
        setOverlayPlacement(normalizeOverlayPlacement(state.overlaySettings?.placement));
      }).catch(() => {});
    }

    return api.onRecordingState?.((state) => {
      setIsRecording(state.isRecording);
      setRuntimeInfo((current) => (current ? { ...current, isRecording: state.isRecording } : current));
    });
  }, []);

  const activeTitle = useMemo(() => navItems.find((item) => item.id === activeView)?.label ?? "Dashboard", [activeView]);

  const handleWindowAction = (action: WindowAction) => {
    void window.asrpro?.windowControl(action);
  };

  const handleSetRecording = useCallback((active: boolean) => {
    const api = window.asrpro;
    if (!api?.setRecording) {
      setIsRecording(active);
      setRuntimeInfo((current) => (current ? { ...current, isRecording: active } : current));
      return;
    }

    api.setRecording(active).then((state) => {
      setIsRecording(state.isRecording);
      setRuntimeInfo((current) => (current ? { ...current, isRecording: state.isRecording } : current));
    }).catch(() => {
      setIsRecording(active);
      setRuntimeInfo((current) => (current ? { ...current, isRecording: active } : current));
    });
  }, []);

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
          <Toolbar activeTitle={activeTitle} isRecording={isRecording} onToggleRecording={() => handleSetRecording(!isRecording)} />
          <main className="min-h-0 min-w-0 overflow-y-auto px-4 pb-6 pt-3 sm:px-6 lg:px-8">
            {activeView === "dashboard" && (
              <DashboardView
                isRecording={isRecording}
                waveformFrame={waveformFrame}
                selectedModel={selectedModel}
                onToggleRecording={() => handleSetRecording(!isRecording)}
                onSelectFiles={handleSelectFiles}
              />
            )}
            {activeView === "files" && <FilesView queuedFiles={queuedFiles} onSelectFiles={handleSelectFiles} />}
            {activeView === "models" && <ModelsView selectedModel={selectedModel} onSelectModel={setSelectedModel} />}
            {activeView === "history" && <HistoryView activeFilter={historyFilter} onFilterChange={setHistoryFilter} />}
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
  onToggleRecording: () => void;
}

function Toolbar({ activeTitle, isRecording, onToggleRecording }: ToolbarProps) {
  return (
    <header className="flex min-w-0 items-center justify-between bg-[#f5f5f7]/62 px-4 backdrop-blur-2xl [-webkit-app-region:drag] sm:px-6 lg:px-8">
      <h1 className="truncate text-[14px] font-semibold leading-none text-[#2c2c31]">{activeTitle}</h1>
      <button
        type="button"
        aria-label={isRecording ? "Stop Recording from toolbar" : "Start Recording from toolbar"}
        onClick={onToggleRecording}
        className="inline-flex h-8 items-center gap-2 rounded-full border border-[rgba(0,0,0,0.08)] bg-white/58 px-3.5 text-[13px] font-normal tracking-[-0.12px] text-[#0066cc] backdrop-blur-2xl transition active:scale-[0.95] [-webkit-app-region:no-drag]"
      >
        {isRecording ? <CircleStop className="size-4" /> : <Mic2 className="size-4" />}
        <span className="hidden sm:inline">{isRecording ? "Stop Recording" : "Start Recording"}</span>
      </button>
    </header>
  );
}

interface DashboardViewProps {
  isRecording: boolean;
  waveformFrame: number[];
  selectedModel: string;
  onToggleRecording: () => void;
  onSelectFiles: () => void;
}

function DashboardView({ isRecording, waveformFrame, selectedModel, onToggleRecording, onSelectFiles }: DashboardViewProps) {
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
            <p className="truncate text-[12px] leading-4 text-[#6e6e73]">{selectedModel}</p>
          </div>
        </div>
        <span
          className={`size-2.5 shrink-0 rounded-full ${isRecording ? "bg-[#ff453a] shadow-[0_0_0_4px_rgba(255,69,58,0.12)]" : "bg-[#b8b8bf]"}`}
          aria-hidden="true"
        />
      </div>

      <Waveform active={isRecording} frame={waveformFrame} />

      <div className="flex flex-col items-start gap-2 sm:flex-row">
        <PrimaryButton onClick={onToggleRecording}>
          {isRecording ? <CircleStop className="size-4" /> : <Mic2 className="size-4" />}
          {isRecording ? "Stop Recording" : "Start Recording"}
        </PrimaryButton>
        <SecondaryButton onClick={onSelectFiles}>
          <FolderUp className="size-4" />
          Add Files
        </SecondaryButton>
      </div>
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
  activeFilter: string;
  onFilterChange: (filter: string) => void;
}

function HistoryView({ activeFilter, onFilterChange }: HistoryViewProps) {
  const filters = ["All", "Dictation", "Files", "Exports"];

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

      <GroupedPanel>
        {transcriptRows.map((row) => (
          <PanelRow
            key={row.title}
            icon={<History className="size-4" />}
            title={row.title}
            detail={`${row.kind} - ${row.duration} - ${row.model}`}
            trailing={<span className="text-[12px] font-medium text-[#77777f]">{row.time}</span>}
          />
        ))}
      </GroupedPanel>
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
        <PanelRow icon={<Activity className="size-4" />} title="Tray and overlay" detail="Close hides to tray; recording shows a draggable floating pill" trailing={<StatusPill tone="green">On</StatusPill>} />
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
  frame: number[];
}

function Waveform({ active, frame }: WaveformProps) {
  return (
    <div className={`in-app-waveform ${active ? "is-active" : ""}`} role="img" aria-label="Live recording waveform">
      <div className="in-app-waveform__bars" aria-hidden="true">
        {waveformBaseBars.map((bar, index) => (
          <span
            key={bar.id}
            style={{
              "--wave-height": `${frame[index] ?? bar.baseHeight}px`,
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
}

function PrimaryButton({ children, onClick }: PrimaryButtonProps) {
  return (
    <button
      type="button"
      className="inline-flex h-10 items-center justify-center gap-2 rounded-full border border-[#0066cc] bg-[#0066cc] px-[22px] text-[15px] font-normal tracking-[-0.224px] text-white shadow-none transition active:scale-[0.95] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#0071e3]"
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
