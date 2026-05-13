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
      selectAudioFiles: () => Promise<Array<{ fileName: string; path: string }>>;
      onAddFiles: (callback: () => void) => () => void;
      windowControl: (action: "minimize" | "maximize" | "close") => Promise<void>;
    };
  }
}
