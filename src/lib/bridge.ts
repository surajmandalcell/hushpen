import errorCodes from "../../shared/error-codes.json";
import type { ErrorCode, ErrorShape, SettingKey, SettingsValues } from "../types/contracts";
import type { AppInfo, WindowAction } from "../types/app";
import type { EngineRuntimeState } from "../types/engine";
import type { RuntimeInfo } from "../types/runtime";
import type { StartupSettings } from "../types/settings";

export class AppError extends Error {
  readonly code: ErrorCode;
  readonly params?: Record<string, unknown>;
  readonly detail?: string;

  constructor(code: ErrorCode, params?: Record<string, unknown>, detail?: string) {
    super(detail ? `${code}: ${detail}` : code);
    this.name = "AppError";
    this.code = code;
    this.params = params;
    this.detail = detail;
  }
}

export function isAppError(value: unknown): value is AppError {
  return value instanceof AppError;
}

const knownCodes = new Set<string>(Object.keys(errorCodes.codes));

function toAppError(shape: unknown): AppError {
  const candidate = shape as Partial<ErrorShape> | null | undefined;
  const code = candidate && typeof candidate.code === "string" && knownCodes.has(candidate.code)
    ? (candidate.code as ErrorCode)
    : "INTERNAL";
  return new AppError(code, candidate?.params, typeof candidate?.detail === "string" ? candidate.detail : undefined);
}

function isEnvelope(value: unknown): value is { ok: boolean; value?: unknown; error?: unknown } {
  return Boolean(value) && typeof value === "object" && typeof (value as { ok?: unknown }).ok === "boolean";
}

async function invoke<T>(channel: string, payload?: unknown): Promise<T> {
  const transport = window.asrpro;
  if (!transport?.invoke) {
    throw new AppError("INTERNAL", undefined, "The desktop bridge is not available.");
  }

  let reply: unknown;
  try {
    reply = await transport.invoke(channel, payload);
  } catch (caught) {
    throw new AppError("INTERNAL", undefined, caught instanceof Error ? caught.message : undefined);
  }

  if (!isEnvelope(reply)) {
    throw new AppError("INTERNAL", undefined, "The main process sent an unexpected reply.");
  }
  if (!reply.ok) {
    throw toAppError(reply.error);
  }
  return reply.value as T;
}

function subscribe<T>(channel: string, callback: (payload: T) => void) {
  const unsubscribe = window.asrpro?.on?.(channel, (payload) => callback(payload as T));
  return unsubscribe ?? (() => {});
}

export interface RecordingStateEvent {
  isRecording: boolean;
  source: string;
}

export interface SettingsReply {
  values: SettingsValues;
  startup?: StartupSettings;
}

export interface TranscriptionRequest {
  audioData: ArrayBuffer | Uint8Array;
  mimeType: string;
  modelId?: string;
}

export interface TranscriptionResult {
  text: string;
  model: string;
  modelName?: string;
}

export interface LegacySettingsImport {
  selectedModelName?: string;
  audioInputId?: string;
}

export const bridge = {
  isAvailable: () => typeof window !== "undefined" && typeof window.asrpro?.invoke === "function",
  isScreenshotMode: () => Boolean(window.asrpro?.isScreenshotMode),

  getAppInfo: () => invoke<AppInfo>("app:info"),
  getRuntimeState: () => invoke<RuntimeInfo>("runtime:state"),
  getEngineState: () => invoke<EngineRuntimeState>("engine:get-state"),
  windowControl: (action: WindowAction) => invoke<void>("window:control", { action }),

  setSetting: (key: SettingKey, value: unknown) => invoke<SettingsReply>("settings:set", { key, value }),
  importLegacySettings: (legacy: LegacySettingsImport) => (
    invoke<{ imported: boolean; values: SettingsValues }>("settings:import-legacy", legacy)
  ),

  downloadModel: (modelId: string) => invoke<Partial<RuntimeInfo>>("models:download", { modelId }),
  deleteModel: (modelId: string) => invoke<Partial<RuntimeInfo>>("models:delete", { modelId }),
  transcribeAudio: (request: TranscriptionRequest) => invoke<TranscriptionResult>("engine:transcribe-audio", request),

  openTranscriptText: (request: { title: string; text: string }) => (
    invoke<{ filePath: string }>("transcript:open-text", request)
  ),
  deleteTranscriptText: (request: { title: string; filePath?: string }) => (
    invoke<{ deleted: boolean; filePath: string }>("transcript:delete-text", request)
  ),

  setRecording: (active: boolean) => invoke<{ isRecording: boolean }>("recording:set", { active }),
  toggleRecording: () => invoke<{ isRecording: boolean }>("recording:toggle"),
  sendWaveformFrame: (frame: number[]) => window.asrpro?.send?.("recording:waveform-frame", frame),

  onRecordingState: (callback: (state: RecordingStateEvent) => void) => subscribe("recording:state", callback),
  onEngineState: (callback: (state: EngineRuntimeState) => void) => subscribe("engine:state", callback),
  onSettingsChanged: (callback: (event: { changed: SettingKey[]; values: SettingsValues }) => void) => (
    subscribe("settings:changed", callback)
  ),
};
