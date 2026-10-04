import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  Activity,
  ArrowUpRight,
  BrainCircuit,
  Bug,
  Check,
  CheckCircle2,
  Copy,
  Download,
  FileText,
  Github,
  HardDrive,
  History,
  Home,
  Info,
  Library,
  Mic2,
  Minus,
  Pause,
  Play,
  RefreshCw,
  Search,
  Settings,
  Trash2,
  Volume2,
  VolumeX,
  X,
  type LucideIcon,
} from "lucide-react";
import { AppLogoMark } from "./components/icons";
import { AudioInputDeviceIcon } from "./components/ui/AudioInputDeviceIcon";
import {
  focusRingClass,
  historyActionButtonClass,
  iconTileClass,
  modelMetaBadgeClass,
  panelDividerClass,
  panelSurfaceClass,
  sharedRadiusClass,
} from "./components/ui/classes";
import { DropdownOptionButton, DropdownSurface } from "./components/ui/Dropdown";
import { GroupedPanel } from "./components/ui/GroupedPanel";
import { HoverPopover } from "./components/ui/HoverPopover";
import { NavigateButton } from "./components/ui/NavigateButton";
import { PanelControlButton } from "./components/ui/PanelControlButton";
import { PanelRow } from "./components/ui/PanelRow";
import { SegmentedControl, type SegmentedControlOption } from "./components/ui/SegmentedControl";
import { ShortcutCluster } from "./components/ui/ShortcutCluster";
import { StatusLabel } from "./components/ui/StatusLabel";
import { TextEditorIcon } from "./components/ui/TextEditorIcon";
import { ToggleSwitch } from "./components/ui/ToggleSwitch";
import { ViewFrame } from "./components/ui/ViewFrame";
import { buildAudioInputDeviceOptions } from "./lib/audioDevices";
import {
  defaultAppInfo,
  defaultAudioInputId,
  defaultAudioInputLabel,
  defaultAudioInputOptions,
  defaultAutoCopyTranscripts,
  defaultModelName,
  defaultTextEditorId,
  defaultTextEditorOptions,
  modelIdsByName,
} from "./lib/defaults";
import { getErrorMessage, getRecordingErrorTitle } from "./lib/errors";
import {
  countWords,
  formatByteCount,
  formatDuration,
  formatEngineStatus,
  formatHistoryGroupLabel,
  formatHomeRelativePath,
} from "./lib/format";
import { buildHistoryTitle, createTranscriptHistoryRow } from "./lib/history";
import { clampNumber } from "./lib/math";
import {
  getRuntimeModels,
  mergeOverlaySettings,
  mergeRuntimeInfo,
  normalizeAutoCopyTranscripts,
  normalizeLaunchAtStartup,
  normalizeOverlayPlacement,
  normalizeTextEditorId,
  normalizeTextEditorOptions,
} from "./lib/runtime";
import { formatShortcutParts } from "./lib/shortcut";
import {
  loadSelectedAudioInputId,
  loadSelectedModelName,
  loadTranscriptHistory,
  normalizeSelectedModelName,
  saveSelectedAudioInputId,
  saveSelectedModelName,
  saveTranscriptHistory,
} from "./lib/storage";
import { dataUrlToBlob, createTranscriptionAudioPayload, readBlobAsDataUrl } from "./lib/wav";
import {
  buildReactiveWaveformFrame,
  cancelWaveformFrame,
  historyWaveformBars,
  idleWaveformFrame,
  scheduleWaveformFrame,
  sendOverlayWaveformFrame,
} from "./lib/waveform";
import { audioRecordingService } from "./services/audioRecording";
import type { AppInfo, NavItem, RecordingStatus, ViewId, WindowAction } from "./types/app";
import type { AudioInputDeviceOption } from "./types/audio";
import type { EngineModelInfo, EngineRuntimeState } from "./types/engine";
import type { TranscriptHistoryRow } from "./types/history";
import type { RuntimeInfo, RuntimeStorageStats } from "./types/runtime";
import type { OverlayPlacement, TextEditorOption } from "./types/settings";

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
  const [selectedModel, setSelectedModel] = useState(() => loadSelectedModelName() ?? defaultModelName);
  const [audioInputDevices, setAudioInputDevices] = useState<AudioInputDeviceOption[]>(defaultAudioInputOptions);
  const [selectedAudioInputId, setSelectedAudioInputId] = useState(loadSelectedAudioInputId);
  const [audioInputDevicesLoading, setAudioInputDevicesLoading] = useState(false);
  const [audioInputDevicesError, setAudioInputDevicesError] = useState<string | null>(null);
  const [runtimeInfo, setRuntimeInfo] = useState<RuntimeInfo | null>(null);
  const [appInfo, setAppInfo] = useState<AppInfo>(defaultAppInfo);
  const [overlayPlacement, setOverlayPlacement] = useState<OverlayPlacement>("top");
  const [textEditorOptions, setTextEditorOptions] = useState<TextEditorOption[]>(defaultTextEditorOptions);
  const [selectedTextEditorId, setSelectedTextEditorId] = useState(defaultTextEditorId);
  const [autoCopyTranscripts, setAutoCopyTranscripts] = useState(defaultAutoCopyTranscripts);
  const [launchAtStartup, setLaunchAtStartup] = useState(false);
  const [historyRows, setHistoryRows] = useState<TranscriptHistoryRow[]>(loadTranscriptHistory);
  const [reprocessingHistoryRowId, setReprocessingHistoryRowId] = useState<string | null>(null);
  const [openingTranscriptRowId, setOpeningTranscriptRowId] = useState<string | null>(null);
  const [modelActionIds, setModelActionIds] = useState<Set<string>>(() => new Set());
  const [modelDownloadProgress, setModelDownloadProgress] = useState<Record<string, number>>({});
  const [modelLibraryError, setModelLibraryError] = useState<string | null>(null);
  const [isScrollbarVisible, setIsScrollbarVisible] = useState(true);
  const recordingStartedAtRef = useRef<number | null>(null);
  const recordingTransitionRef = useRef<"starting" | "stopping" | null>(null);
  const runtimeStateLoadedRef = useRef(false);
  const scrollbarTimerRef = useRef<number | null>(null);
  const overlayPlacementTouchedRef = useRef(false);
  const modelActionIdsRef = useRef<Set<string>>(new Set());
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

  const deleteHistoryRow = useCallback((row: TranscriptHistoryRow) => {
    const deleteTranscriptText = window.asrpro?.deleteTranscriptText;
    if (deleteTranscriptText) {
      void deleteTranscriptText({
        title: row.title,
        filePath: row.transcriptFilePath,
      }).catch(() => {});
    }

    setHistoryRows((current) => {
      const next = current.filter((currentRow) => currentRow.id !== row.id);
      saveTranscriptHistory(next);
      return next;
    });
  }, []);

  const writeTextToClipboard = useCallback((text: string) => {
    void navigator.clipboard?.writeText(text).catch(() => {});
  }, []);

  const copyHistoryText = useCallback((text: string) => {
    writeTextToClipboard(text);
  }, [writeTextToClipboard]);

  const runtimeModels = useMemo(() => getRuntimeModels(runtimeInfo?.models), [runtimeInfo?.models]);
  const selectedModelId = useMemo(() => (
    runtimeModels.find((model) => model.displayName === selectedModel)?.id ?? modelIdsByName[selectedModel] ?? "whisper-base-en"
  ), [runtimeModels, selectedModel]);

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
      modelId: selectedModelId,
    });
  }, [selectedModelId]);

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
        const result = await window.asrpro.openTranscriptText(request);
        if (result?.filePath) {
          updateHistoryRow(row.id, (current) => ({
            ...current,
            transcriptFilePath: result.filePath,
          }));
        }
        return;
      }

      const blobUrl = URL.createObjectURL(new Blob([`${row.text.trim()}\n`], { type: "text/plain;charset=utf-8" }));
      window.open(blobUrl, "_blank", "noopener,noreferrer");
      window.setTimeout(() => URL.revokeObjectURL(blobUrl), 60_000);
    } finally {
      setOpeningTranscriptRowId(null);
    }
  }, [openingTranscriptRowId, updateHistoryRow]);

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

  const handleSelectModel = useCallback((modelName: string) => {
    setSelectedModel(modelName);
    saveSelectedModelName(modelName);
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

  const handleAutoCopyTranscriptsChange = useCallback((enabled: boolean) => {
    setAutoCopyTranscripts(enabled);
    setRuntimeInfo((current) => (current ? { ...current, autoCopyTranscripts: enabled } : current));

    const saveAutoCopyTranscripts = window.asrpro?.setAutoCopyTranscripts?.(enabled);
    if (!saveAutoCopyTranscripts) return;

    saveAutoCopyTranscripts.then((settings) => {
      const nextAutoCopy = normalizeAutoCopyTranscripts(settings.autoCopyTranscripts);
      setAutoCopyTranscripts(nextAutoCopy);
      setRuntimeInfo((current) => (current ? { ...current, autoCopyTranscripts: nextAutoCopy } : current));
    }).catch(() => {});
  }, []);

  const handleStartupLaunchChange = useCallback((enabled: boolean) => {
    setLaunchAtStartup(enabled);
    setRuntimeInfo((current) => (current ? {
      ...current,
      launchAtStartup: enabled,
      startup: current.startup ? { ...current.startup, enabled } : current.startup,
    } : current));

    const saveStartup = window.asrpro?.setStartupLaunch?.(enabled);
    if (!saveStartup) return;

    saveStartup.then((settings) => {
      const nextLaunchAtStartup = normalizeLaunchAtStartup(settings.startup, settings.launchAtStartup);
      setLaunchAtStartup(nextLaunchAtStartup);
      setRuntimeInfo((current) => (current ? {
        ...current,
        launchAtStartup: nextLaunchAtStartup,
        startup: settings.startup ?? current.startup,
      } : current));
    }).catch(() => {
      setLaunchAtStartup(!enabled);
      setRuntimeInfo((current) => (current ? {
        ...current,
        launchAtStartup: !enabled,
        startup: current.startup ? { ...current.startup, enabled: !enabled } : current.startup,
      } : current));
    });
  }, []);

  const selectedAudioInputLabel = useMemo(() => (
    audioInputDevices.find((device) => device.id === selectedAudioInputId)?.label ?? defaultAudioInputLabel
  ), [audioInputDevices, selectedAudioInputId]);

  const selectedTextEditorLabel = useMemo(() => (
    textEditorOptions.find((editor) => editor.id === selectedTextEditorId)?.label ?? defaultTextEditorOptions[0].label
  ), [selectedTextEditorId, textEditorOptions]);

  const beginModelAction = useCallback((modelId: string) => {
    if (modelActionIdsRef.current.has(modelId)) return false;
    const nextIds = new Set(modelActionIdsRef.current);
    nextIds.add(modelId);
    modelActionIdsRef.current = nextIds;
    setModelActionIds(nextIds);
    return true;
  }, []);

  const endModelAction = useCallback((modelId: string) => {
    if (!modelActionIdsRef.current.has(modelId)) return;
    const nextIds = new Set(modelActionIdsRef.current);
    nextIds.delete(modelId);
    modelActionIdsRef.current = nextIds;
    setModelActionIds(nextIds);
  }, []);

  const updateModelDownloadProgress = useCallback((modelId: string, progress: number) => {
    const nextProgress = clampNumber(progress, 0, 100);
    setModelDownloadProgress((current) => (
      current[modelId] === nextProgress ? current : { ...current, [modelId]: nextProgress }
    ));
  }, []);

  const clearModelDownloadProgress = useCallback((modelId: string) => {
    setModelDownloadProgress((current) => {
      if (!(modelId in current)) return current;
      const next = { ...current };
      delete next[modelId];
      return next;
    });
  }, []);

  const handleDownloadModel = useCallback(async (modelId: string) => {
    const downloadModel = window.asrpro?.downloadModel;
    if (!downloadModel) return;
    if (!beginModelAction(modelId)) return;

    updateModelDownloadProgress(modelId, 0);
    setModelLibraryError(null);

    try {
      const state = await downloadModel(modelId);
      setRuntimeInfo((current) => mergeRuntimeInfo(current, state));
    } catch (error) {
      setModelLibraryError(getErrorMessage(error));
    } finally {
      endModelAction(modelId);
      clearModelDownloadProgress(modelId);
    }
  }, [beginModelAction, clearModelDownloadProgress, endModelAction, updateModelDownloadProgress]);

  const handleDeleteModel = useCallback(async (modelId: string) => {
    const deleteModel = window.asrpro?.deleteModel;
    if (!deleteModel) return;
    if (!beginModelAction(modelId)) return;

    setModelLibraryError(null);

    try {
      const state = await deleteModel(modelId);
      setRuntimeInfo((current) => mergeRuntimeInfo(current, state));
    } catch (error) {
      setModelLibraryError(getErrorMessage(error));
    } finally {
      endModelAction(modelId);
      clearModelDownloadProgress(modelId);
    }
  }, [beginModelAction, clearModelDownloadProgress, endModelAction]);

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
      const normalizedText = text.replace(/\s+/g, " ").trim();

      addHistoryRow(createTranscriptHistoryRow({
        text: normalizedText,
        model: selectedModel,
        durationSeconds,
        startedAt,
        recordingUrl,
      }));
      if (autoCopyTranscripts) {
        writeTextToClipboard(normalizedText);
      }
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
  }, [addHistoryRow, autoCopyTranscripts, selectedModel, syncRecordingBridge, transcribeRecording, writeTextToClipboard]);

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

    if (api.getRuntimeState && !runtimeStateLoadedRef.current) {
      runtimeStateLoadedRef.current = true;
      Promise.resolve(api.getRuntimeState()).then((state) => {
        if (!state) return;
        setRuntimeInfo(state);
        const nextModels = getRuntimeModels(state.models);
        const nextSelectedModel = loadSelectedModelName(nextModels)
          ?? normalizeSelectedModelName(state.defaultModelId, nextModels)
          ?? normalizeSelectedModelName(state.defaultModel, nextModels)
          ?? defaultModelName;
        setSelectedModel(nextSelectedModel);
        const nextTextEditorOptions = normalizeTextEditorOptions(state.textEditors);
        setTextEditorOptions(nextTextEditorOptions);
        setSelectedTextEditorId(normalizeTextEditorId(state.defaultTextEditor, nextTextEditorOptions));
        setAutoCopyTranscripts(normalizeAutoCopyTranscripts(state.autoCopyTranscripts));
        setLaunchAtStartup(normalizeLaunchAtStartup(state.startup, state.launchAtStartup));
        if (!overlayPlacementTouchedRef.current) {
          setOverlayPlacement(normalizeOverlayPlacement(state.overlaySettings?.placement));
        }
        if (state.isRecording) {
          void startRecordingFlow(false);
        } else if (!recordingTransitionRef.current && !audioRecordingService.isRecording()) {
          setIsRecording(false);
        }
      }).catch(() => {
        runtimeStateLoadedRef.current = false;
      });
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
      if (engineState.modelId && engineState.status === "downloading" && typeof engineState.progress === "number") {
        updateModelDownloadProgress(engineState.modelId, engineState.progress);
      } else if (engineState.modelId && engineState.status !== "downloading") {
        clearModelDownloadProgress(engineState.modelId);
      }
    });

    return () => {
      unsubscribeRecording?.();
      unsubscribeEngine?.();
    };
  }, [clearModelDownloadProgress, startRecordingFlow, stopRecordingFlow, updateModelDownloadProgress]);

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
            {activeView === "models" && (
              <ModelsView
                selectedModel={selectedModel}
                models={runtimeModels}
                storageStats={runtimeInfo?.storageStats}
                engine={runtimeInfo?.engine}
                busyModelIds={modelActionIds}
                modelProgressById={modelDownloadProgress}
                actionError={modelLibraryError}
                onSelectModel={handleSelectModel}
                onDownloadModel={handleDownloadModel}
                onDeleteModel={handleDeleteModel}
              />
            )}
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
                autoCopyTranscripts={autoCopyTranscripts}
                launchAtStartup={launchAtStartup}
                onOverlayPlacementChange={handleOverlayPlacementChange}
                onTextEditorChange={handleTextEditorChange}
                onAutoCopyTranscriptsChange={handleAutoCopyTranscriptsChange}
                onStartupLaunchChange={handleStartupLaunchChange}
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

