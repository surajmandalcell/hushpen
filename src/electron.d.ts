export {};

type OverlayPlacement = "top" | "bottom";
type SidecarState = {
  status: string;
  mode?: string;
  healthUrl?: string;
  command?: string | null;
  args?: string[];
  pid?: number | null;
  error?: string | null;
  updatedAt?: string;
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
        sidecar?: SidecarState;
      }>;
      getSidecarState?: () => Promise<SidecarState>;
      ensureEngineReady?: () => Promise<SidecarState>;
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
      onSidecarState?: (callback: (state: SidecarState) => void) => () => void;
      windowControl: (action: "minimize" | "close") => Promise<void>;
    };
  }
}
