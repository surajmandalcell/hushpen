import { useCallback, useEffect, useMemo, useState, type CSSProperties, type ReactNode } from "react";
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

const waveformBars = [
  { id: "lead-low", height: 30, delay: 0 },
  { id: "lead-mid", height: 58, delay: 38 },
  { id: "lead-high", height: 84, delay: 76 },
  { id: "soft-dip", height: 42, delay: 114 },
  { id: "peak-one", height: 96, delay: 152 },
  { id: "mid-one", height: 64, delay: 190 },
  { id: "lift-one", height: 78, delay: 228 },
  { id: "quiet-one", height: 36, delay: 266 },
  { id: "peak-two", height: 88, delay: 304 },
  { id: "mid-two", height: 54, delay: 342 },
  { id: "lift-two", height: 72, delay: 380 },
  { id: "quiet-two", height: 46, delay: 418 },
  { id: "peak-three", height: 92, delay: 456 },
  { id: "mid-three", height: 62, delay: 494 },
  { id: "quiet-three", height: 38, delay: 532 },
  { id: "lift-three", height: 70, delay: 570 },
  { id: "peak-four", height: 98, delay: 608 },
  { id: "mid-four", height: 52, delay: 646 },
  { id: "lift-four", height: 82, delay: 684 },
  { id: "soft-four", height: 44, delay: 722 },
  { id: "mid-five", height: 66, delay: 760 },
  { id: "tail-low", height: 34, delay: 798 },
  { id: "tail-high", height: 76, delay: 836 },
  { id: "tail-mid", height: 56, delay: 874 },
] as const;

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
    <div className="h-screen w-screen overflow-hidden bg-[#e8e8ed] font-[Inter,-apple-system,BlinkMacSystemFont,'Segoe_UI',sans-serif] text-[#1d1d1f] antialiased">
      <div className="grid h-full grid-cols-1 grid-rows-[auto_minmax(0,1fr)] md:grid-cols-[232px_minmax(0,1fr)] md:grid-rows-1">
        <Sidebar activeView={activeView} onChange={setActiveView} onWindowAction={handleWindowAction} />
        <section className="grid min-h-0 min-w-0 grid-rows-[52px_minmax(0,1fr)] bg-[#f5f5f7] md:rounded-tl-[10px] md:border-l md:border-[#d6d6dc]">
          <Toolbar activeTitle={activeTitle} isRecording={isRecording} onToggleRecording={() => handleSetRecording(!isRecording)} />
          <main className="min-h-0 min-w-0 overflow-y-auto px-4 pb-5 pt-4 sm:px-6 lg:px-8">
            {activeView === "dashboard" && (
              <DashboardView
                isRecording={isRecording}
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
    <aside className="flex min-h-0 flex-col border-b border-[#d5d5dc] bg-white/58 backdrop-blur-2xl md:border-b-0">
      <div className="flex h-[52px] items-center gap-3 px-4 [-webkit-app-region:drag]">
        <WindowDots onWindowAction={onWindowAction} />
        <p className="truncate text-[13px] font-semibold leading-tight text-[#242429]">ASR Pro</p>
      </div>

      <nav className="flex gap-1 overflow-x-auto px-3 pb-3 pt-1 md:block md:min-h-0 md:overflow-y-auto" aria-label="Primary">
        {navItems.map((item) => {
          const Icon = item.icon;
          const isActive = activeView === item.id;

          return (
            <button
              key={item.id}
              type="button"
              aria-label={item.label}
              className={`mb-1 flex h-8 shrink-0 items-center gap-2 rounded-md px-2.5 text-left text-[13px] font-medium transition md:w-full ${
                isActive
                  ? "bg-[#dfe7f5] text-[#0c4a9b] shadow-[inset_0_0_0_1px_rgba(40,94,160,0.08)]"
                  : "text-[#4d4d55] hover:bg-white/68"
              }`}
              onClick={() => onChange(item.id)}
            >
              <Icon className="h-4 w-4 shrink-0" />
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
        className="h-3 w-3 rounded-full border border-[#e14640] bg-[#ff5f57]"
        type="button"
        onClick={() => onWindowAction("close")}
      />
      <button
        aria-label="Minimize window"
        className="h-3 w-3 rounded-full border border-[#dfa023] bg-[#febc2e]"
        type="button"
        onClick={() => onWindowAction("minimize")}
      />
      <button
        aria-label="Maximize window"
        className="h-3 w-3 rounded-full border border-[#18a433] bg-[#28c840]"
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
    <header className="flex min-w-0 items-center justify-between border-b border-[#d7d7dd] bg-[#f7f7f9]/90 px-4 backdrop-blur-xl [-webkit-app-region:drag] sm:px-6 lg:px-8">
      <h1 className="truncate text-[15px] font-semibold leading-none text-[#202024]">{activeTitle}</h1>
      <button
        type="button"
        aria-label={isRecording ? "Stop Recording from toolbar" : "Start Recording from toolbar"}
        onClick={onToggleRecording}
        className={`inline-flex h-8 items-center gap-2 rounded-md px-3 text-[13px] font-semibold shadow-[0_1px_0_rgba(255,255,255,0.35)] transition [-webkit-app-region:no-drag] ${
          isRecording ? "bg-[#ff453a] text-white hover:bg-[#ea352b]" : "bg-[#0a84ff] text-white hover:bg-[#0071e3]"
        }`}
      >
        {isRecording ? <CircleStop className="h-4 w-4" /> : <Mic2 className="h-4 w-4" />}
        <span className="hidden sm:inline">{isRecording ? "Stop Recording" : "Start Recording"}</span>
      </button>
    </header>
  );
}

interface DashboardViewProps {
  isRecording: boolean;
  selectedModel: string;
  onToggleRecording: () => void;
  onSelectFiles: () => void;
}

function DashboardView({ isRecording, selectedModel, onToggleRecording, onSelectFiles }: DashboardViewProps) {
  return (
    <ViewFrame eyebrow={isRecording ? "Recording now" : "Ready"} title="Ready to Dictate" description="Start recording or add files for local transcription.">
      <GroupedPanel>
        <PanelRow
          icon={<Mic2 className="h-4 w-4" />}
          title="Live dictation"
          detail={isRecording ? "Recording now" : `Using ${selectedModel}`}
          trailing={<StatusPill tone={isRecording ? "red" : "green"}>{isRecording ? "Recording" : "Ready"}</StatusPill>}
        />
        <div className="border-t border-[#e2e2e7] p-4">
          <Waveform active={isRecording} />
          <div className="mt-4 flex flex-col gap-2 sm:flex-row">
            <PrimaryButton onClick={onToggleRecording} tone={isRecording ? "red" : "blue"}>
              {isRecording ? <CircleStop className="h-4 w-4" /> : <Mic2 className="h-4 w-4" />}
              {isRecording ? "Stop Recording" : "Start Recording"}
            </PrimaryButton>
            <SecondaryButton onClick={onSelectFiles}>
              <FolderUp className="h-4 w-4" />
              Add Files
            </SecondaryButton>
          </div>
        </div>
      </GroupedPanel>
    </ViewFrame>
  );
}

interface FilesViewProps {
  queuedFiles: AudioFile[];
  onSelectFiles: () => void;
}

function FilesView({ queuedFiles, onSelectFiles }: FilesViewProps) {
  return (
    <ViewFrame eyebrow="Files" title="Drop audio or video" description="Queue recordings, interviews, and screen captures for transcription.">
      <div className="rounded-lg border border-dashed border-[#bfc0c7] bg-white/72 p-6 text-center shadow-[0_1px_2px_rgba(0,0,0,0.03)]">
        <div className="mx-auto grid h-12 w-12 place-items-center rounded-lg bg-[#e7f0ff] text-[#0a64c9]">
          <UploadCloud className="h-6 w-6" />
        </div>
        <p className="mt-3 text-[18px] font-semibold text-[#202024]">Drop files here</p>
        <p className="mx-auto mt-1 max-w-md text-[13px] leading-5 text-[#686870]">Choose files from the desktop or drag them onto this window.</p>
        <div className="mt-4 flex justify-center">
          <PrimaryButton onClick={onSelectFiles}>
            <FolderUp className="h-4 w-4" />
            Choose Files
          </PrimaryButton>
        </div>
      </div>

      <GroupedPanel title="Queued transcription jobs">
        {queuedFiles.map((file, index) => (
          <PanelRow
            key={`${file.path}-${index}`}
            icon={<FileAudio2 className="h-4 w-4" />}
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
    <ViewFrame eyebrow="Models" title="Speech models" description="Choose the local engine used for dictation and file transcription.">
      <GroupedPanel>
        {modelCards.map((model) => (
          <button
            key={model.name}
            type="button"
            className="flex w-full items-center gap-3 border-t border-[#e2e2e7] p-4 text-left first:border-t-0 hover:bg-[#f7f7f9]"
            onClick={() => onSelectModel(model.name)}
          >
            <div className="grid h-8 w-8 shrink-0 place-items-center rounded-md bg-[#eef3fb] text-[#0a64c9]">
              <BrainCircuit className="h-4 w-4" />
            </div>
            <div className="min-w-0 flex-1">
              <p className="truncate text-[13px] font-semibold text-[#25252a]">{model.name}</p>
              <p className="truncate text-[12px] text-[#74747b]">{model.detail}</p>
            </div>
            <div className="hidden text-right text-[12px] text-[#77777f] sm:block">
              <p>{model.speed}</p>
              <p>{model.status}</p>
            </div>
            {selectedModel === model.name ? <CheckCircle2 className="h-4 w-4 text-[#30a46c]" /> : null}
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
    <ViewFrame eyebrow="History" title="Transcript library" description="Review previous sessions, source files, models, and export status.">
      <div className="inline-flex rounded-lg border border-[#d4d4da] bg-white/70 p-1">
        {filters.map((filter) => (
          <button
            key={filter}
            type="button"
            className={`rounded-md px-3 py-1.5 text-[13px] font-medium transition ${
              activeFilter === filter ? "bg-white text-[#1d1d1f] shadow-[0_1px_2px_rgba(0,0,0,0.08)]" : "text-[#696970] hover:text-[#1d1d1f]"
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
            icon={<History className="h-4 w-4" />}
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
    <ViewFrame eyebrow="Settings" title="Desktop behavior" description="Actual app behavior, storage, and integration state.">
      <GroupedPanel title="Storage">
        <PanelRow icon={<Database className="h-4 w-4" />} title="Data folder" detail={runtimeInfo?.dataDir ?? "App-contained data directory"} />
        <PanelRow icon={<BrainCircuit className="h-4 w-4" />} title="Default model" detail={runtimeInfo?.defaultModelRepo ?? selectedModel} trailing={<StatusPill tone="green">Active</StatusPill>} />
      </GroupedPanel>

      <GroupedPanel title="Desktop integration">
        <PanelRow icon={<Layers2 className="h-4 w-4" />} title="Single instance" detail="Launching again focuses the running app" trailing={<StatusPill tone="green">On</StatusPill>} />
        <PanelRow icon={<Activity className="h-4 w-4" />} title="Tray and overlay" detail="Close hides to tray; recording shows a draggable floating pill" trailing={<StatusPill tone="green">On</StatusPill>} />
        <PanelRow
          icon={<Settings className="h-4 w-4" />}
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
    <div className="inline-flex rounded-lg border border-[#d4d4da] bg-[#f2f2f5] p-0.5">
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
              active ? "bg-[#0a64c9] text-white shadow-[0_1px_2px_rgba(0,0,0,0.12)]" : "text-[#67676f] hover:text-[#1d1d1f]"
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
  eyebrow: string;
  title: string;
  description: string;
  children: ReactNode;
}

function ViewFrame({ eyebrow, title, description, children }: ViewFrameProps) {
  return (
    <section className="mx-auto w-full max-w-4xl space-y-4">
      <div>
        <p className="text-[12px] font-semibold uppercase tracking-[0.04em] text-[#74747b]">{eyebrow}</p>
        <h2 className="mt-1 text-[24px] font-semibold tracking-normal text-[#1d1d1f] sm:text-[28px]">{title}</h2>
        <p className="mt-1 max-w-2xl text-[13px] leading-5 text-[#67676f]">{description}</p>
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
      {title ? <h3 className="mb-2 px-1 text-[12px] font-semibold uppercase tracking-[0.04em] text-[#77777f]">{title}</h3> : null}
      <div className="overflow-hidden rounded-lg border border-[#d9d9df] bg-white/82 shadow-[0_1px_2px_rgba(0,0,0,0.045)]">{children}</div>
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
    <div className="flex min-w-0 items-center gap-3 border-t border-[#e2e2e7] p-4 first:border-t-0">
      <div className="grid h-8 w-8 shrink-0 place-items-center rounded-md bg-[#eef3fb] text-[#0a64c9]">{icon}</div>
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
  return (
    <div className={`in-app-waveform ${active ? "is-active" : ""}`} role="img" aria-label="Live recording waveform">
      <div className="in-app-waveform__header">
        <span className="in-app-waveform__state">{active ? "Listening" : "Ready"}</span>
        <span className="in-app-waveform__meter">{active ? "Live input" : "Input idle"}</span>
      </div>
      <div className="in-app-waveform__bars" aria-hidden="true">
        {waveformBars.map((bar) => (
          <span
            key={bar.id}
            style={{
              "--wave-height": `${bar.height}px`,
              "--wave-delay": `${bar.delay}ms`,
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
    blue: "bg-[#e7f0ff] text-[#0759b8] border-[#bfd7ff]",
    green: "bg-[#e8f6ee] text-[#247a48] border-[#c8ead5]",
    gray: "bg-[#f1f1f4] text-[#65656d] border-[#dadbe2]",
    red: "bg-[#fff0ef] text-[#b3261e] border-[#ffd1ce]",
  };

  return <span className={`inline-flex rounded-full border px-2 py-0.5 text-[11px] font-semibold ${tones[tone]}`}>{children}</span>;
}

interface PrimaryButtonProps {
  children: ReactNode;
  tone?: "blue" | "red";
  onClick: () => void;
}

function PrimaryButton({ children, tone = "blue", onClick }: PrimaryButtonProps) {
  return (
    <button
      type="button"
      className={`inline-flex h-9 items-center justify-center gap-2 rounded-md px-4 text-[13px] font-semibold text-white shadow-[0_1px_0_rgba(255,255,255,0.35)] transition ${
        tone === "red" ? "bg-[#ff453a] hover:bg-[#ea352b]" : "bg-[#0a84ff] hover:bg-[#0071e3]"
      }`}
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
      className="inline-flex h-9 items-center justify-center gap-2 rounded-md border border-[#d2d2d8] bg-white/82 px-4 text-[13px] font-semibold text-[#34343a] shadow-[0_1px_0_rgba(255,255,255,0.9)] transition hover:bg-white"
      onClick={onClick}
    >
      {children}
    </button>
  );
}

export default App;