const overlayPlacementControlOptions: readonly SegmentedControlOption<OverlayPlacement>[] = [
  { value: "top", label: "Top", ariaLabel: "Top overlay position" },
  { value: "bottom", label: "Bottom", ariaLabel: "Bottom overlay position" },
];

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
  models: EngineModelInfo[];
  storageStats?: RuntimeStorageStats;
  engine?: EngineRuntimeState;
  busyModelIds: ReadonlySet<string>;
  modelProgressById: Record<string, number>;
  actionError: string | null;
  onSelectModel: (model: string) => void;
  onDownloadModel: (modelId: string) => void;
  onDeleteModel: (modelId: string) => void;
}

function ModelsView({
  selectedModel,
  models,
  storageStats,
  engine,
  busyModelIds,
  modelProgressById,
  actionError,
  onSelectModel,
  onDownloadModel,
  onDeleteModel,
}: ModelsViewProps) {
  return (
    <ViewFrame title="Models library">
      <GroupedPanel title="Recognition models" allowOverflow>
        {models.map((model) => {
          const selected = selectedModel === model.displayName;
          const busy = busyModelIds.has(model.id);
          const installed = Boolean(model.installed);
          const diskLabel = installed && model.diskBytes ? formatByteCount(model.diskBytes) : model.sizeLabel;
          const trackedProgress = modelProgressById[model.id];
          const fallbackProgress = busy && engine?.modelId === model.id && typeof engine.progress === "number"
            ? clampNumber(engine.progress, 0, 100)
            : null;
          const progress = busy && typeof trackedProgress === "number"
            ? clampNumber(trackedProgress, 0, 100)
            : fallbackProgress;

          return (
            <div
              key={model.id}
              className={`border-t ${panelDividerClass} p-3 first:border-t-0 ${selected ? "bg-white/[0.055]" : ""}`}
            >
              <div className="flex min-w-0 flex-col gap-3 sm:flex-row sm:items-center">
                <div
                  className={`flex min-w-0 flex-1 items-center gap-3 ${sharedRadiusClass} px-2 py-1.5 text-left`}
                >
                  <div className={iconTileClass}>
                    <BrainCircuit className="size-3" />
                  </div>
                  <span className="min-w-0 flex-1">
                    <span className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
                      <span className="text-[13px] font-semibold leading-5 text-[#f2f2f2]">{model.displayName}</span>
                      <span className={modelMetaBadgeClass}>{diskLabel}</span>
                    </span>
                    <span className="selectable-text mt-0.5 block text-[12px] font-medium leading-4 text-[#aaa]">{model.detail}</span>
                  </span>
                </div>
                <div data-model-controls className="grid shrink-0 grid-cols-[32px_32px_32px] items-center gap-2 pl-11 sm:pl-0">
                  <ModelSelectButton
                    modelName={model.displayName}
                    selected={selected}
                    onClick={() => onSelectModel(model.displayName)}
                  />
                  <ModelStatusLabel installed={installed} modelName={model.displayName} />
                  {installed ? (
                    <ModelActionButton
                      ariaLabel={`Delete ${model.displayName}`}
                      busy={busy}
                      kind="delete"
                      onClick={() => onDeleteModel(model.id)}
                    />
                  ) : (
                    <ModelActionButton
                      ariaLabel={`Download ${model.displayName}`}
                      busy={busy}
                      kind="download"
                      onClick={() => onDownloadModel(model.id)}
                    />
                  )}
                </div>
              </div>
              {progress !== null ? (
                <div className="mt-3 pl-11">
                  <div className="flex items-center justify-between text-[11px] font-semibold text-[#9c9c9c]">
                    <span>Setup progress</span>
                    <span>{Math.round(progress)}%</span>
                  </div>
                  <div className="mt-1 h-1.5 overflow-hidden rounded-full bg-white/[0.08]">
                    <div className="h-full rounded-full bg-[#9bcfff]" style={{ width: `${progress}%` }} />
                  </div>
                </div>
              ) : null}
            </div>
          );
        })}
      </GroupedPanel>
      {actionError ? (
        <p role="status" className="selectable-text px-1 text-[12px] font-semibold text-[#ffb3aa]">{actionError}</p>
      ) : null}
      <ResourceStatsPanel stats={storageStats} />
    </ViewFrame>
  );
}

