export {};

type OverlayPlacement = "top" | "bottom";
type EngineState = {
  status: string;
  mode?: string;
  modelId?: string;
  model?: string;
  detail?: string;
  progress?: number | null;
  error?: string | null;
  updatedAt?: string;
};

type EngineModelInfo = {
  id: string;
  displayName: string;
  detail: string;
  sizeLabel: string;
  installed?: boolean;
  diskBytes?: number;
  path?: string;
  downloadUrl?: string;
};

type StorageStatsGroup = {
  id: string;
  label: string;
  totalBytes: number;
  detail?: string;
  items: Array<{
    id: string;
    label: string;
    bytes: number;
    detail?: string;
    path?: string;
  }>;
};

type RuntimeStorageStats = {
  generatedAt?: string;
  groups: StorageStatsGroup[];
};

type TextEditorOption = {
  id: string;
  label: string;
  detail: string;
  iconDataUrl?: string;
};

declare global {
  interface Window {
    asrpro?: {
      isScreenshotMode?: boolean;
      getPlatform: () => Promise<{
        platform: string;
        arch: string;
        versions: {
          electron?: string;
          chrome?: string;
          node?: string;
        };
      }>;
      getAppInfo: () => Promise<{
        name: string;
        version: string;
      }>;
      getRuntimeState: () => Promise<{
        isRecording: boolean;
        dataDir: string;
        defaultModel: string;
        defaultModelId?: string;
        models?: EngineModelInfo[];
        storageStats?: RuntimeStorageStats;
        defaultTextEditor?: string;
        autoCopyTranscripts?: boolean;
        textEditors?: TextEditorOption[];
        overlaySettings?: {
          placement: OverlayPlacement;
          customBounds: {
            displayId: number;
            x: number;
            y: number;
          } | null;
        };
        shortcut: string;
        shortcutRegistered: boolean;
        engine?: EngineState;
        capabilities?: {
          nativeWhisper?: boolean;
        };
      }>;
      getEngineState?: () => Promise<EngineState>;
      getModels?: () => Promise<EngineModelInfo[]>;
      downloadModel?: (modelId: string) => Promise<{
        isRecording: boolean;
        models?: EngineModelInfo[];
        storageStats?: RuntimeStorageStats;
      }>;
      deleteModel?: (modelId: string) => Promise<{
        isRecording: boolean;
        models?: EngineModelInfo[];
        storageStats?: RuntimeStorageStats;
      }>;
      transcribeAudio?: (request: { audioData: ArrayBuffer; mimeType: string; modelId: string }) => Promise<{
        text: string;
        model: string;
        modelName?: string;
      }>;
      openTranscriptText?: (request: { title: string; text: string }) => Promise<{
        filePath: string;
      }>;
      deleteTranscriptText?: (request: { title: string; filePath?: string }) => Promise<{
        deleted: boolean;
        filePath: string;
      }>;
      setDefaultTextEditor?: (editorId: string) => Promise<{
        defaultTextEditor: string;
      }>;
      setAutoCopyTranscripts?: (enabled: boolean) => Promise<{
        autoCopyTranscripts: boolean;
      }>;
      getOverlaySettings?: () => Promise<{
        placement: OverlayPlacement;
        customBounds: {
          displayId: number;
          x: number;
          y: number;
        } | null;
      }>;
      setOverlaySettings?: (settings: { placement: OverlayPlacement }) => Promise<{
        placement: OverlayPlacement;
        customBounds: {
          displayId: number;
          x: number;
          y: number;
        } | null;
      }>;
      setRecording: (active: boolean) => Promise<{ isRecording: boolean }>;
      toggleRecording: () => Promise<{ isRecording: boolean }>;
      setWaveformFrame?: (frame: number[]) => void;
      onRecordingState: (callback: (state: { isRecording: boolean; source: string }) => void) => () => void;
      onEngineState?: (callback: (state: EngineState) => void) => () => void;
      windowControl: (action: "minimize" | "close") => Promise<void>;
    };
  }
}
