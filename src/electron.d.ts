export {};

type OverlayPlacement = "top" | "bottom";

declare global {
  interface Window {
    asrpro?: {
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
      selectAudioFiles: () => Promise<Array<{ fileName: string; path: string }>>;
      onAddFiles: (callback: () => void) => () => void;
      setRecording: (active: boolean) => Promise<{ isRecording: boolean }>;
      toggleRecording: () => Promise<{ isRecording: boolean }>;
      onRecordingState: (callback: (state: { isRecording: boolean; source: string }) => void) => () => void;
      windowControl: (action: "minimize" | "maximize" | "close") => Promise<void>;
    };
  }
}