interface ModelSelectButtonProps {
  modelName: string;
  selected: boolean;
  onClick: () => void;
}

function ModelSelectButton({ modelName, selected, onClick }: ModelSelectButtonProps) {
  return (
    <HoverPopover content={selected ? "Current model" : "Select model"}>
      <button
        type="button"
        aria-label={`Select ${modelName}`}
        aria-pressed={selected}
        className={`grid size-8 shrink-0 place-items-center rounded-full border-0 bg-transparent p-0 transition active:scale-[0.96] ${focusRingClass} ${selected ? "text-[#9bcfff] hover:bg-[#263b4d]" : "text-[#cfcfcf] hover:bg-white/[0.08] hover:text-[#eeeeee]"}`}
        onClick={onClick}
      >
        <Check className="size-3.5" />
      </button>
    </HoverPopover>
  );
}

function ModelStatusLabel({ installed, modelName }: { installed: boolean; modelName: string }) {
  if (installed) {
    return (
      <HoverPopover content="Downloaded model">
        <span
          role="img"
          aria-label={`${modelName} downloaded`}
          className="grid size-8 shrink-0 place-items-center rounded-full text-[#a9d9b8]"
        >
          <CheckCircle2 className="size-3.5" />
        </span>
      </HoverPopover>
    );
  }

  return (
    <HoverPopover content="Not downloaded">
      <span
        role="img"
        aria-label={`${modelName} not downloaded`}
        className="grid size-8 shrink-0 place-items-center rounded-full text-[#cfcfcf] opacity-75"
      >
        <CheckCircle2 className="size-3.5" />
      </span>
    </HoverPopover>
  );
}

