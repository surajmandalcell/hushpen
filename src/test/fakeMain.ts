import { vi } from "vitest";
import { isAppError } from "../lib/bridge";

type Handler = (...args: never[]) => unknown;

/**
 * Per-feature doubles for the main process. `installFakeMain` exposes them as
 * the `window.asrpro` transport (`invoke`, `send`, `on`) and wraps every reply
 * in the same envelope the real router sends, so tests exercise `bridge.ts`.
 * A double that throws an `AppError` replies with that error code.
 */
export interface FakeMainHandlers {
  isScreenshotMode?: boolean;
  getPlatform?: Handler;
  getAppInfo?: Handler;
  getRuntimeState?: Handler;
  getEngineState?: Handler;
  windowControl?: Handler;
  setRecording?: Handler;
  toggleRecording?: Handler;
  downloadModel?: Handler;
  deleteModel?: Handler;
  transcribeAudio?: Handler;
  openTranscriptText?: Handler;
  deleteTranscriptText?: Handler;
  setDefaultTextEditor?: Handler;
  setAutoCopyTranscripts?: Handler;
  setStartupLaunch?: Handler;
  setOverlaySettings?: Handler;
  importLegacySettings?: Handler;
  setWaveformFrame?: Handler;
  onRecordingState?: Handler;
  onEngineState?: Handler;
}

export interface FakeMain {
  /** Every `settings:set` call, in order. */
  settingsWrites: { key: string; value: unknown }[];
  invoke: ReturnType<typeof vi.fn>;
}

function call(handler: Handler | undefined, ...args: unknown[]) {
  return handler ? (handler as (...input: unknown[]) => unknown)(...args) : undefined;
}

function envelope(run: () => unknown): Promise<unknown> {
  return Promise.resolve()
    .then(run)
    .then((value) => ({ ok: true, value }))
    .catch((error: unknown) => {
      if (isAppError(error)) {
        return { ok: false, error: { code: error.code, params: error.params, detail: error.detail } };
      }
      return { ok: false, error: { code: "INTERNAL", detail: error instanceof Error ? error.message : undefined } };
    });
}

export function installFakeMain(handlers: FakeMainHandlers = {}): FakeMain {
  const settingsWrites: FakeMain["settingsWrites"] = [];

  const setSetting = async ({ key, value }: { key: string; value: unknown }) => {
    settingsWrites.push({ key, value });
    switch (key) {
      case "editor.defaultTextEditor": {
        const reply = (await call(handlers.setDefaultTextEditor, value)) as { defaultTextEditor?: string } | undefined;
        return { values: { [key]: reply?.defaultTextEditor ?? value } };
      }
      case "output.autoCopy": {
        const reply = (await call(handlers.setAutoCopyTranscripts, value)) as { autoCopyTranscripts?: boolean } | undefined;
        return { values: { [key]: reply?.autoCopyTranscripts ?? value } };
      }
      case "startup.launchAtLogin": {
        const reply = (await call(handlers.setStartupLaunch, value)) as { launchAtStartup?: boolean; startup?: unknown } | undefined;
        return { values: { [key]: reply?.launchAtStartup ?? value }, startup: reply?.startup };
      }
      case "overlay.placement": {
        const reply = (await call(handlers.setOverlaySettings, { placement: value })) as { placement?: string } | undefined;
        return { values: { [key]: reply?.placement ?? value } };
      }
      default:
        return { values: { [key]: value } };
    }
  };

  const routes: Record<string, (payload: never) => unknown> = {
    "app:platform": () => call(handlers.getPlatform),
    "app:info": () => call(handlers.getAppInfo),
    "runtime:state": () => call(handlers.getRuntimeState),
    "engine:get-state": () => call(handlers.getEngineState),
    "window:control": (payload: { action: string }) => call(handlers.windowControl, payload.action),
    "recording:set": (payload: { active: boolean }) => call(handlers.setRecording, payload.active),
    "recording:toggle": () => call(handlers.toggleRecording),
    "models:download": (payload: { modelId: string }) => call(handlers.downloadModel, payload.modelId),
    "models:delete": (payload: { modelId: string }) => call(handlers.deleteModel, payload.modelId),
    "engine:transcribe-audio": (payload: unknown) => call(handlers.transcribeAudio, payload),
    "transcript:open-text": (payload: unknown) => call(handlers.openTranscriptText, payload),
    "transcript:delete-text": (payload: unknown) => call(handlers.deleteTranscriptText, payload),
    "settings:set": setSetting,
    "settings:import-legacy": (payload: unknown) => (
      handlers.importLegacySettings ? call(handlers.importLegacySettings, payload) : { imported: false, values: {} }
    ),
  };

  const invoke = vi.fn((channel: string, payload?: unknown) => envelope(() => {
    const route = routes[channel];
    if (!route) throw new Error(`No fake handler for ${channel}`);
    return (route as (input: unknown) => unknown)(payload);
  }));

  window.asrpro = {
    isScreenshotMode: handlers.isScreenshotMode,
    invoke,
    send: (channel: string, payload?: unknown) => {
      if (channel === "recording:waveform-frame") call(handlers.setWaveformFrame, payload);
    },
    on: (channel: string, callback: (payload: unknown) => void) => {
      const subscribe = channel === "recording:state"
        ? handlers.onRecordingState
        : channel === "engine:state"
          ? handlers.onEngineState
          : undefined;
      return call(subscribe, callback) as (() => void) | undefined ?? (() => {});
    },
  };

  return { settingsWrites, invoke };
}
