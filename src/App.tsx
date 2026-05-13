import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import {
  Activity,
  AudioLines,
  BrainCircuit,
  CheckCircle2,
  ChevronRight,
  CircleStop,
  Clock3,
  Download,
  FileAudio2,
  FolderUp,
  History,
  Keyboard,
  Mic2,
  Play,
  Search,
  Settings,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  UploadCloud,
  type LucideIcon,
} from "lucide-react";

type ViewId = "dashboard" | "dictate" | "files" | "models" | "history" | "shortcuts" | "settings";
type WindowAction = "minimize" | "maximize" | "close";

interface PlatformInfo {
  platform: string;
  arch: string;
  versions: {
    electron?: string;
    chrome?: string;
    node?: string;
  };
}

interface AudioFile {
  fileName: string;
  path: string;
}

interface NavItem {
  id: ViewId;
  label: string;
  icon: LucideIcon;
}

const navItems: NavItem[] = [
  { id: "dashboard", label: "Dashboard", icon: Activity },
  { id: "dictate", label: "Dictate", icon: Mic2 },
  { id: "files", label: "Files", icon: FileAudio2 },
  { id: "models", label: "Models", icon: BrainCircuit },
  { id: "history", label: "History", icon: History },
  { id: "shortcuts", label: "Shortcuts", icon: Keyboard },
  { id: "settings", label: "Settings", icon: Settings },
];

const modelCards = [
  {
    name: "Local Whisper",
    detail: "Whisper Base, offline, private",
    speed: "1.8x realtime",
    accuracy: "94%",
    status: "Active",
  },
  {
    name: "Parakeet TDT",
    detail: "Fast meeting notes and clean punctuation",
    speed: "3.2x realtime",
    accuracy: "92%",
    status: "Ready",
  },
  {
    name: "Whisper Large V3 Turbo",
    detail: "High accuracy for noisy files",
    speed: "0.9x realtime",
    accuracy: "97%",
    status: "Download",
  },
];

