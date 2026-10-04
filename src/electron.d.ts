export {};

declare global {
  interface Window {
    asrpro?: {
      isScreenshotMode?: boolean;
      invoke: (channel: string, payload?: unknown) => Promise<unknown>;
      send: (channel: string, payload?: unknown) => void;
      on: (channel: string, callback: (payload: unknown) => void) => () => void;
    };
  }
}
