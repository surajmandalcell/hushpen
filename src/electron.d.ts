export {};

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
        shortcut: string;
        shortcutRegistered: boolean;
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