const transcriptRows = [
  {
    title: "Design review notes",
    kind: "Dictation",
    model: "Local Whisper",
    duration: "06:12",
    time: "4 min ago",
  },
  {
    title: "Product demo call",
    kind: "File",
    model: "Parakeet TDT",
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

const shortcutRows = [
  { action: "Start or stop dictation", keys: ["Ctrl", "Space"], scope: "Global" },
  { action: "Push-to-talk", keys: ["Alt", "Space"], scope: "Focused app" },
  { action: "Transcribe clipboard media", keys: ["Ctrl", "Shift", "V"], scope: "Global" },
];

const activityRows = [
  { title: "Input ready", detail: "Built-in microphone calibrated", value: "44.1 kHz" },
  { title: "Privacy mode", detail: "Local-only transcription enabled", value: "On" },
  { title: "Output", detail: "Copy clean text after each session", value: "Auto" },
];

const formatPlatform = (platform?: string) => {
  if (platform === "darwin") return "macOS";
  if (platform === "win32") return "Windows";
  if (platform === "linux") return "Linux";
  return "Desktop";
};

function App() {
  const [activeView, setActiveView] = useState<ViewId>("dashboard");
  const [isRecording, setIsRecording] = useState(false);
  const [selectedModel, setSelectedModel] = useState("Local Whisper");
  const [platformInfo, setPlatformInfo] = useState<PlatformInfo | null>(null);
  const [queuedFiles, setQueuedFiles] = useState<AudioFile[]>([
    { fileName: "board-meeting.wav", path: "demo://board-meeting.wav" },
    { fileName: "voice-note.m4a", path: "demo://voice-note.m4a" },
  ]);
  const [historyFilter, setHistoryFilter] = useState("All");

  useEffect(() => {
    window.asrpro?.getPlatform().then(setPlatformInfo).catch(() => {
      setPlatformInfo(null);
    });
  }, []);

  const activeTitle = useMemo(() => navItems.find((item) => item.id === activeView)?.label ?? "Dashboard", [activeView]);

  const handleWindowAction = (action: WindowAction) => {
    void window.asrpro?.windowControl(action);
  };

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

  useEffect(() => window.asrpro?.onAddFiles?.(() => {
    void handleSelectFiles();
  }), [handleSelectFiles]);

  return (
    <div className="h-screen w-screen overflow-hidden bg-[#e8e8ed] font-[Inter,-apple-system,BlinkMacSystemFont,'Segoe_UI',sans-serif] text-[#1d1d1f] antialiased">
      <div className="grid h-full grid-cols-1 grid-rows-[auto_minmax(0,1fr)] md:grid-cols-[244px_minmax(0,1fr)] md:grid-rows-1">
        <Sidebar
          activeView={activeView}
          onChange={setActiveView}
          onWindowAction={handleWindowAction}
          selectedModel={selectedModel}
        />
        <section className="grid min-h-0 min-w-0 grid-rows-[52px_minmax(0,1fr)] bg-[#f4f4f6] md:rounded-tl-[10px] md:border-l md:border-[#d6d6dc]">
          <Toolbar activeTitle={activeTitle} isRecording={isRecording} onToggleRecording={() => setIsRecording((current) => !current)} />
          <main className="min-h-0 min-w-0 overflow-y-auto px-4 pb-5 pt-4 sm:px-6 lg:px-8">
            {activeView === "dashboard" && (
              <DashboardView
                isRecording={isRecording}
                selectedModel={selectedModel}
                queuedCount={queuedFiles.length}
                onToggleRecording={() => setIsRecording((current) => !current)}
                onSelectFiles={handleSelectFiles}
              />
            )}
            {activeView === "dictate" && (
              <DictateView
                isRecording={isRecording}
                selectedModel={selectedModel}
                onToggleRecording={() => setIsRecording((current) => !current)}
              />
            )}
            {activeView === "files" && <FilesView queuedFiles={queuedFiles} onSelectFiles={handleSelectFiles} />}
            {activeView === "models" && <ModelsView selectedModel={selectedModel} onSelectModel={setSelectedModel} />}
            {activeView === "history" && <HistoryView activeFilter={historyFilter} onFilterChange={setHistoryFilter} />}
            {activeView === "shortcuts" && <ShortcutsView />}
            {activeView === "settings" && <SettingsView platformInfo={platformInfo} />}
          </main>
        </section>
      </div>
    </div>
  );
}

interface SidebarProps {
  activeView: ViewId;
  selectedModel: string;
  onChange: (view: ViewId) => void;
  onWindowAction: (action: WindowAction) => void;
}

function Sidebar({ activeView, selectedModel, onChange, onWindowAction }: SidebarProps) {
  return (
    <aside className="flex min-h-0 flex-col border-b border-[#d5d5dc] bg-white/55 backdrop-blur-2xl md:border-b-0">
      <div className="flex h-[52px] items-center gap-3 px-4 [-webkit-app-region:drag]">
        <WindowDots onWindowAction={onWindowAction} />
        <div className="min-w-0">
          <p className="truncate text-[13px] font-semibold leading-tight text-[#242429]">ASR Pro</p>
        </div>
      </div>

      <div className="px-3 pb-3">
        <label className="flex h-8 items-center gap-2 rounded-md border border-[#d4d4da] bg-white/65 px-2.5 text-[12px] text-[#74747b] shadow-[0_1px_0_rgba(255,255,255,0.85)]">
          <Search className="h-3.5 w-3.5 shrink-0" />
          <span className="truncate">Search transcripts</span>
        </label>
      </div>

      <nav className="flex gap-1 overflow-x-auto px-3 pb-3 md:block md:min-h-0 md:overflow-y-auto" aria-label="Primary">
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
                  : "text-[#4d4d55] hover:bg-white/65"
              }`}
              onClick={() => onChange(item.id)}
            >
              <Icon className="h-4 w-4 shrink-0" />
              <span>{item.label}</span>
            </button>
          );
        })}
      </nav>

      <div className="mt-auto hidden px-3 pb-4 md:block">
        <div className="rounded-lg border border-[#d2d2d8] bg-white/62 p-3 shadow-[0_1px_2px_rgba(0,0,0,0.04)]">
          <p className="text-[11px] font-semibold uppercase tracking-[0.04em] text-[#77777f]">Current model</p>
          <p className="mt-1 truncate text-[13px] font-semibold text-[#25252a]">{selectedModel}</p>
          <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-[#d9d9df]">
            <div className="h-full w-[74%] rounded-full bg-[#0a84ff]" />
          </div>
          <p className="mt-2 text-[11px] text-[#77777f]">Offline engine warmed and ready</p>
        </div>
      </div>
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
    <header className="flex min-w-0 items-center justify-between border-b border-[#d7d7dd] bg-[#f7f7f9]/88 px-4 backdrop-blur-xl [-webkit-app-region:drag] sm:px-6 lg:px-8">
      <div className="flex min-w-0 items-center gap-3">
        <h1 className="truncate text-[15px] font-semibold leading-none text-[#202024]">{activeTitle}</h1>
        <span className="hidden rounded-full border border-[#d3d3da] bg-white/75 px-2 py-0.5 text-[11px] font-medium text-[#696970] sm:inline-flex">
          Local first
        </span>
      </div>
      <div className="flex items-center gap-2 [-webkit-app-region:no-drag]">
        <button
          className="grid h-8 w-8 place-items-center rounded-md border border-[#d2d2d8] bg-white/72 text-[#4f4f56] shadow-[0_1px_0_rgba(255,255,255,0.9)] transition hover:bg-white"
          type="button"
          aria-label="Open controls"
        >
          <SlidersHorizontal className="h-4 w-4" />
        </button>
        <button
          type="button"
          aria-label={isRecording ? "Stop Recording from toolbar" : "Start Recording from toolbar"}
          onClick={onToggleRecording}
          className={`inline-flex h-8 items-center gap-2 rounded-md px-3 text-[13px] font-semibold shadow-[0_1px_0_rgba(255,255,255,0.35)] transition ${
            isRecording ? "bg-[#ff453a] text-white hover:bg-[#ea352b]" : "bg-[#0a84ff] text-white hover:bg-[#0071e3]"
          }`}
        >
          {isRecording ? <CircleStop className="h-4 w-4" /> : <Mic2 className="h-4 w-4" />}
          <span className="hidden sm:inline">{isRecording ? "Stop Recording" : "Start Recording"}</span>
        </button>
      </div>
    </header>
  );
}

interface DashboardViewProps {
  isRecording: boolean;
  selectedModel: string;
  queuedCount: number;
  onToggleRecording: () => void;
  onSelectFiles: () => void;
}

function DashboardView({ isRecording, selectedModel, queuedCount, onToggleRecording, onSelectFiles }: DashboardViewProps) {
  return (
    <ViewFrame
      eyebrow={isRecording ? "Recording now" : "Ready"}
      title="Ready to Dictate"
      description="Speak naturally, drop in media, and keep transcription private on this machine."
      side={<SessionInspector isRecording={isRecording} queuedCount={queuedCount} selectedModel={selectedModel} />}
    >
      <GroupedPanel>
        <PanelRow
          icon={<Mic2 className="h-4 w-4" />}
          title="Live dictation"
          detail={isRecording ? "Recording now" : "Idle and listening for your command"}
          trailing={<StatusPill tone={isRecording ? "red" : "green"}>{isRecording ? "Recording now" : "Ready"}</StatusPill>}
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

      <div className="grid gap-4 lg:grid-cols-3">
        {activityRows.map((row) => (
          <MetricTile key={row.title} title={row.title} detail={row.detail} value={row.value} />
        ))}
      </div>

      <GroupedPanel title="Recent transcripts">
        {transcriptRows.map((row) => (
          <PanelRow
            key={row.title}
            icon={<FileAudio2 className="h-4 w-4" />}
            title={row.title}
            detail={`${row.kind} - ${row.model}`}
            trailing={<span className="text-[12px] font-medium text-[#77777f]">{row.time}</span>}
          />
        ))}
      </GroupedPanel>
    </ViewFrame>
  );
}

interface DictateViewProps {
  isRecording: boolean;
  selectedModel: string;
  onToggleRecording: () => void;
}

function DictateView({ isRecording, selectedModel, onToggleRecording }: DictateViewProps) {
  return (
    <ViewFrame
      eyebrow={isRecording ? "Recording now" : "Dictation"}
      title="Live workspace"
      description="Capture clean speech with native desktop shortcuts and automatic text cleanup."
      side={<QuickPreferences selectedModel={selectedModel} />}
    >
      <GroupedPanel>
        <PanelRow
          icon={<AudioLines className="h-4 w-4" />}
          title="Microphone input"
          detail="Built-in microphone, noise reduction enabled"
          trailing={<StatusPill tone={isRecording ? "red" : "green"}>{isRecording ? "Recording now" : "Armed"}</StatusPill>}
        />
        <div className="border-t border-[#e2e2e7] p-4">
          <Waveform active={isRecording} large />
          <div className="mt-4 flex items-center justify-between gap-3 rounded-md bg-[#f5f5f7] px-3 py-2">
            <div>
              <p className="text-[13px] font-semibold text-[#25252a]">Transcript preview</p>
              <p className="text-[12px] text-[#73737a]">
                {isRecording ? "Listening for speech and preparing clean text." : "Start recording to preview recognized text here."}
              </p>
            </div>
            <PrimaryButton compact onClick={onToggleRecording} tone={isRecording ? "red" : "blue"}>
              {isRecording ? <CircleStop className="h-4 w-4" /> : <Play className="h-4 w-4" />}
              {isRecording ? "Stop Recording" : "Start Recording"}
            </PrimaryButton>
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
    <ViewFrame
      eyebrow="File transcription"
      title="Drop audio or video"
      description="Queue recordings, interviews, and screen captures for local transcription."
      side={<QueueSummary count={queuedFiles.length} />}
    >
      <div className="rounded-lg border border-dashed border-[#bfc0c7] bg-white/72 p-6 text-center shadow-[0_1px_2px_rgba(0,0,0,0.03)]">
        <div className="mx-auto grid h-12 w-12 place-items-center rounded-lg bg-[#e7f0ff] text-[#0a64c9]">
          <UploadCloud className="h-6 w-6" />
        </div>
        <p className="mt-3 text-[18px] font-semibold text-[#202024]">Drop files here</p>
        <p className="mx-auto mt-1 max-w-md text-[13px] leading-5 text-[#686870]">
          Choose files from Finder or drag them onto this window. ASR Pro keeps jobs local until you export them.
        </p>
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
    <ViewFrame
      eyebrow="Engines"
      title="Dictation models"
      description="Pick a local speech engine based on speed, accuracy, and file quality."
      side={<DownloadPanel />}
    >
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
              <p>{model.accuracy}</p>
            </div>
            {selectedModel === model.name ? <CheckCircle2 className="h-4 w-4 text-[#30a46c]" /> : <ChevronRight className="h-4 w-4 text-[#8a8a92]" />}
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
    <ViewFrame
      eyebrow="History"
      title="Transcript library"
      description="Review previous sessions, source files, models, and export status."
      side={<LibraryStats />}
    >
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

function ShortcutsView() {
  return (
    <ViewFrame
      eyebrow="Keyboard"
      title="Keyboard controls"
      description="Use global desktop shortcuts without changing the visual shell across platforms."
      side={<ShortcutNote />}
    >
      <GroupedPanel>
        {shortcutRows.map((row) => (
          <PanelRow
            key={row.action}
            icon={<Keyboard className="h-4 w-4" />}
            title={row.action}
            detail={row.scope}
            trailing={
              <div className="flex shrink-0 gap-1">
                {row.keys.map((key) => (
                  <kbd key={key} className="rounded border border-[#cdced6] bg-[#f5f5f7] px-1.5 py-0.5 text-[11px] font-semibold text-[#55555c]">
                    {key}
                  </kbd>
                ))}
              </div>
            }
          />
        ))}
      </GroupedPanel>
    </ViewFrame>
  );
}

interface SettingsViewProps {
  platformInfo: PlatformInfo | null;
}

function SettingsView({ platformInfo }: SettingsViewProps) {
  return (
    <ViewFrame
      eyebrow="Preferences"
      title="Settings"
      description="Native-feeling desktop defaults with privacy, model, and export controls in one place."
      side={<SystemPanel platformInfo={platformInfo} />}
    >
      <GroupedPanel title="General">
        <ToggleRow title="Launch at login" detail="Start ASR Pro with this desktop" enabled />
        <ToggleRow title="Keep transcripts local" detail="Disable remote processing by default" enabled />
        <ToggleRow title="Show menu bar helper" detail="Quick access to recording and recent items" enabled={false} />
      </GroupedPanel>

      <GroupedPanel title="Export">
        <PanelRow icon={<Download className="h-4 w-4" />} title="Default format" detail="Markdown and plain text" trailing={<ChevronRight className="h-4 w-4 text-[#8a8a92]" />} />
        <PanelRow icon={<ShieldCheck className="h-4 w-4" />} title="Redaction" detail="Detect sensitive numbers before export" trailing={<StatusPill tone="green">On</StatusPill>} />
      </GroupedPanel>
    </ViewFrame>
  );
}

interface ViewFrameProps {
  eyebrow: string;
  title: string;
  description: string;
  children: ReactNode;
  side?: ReactNode;
}

function ViewFrame({ eyebrow, title, description, children, side }: ViewFrameProps) {
  return (
    <div className="mx-auto grid w-full max-w-6xl gap-5 lg:grid-cols-[minmax(0,1fr)_280px]">
      <section className="min-w-0 space-y-4">
        <div>
          <p className="text-[12px] font-semibold uppercase tracking-[0.04em] text-[#74747b]">{eyebrow}</p>
          <h2 className="mt-1 text-[28px] font-semibold tracking-normal text-[#1d1d1f]">{title}</h2>
          <p className="mt-1 max-w-2xl text-[13px] leading-5 text-[#67676f]">{description}</p>
        </div>
        {children}
      </section>
      {side ? <aside className="min-w-0 space-y-4 lg:pt-[74px]">{side}</aside> : null}
    </div>
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

interface MetricTileProps {
  title: string;
  detail: string;
  value: string;
}

function MetricTile({ title, detail, value }: MetricTileProps) {
  return (
    <div className="rounded-lg border border-[#d9d9df] bg-white/78 p-4 shadow-[0_1px_2px_rgba(0,0,0,0.04)]">
      <p className="text-[12px] font-semibold text-[#74747b]">{title}</p>
      <p className="mt-2 text-[22px] font-semibold text-[#1d1d1f]">{value}</p>
      <p className="mt-1 text-[12px] leading-5 text-[#74747b]">{detail}</p>
    </div>
  );
}

interface WaveformProps {
  active: boolean;
  large?: boolean;
}

function Waveform({ active, large }: WaveformProps) {
  const bars = [28, 42, 64, 36, 76, 48, 54, 32, 70, 44, 60, 38, 66, 50, 30, 56, 72, 40];

  return (
    <div className={`flex items-center gap-1 rounded-lg border border-[#d9d9df] bg-[#f7f7f9] px-4 ${large ? "h-40" : "h-28"}`}>
      {bars.map((height, index) => (
        <div
          key={`${height}-${index}`}
          className={`flex-1 rounded-full transition-all duration-300 ${active ? "bg-[#0a84ff]" : "bg-[#c9cbd3]"}`}
          style={{ height: `${active ? height : Math.max(14, height * 0.42)}%` }}
        />
      ))}
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
  compact?: boolean;
  tone?: "blue" | "red";
  onClick: () => void;
}

function PrimaryButton({ children, compact, tone = "blue", onClick }: PrimaryButtonProps) {
  return (
    <button
      type="button"
      className={`inline-flex items-center justify-center gap-2 rounded-md text-[13px] font-semibold text-white shadow-[0_1px_0_rgba(255,255,255,0.35)] transition ${
        compact ? "h-8 px-3" : "h-9 px-4"
      } ${tone === "red" ? "bg-[#ff453a] hover:bg-[#ea352b]" : "bg-[#0a84ff] hover:bg-[#0071e3]"}`}
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

interface SessionInspectorProps {
  isRecording: boolean;
  selectedModel: string;
  queuedCount: number;
}

function SessionInspector({ isRecording, selectedModel, queuedCount }: SessionInspectorProps) {
  return (
    <GroupedPanel title="Session">
      <PanelRow icon={<Clock3 className="h-4 w-4" />} title="State" detail={isRecording ? "Recording now" : "Standing by"} trailing={<StatusPill tone={isRecording ? "red" : "green"}>{isRecording ? "Live" : "Ready"}</StatusPill>} />
      <PanelRow icon={<BrainCircuit className="h-4 w-4" />} title="Model" detail={selectedModel} trailing={<ChevronRight className="h-4 w-4 text-[#8a8a92]" />} />
      <PanelRow icon={<FileAudio2 className="h-4 w-4" />} title="Queue" detail={`${queuedCount} files ready`} trailing={<StatusPill tone="blue">{queuedCount}</StatusPill>} />
    </GroupedPanel>
  );
}

interface QuickPreferencesProps {
  selectedModel: string;
}

function QuickPreferences({ selectedModel }: QuickPreferencesProps) {
  return (
    <GroupedPanel title="Quick settings">
      <PanelRow icon={<BrainCircuit className="h-4 w-4" />} title="Engine" detail={selectedModel} trailing={<ChevronRight className="h-4 w-4 text-[#8a8a92]" />} />
      <PanelRow icon={<Sparkles className="h-4 w-4" />} title="Cleanup" detail="Punctuation and filler removal" trailing={<StatusPill tone="green">On</StatusPill>} />
      <PanelRow icon={<ShieldCheck className="h-4 w-4" />} title="Privacy" detail="Local processing only" trailing={<StatusPill tone="green">On</StatusPill>} />
    </GroupedPanel>
  );
}

interface QueueSummaryProps {
  count: number;
}

function QueueSummary({ count }: QueueSummaryProps) {
  return (
    <GroupedPanel title="Queue">
      <PanelRow icon={<FileAudio2 className="h-4 w-4" />} title="Queued files" detail="Ready for transcription" trailing={<StatusPill tone="blue">{count}</StatusPill>} />
      <PanelRow icon={<Clock3 className="h-4 w-4" />} title="Estimated time" detail="Local model estimate" trailing={<span className="text-[12px] font-semibold text-[#55555c]">03:42</span>} />
    </GroupedPanel>
  );
}

function DownloadPanel() {
  return (
    <GroupedPanel title="Storage">
      <PanelRow icon={<Download className="h-4 w-4" />} title="Model cache" detail="2.4 GB used" trailing={<StatusPill tone="gray">Local</StatusPill>} />
      <PanelRow icon={<ShieldCheck className="h-4 w-4" />} title="Privacy" detail="No remote uploads" trailing={<StatusPill tone="green">On</StatusPill>} />
    </GroupedPanel>
  );
}

function LibraryStats() {
  return (
    <GroupedPanel title="Library">
      <PanelRow icon={<History className="h-4 w-4" />} title="Sessions" detail="This month" trailing={<span className="text-[12px] font-semibold text-[#55555c]">18</span>} />
      <PanelRow icon={<Download className="h-4 w-4" />} title="Exports" detail="Markdown, SRT, TXT" trailing={<StatusPill tone="blue">12</StatusPill>} />
    </GroupedPanel>
  );
}

function ShortcutNote() {
  return (
    <GroupedPanel title="Behavior">
      <PanelRow icon={<Keyboard className="h-4 w-4" />} title="Global capture" detail="Works while ASR Pro is in the background" trailing={<StatusPill tone="green">On</StatusPill>} />
      <PanelRow icon={<ShieldCheck className="h-4 w-4" />} title="Permissions" detail="Microphone access required" trailing={<ChevronRight className="h-4 w-4 text-[#8a8a92]" />} />
    </GroupedPanel>
  );
}

interface SystemPanelProps {
  platformInfo: PlatformInfo | null;
}

function SystemPanel({ platformInfo }: SystemPanelProps) {
  return (
    <GroupedPanel title="System">
      <PanelRow icon={<Activity className="h-4 w-4" />} title="Platform" detail={formatPlatform(platformInfo?.platform)} trailing={<span className="text-[12px] font-semibold text-[#55555c]">{platformInfo?.arch ?? "local"}</span>} />
      <PanelRow icon={<Settings className="h-4 w-4" />} title="Electron" detail={platformInfo?.versions.electron ? `Version ${platformInfo.versions.electron}` : "Desktop runtime"} trailing={<StatusPill tone="gray">App</StatusPill>} />
    </GroupedPanel>
  );
}

interface ToggleRowProps {
  title: string;
  detail: string;
  enabled: boolean;
}

function ToggleRow({ title, detail, enabled }: ToggleRowProps) {
  return (
    <PanelRow
      icon={<Settings className="h-4 w-4" />}
      title={title}
      detail={detail}
      trailing={
        <span className={`relative inline-flex h-5 w-9 rounded-full transition ${enabled ? "bg-[#34c759]" : "bg-[#c7c7cc]"}`}>
          <span className={`absolute top-0.5 h-4 w-4 rounded-full bg-white shadow transition ${enabled ? "left-[18px]" : "left-0.5"}`} />
        </span>
      }
    />
  );
}

export default App;