interface ModelActionButtonProps {
  ariaLabel: string;
  busy: boolean;
  kind: "download" | "delete";
  onClick: () => void;
}

function ModelActionButton({ ariaLabel, busy, kind, onClick }: ModelActionButtonProps) {
  const Icon = busy ? RefreshCw : kind === "delete" ? Trash2 : Download;
  const toneClass = kind === "delete"
    ? "text-[#cfcfcf] hover:bg-[#4a3333] hover:text-[#ffb3aa]"
    : "text-[#cfcfcf] hover:bg-[#344235] hover:text-[#bce7c9]";

  return (
    <HoverPopover content={kind === "delete" ? "Delete model" : "Download model"}>
      <button
        type="button"
        aria-label={ariaLabel}
        className={`grid size-8 shrink-0 place-items-center rounded-full border-0 bg-transparent p-0 transition active:scale-[0.96] disabled:cursor-wait disabled:opacity-55 ${toneClass} ${focusRingClass}`}
        disabled={busy}
        onClick={onClick}
      >
        <Icon className={`size-3.5 ${busy ? "animate-spin" : ""}`} />
      </button>
    </HoverPopover>
  );
}

function ResourceStatsPanel({ stats }: { stats?: RuntimeStorageStats }) {
  const groups = stats?.groups ?? [];

  return (
    <GroupedPanel title="Storage and memory" allowOverflow>
      {groups.length ? (
        groups.map((group) => (
          <section key={group.id} className={`border-t ${panelDividerClass} p-4 first:border-t-0`}>
            <div className="flex items-center gap-3">
              <div className={iconTileClass}>
                {group.id === "memory" ? <Activity className="size-3" /> : <HardDrive className="size-3" />}
              </div>
              <div className="min-w-0 flex-1">
                <p className="text-[13px] font-semibold text-[#eeeeee]">{group.label}</p>
                {group.detail ? <p className="selectable-text mt-0.5 truncate text-[12px] font-medium text-[#aaa]">{group.detail}</p> : null}
              </div>
              <span className="shrink-0 text-[13px] font-semibold text-[#f0f0f0]">{formatByteCount(group.totalBytes)}</span>
            </div>
            <div className="mt-3 grid grid-cols-1 gap-2 sm:grid-cols-2">
              {group.items.map((item) => (
                <div key={item.id} className={`${sharedRadiusClass} border border-white/[0.07] bg-white/[0.045] px-3 py-2`}>
                  <div className="flex min-w-0 items-center justify-between gap-2">
                    <span className="truncate text-[12px] font-semibold text-[#d8d8d8]">{item.label}</span>
                    <span className="shrink-0 text-[12px] font-semibold text-[#eeeeee]">{formatByteCount(item.bytes)}</span>
                  </div>
                  {item.detail || item.path ? (
                    <p className="selectable-text mt-1 truncate text-[11px] font-medium text-[#8f8f8f]">{item.detail ?? formatHomeRelativePath(item.path)}</p>
                  ) : null}
                </div>
              ))}
            </div>
          </section>
        ))
      ) : (
        <div className="p-4">
          <PanelRow
            icon={<HardDrive className="size-3.5" />}
            title="Runtime stats"
            detail="Waiting for desktop storage and memory details"
            trailing={<StatusLabel>Pending</StatusLabel>}
          />
        </div>
      )}
    </GroupedPanel>
  );
}

