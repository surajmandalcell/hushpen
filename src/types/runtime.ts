import type { EngineModelInfo, EngineRuntimeState } from "./engine";
import type { OverlaySettings, StartupSettings, TextEditorOption } from "./settings";

export interface RuntimeInfo {
  isRecording: boolean;
  defaultModel?: string;
  defaultModelId?: string;
  audioInputId?: string;
  dataDir?: string;
  overlaySettings?: OverlaySettings;
  engine?: EngineRuntimeState;
  models?: EngineModelInfo[];
  storageStats?: RuntimeStorageStats;
  defaultTextEditor?: string;
  autoCopyTranscripts?: boolean;
  launchAtStartup?: boolean;
  startup?: StartupSettings;
  textEditors?: TextEditorOption[];
  shortcut?: string;
  shortcutRegistered?: boolean;
  capabilities?: {
    nativeWhisper?: boolean;
  };
}

export interface RuntimeStorageStats {
  generatedAt?: string;
  groups: StorageStatsGroup[];
}

export interface StorageStatsGroup {
  id: string;
  label: string;
  totalBytes: number;
  detail?: string;
  items: StorageStatsItem[];
}

export interface StorageStatsItem {
  id: string;
  label: string;
  bytes: number;
  detail?: string;
  path?: string;
}