interface HistoryViewProps {
  rows: TranscriptHistoryRow[];
  onCopyRow: (text: string) => void;
  onReprocessRow: (row: TranscriptHistoryRow) => void;
  onOpenTranscriptRow: (row: TranscriptHistoryRow) => void;
  onDeleteRow: (row: TranscriptHistoryRow) => void;
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
                    onDelete={() => onDeleteRow(row)}
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
  autoCopyTranscripts: boolean;
  launchAtStartup: boolean;
  onOverlayPlacementChange: (placement: OverlayPlacement) => void;
  onTextEditorChange: (editorId: string) => void;
  onAutoCopyTranscriptsChange: (enabled: boolean) => void;
  onStartupLaunchChange: (enabled: boolean) => void;
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
  autoCopyTranscripts,
  launchAtStartup,
  onOverlayPlacementChange,
  onTextEditorChange,
  onAutoCopyTranscriptsChange,
  onStartupLaunchChange,
  onOpenModels,
  onOpenSound,
}: SettingsViewProps) {
  const shortcutParts = formatShortcutParts(runtimeInfo?.shortcut);
  const engine = runtimeInfo?.engine;
  const engineStatus = formatEngineStatus(engine?.status);
  const engineDetail = engine?.error || engine?.detail || (engine?.status === "idle" ? "Loads the selected Whisper model when needed" : engine?.model || engine?.mode || "Waiting for desktop runtime");
  const startup = runtimeInfo?.startup;
  const startupSupported = startup?.supported ?? Boolean(window.asrpro?.setStartupLaunch);
  const startupPath = startup?.executablePath || startup?.registeredExecutablePath || "Starts ASR Pro when you sign in";
  const startupDetail = startup?.detail || startupPath;

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
        <PanelRow
          title="Auto-copy transcripts"
          detail="Copy completed dictation to clipboard"
          trailing={(
            <ToggleSwitch
              label="Auto-copy transcripts"
              checked={autoCopyTranscripts}
              onChange={onAutoCopyTranscriptsChange}
            />
          )}
        />
        <PanelRow
          title="Launch at startup"
          detail={startupDetail}
          trailing={(
            <ToggleSwitch
              label="Launch at startup"
              checked={launchAtStartup}
              disabled={!startupSupported}
              onChange={onStartupLaunchChange}
            />
          )}
        />
        <PanelRow title="Engine" detail={engineDetail} trailing={<StatusLabel>{engineStatus}</StatusLabel>} />
        <PanelRow title="Data folder" detail={runtimeInfo?.dataDir ?? "App-contained data directory"} trailing={<StatusLabel>Read only</StatusLabel>} />
      </GroupedPanel>
    </ViewFrame>
  );
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
          <div
            data-brand-icon-surface="ink-slate"
            className="grid size-[72px] shrink-0 place-items-center rounded-[16px] text-[#eef4f5] shadow-[inset_0_1px_0_rgba(255,255,255,0.2),inset_0_-16px_24px_rgba(0,0,0,0.45)]"
            style={{ background: "linear-gradient(145deg, #20272d, #10171d 50%, #04070a)" }}
          >
            <AppLogoMark className="size-16 opacity-[0.88]" title="ASR Pro" />
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
  const selectedEditor = options.find((editor) => editor.id === selectedEditorId) ?? {
    id: selectedEditorId,
    label: selectedLabel,
    detail: "",
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
        <TextEditorIcon editor={selectedEditor} className="size-3 shrink-0" />
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
                aria-label={editor.label}
                onClick={() => handleSelect(editor.id)}
              >
                <TextEditorIcon editor={editor} className="mt-0.5 size-3 shrink-0" />
                <span className="min-w-0 flex-1 whitespace-normal break-words">{editor.label}</span>
                {selected ? <Check className="mt-0.5 size-3 shrink-0 text-[#9bcfff]" /> : null}
              </DropdownOptionButton>
            );
          })}
        </DropdownSurface>
      ) : null}
    </div>
  );
}

export default App;
