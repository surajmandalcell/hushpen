import { isAppError } from "./bridge";
import type { ErrorCode } from "../types/contracts";

const fallbackMessage = "Recording failed";
const downloadFailedMessage = "Whisper model download failed. Check your connection and try again.";
const engineUnavailableMessage = "Native Whisper engine could not load. Reinstall dependencies, then restart ASR Pro.";

// Messages are keyed by error code, never by the English detail text. This
// table becomes the `errors.<CODE>` catalog keys when the i18n catalogs land.
const messagesByCode: Partial<Record<ErrorCode, string>> = {
  INTERNAL: "Something went wrong. Try again.",
  INVALID_ARGUMENT: "That request was not valid. Try again.",
  FORBIDDEN_SENDER: "That request was blocked.",
  CANCELLED: "The action was cancelled.",
  OFFLINE: "Failed to load.",
  ENGINE_LOAD_FAILED: engineUnavailableMessage,
  ENGINE_CRASHED: "Native Whisper engine stopped. Try again.",
  ENGINE_NOT_READY: "Native Whisper engine needs restart. Restart ASR Pro, then try again.",
  MODEL_DOWNLOAD_FAILED: downloadFailedMessage,
  MODEL_DOWNLOAD_STALLED: downloadFailedMessage,
  MODEL_CHECKSUM_MISMATCH: downloadFailedMessage,
  MODEL_MISSING: "The speech model is not installed. Download it from Models library.",
  MODEL_IN_USE: "That model is in use. Try again when it is idle.",
  SETTINGS_INVALID: "That setting could not be saved.",
};

const titlesByCode: Partial<Record<ErrorCode, string>> = {
  ENGINE_NOT_READY: "Engine needs restart",
  ENGINE_LOAD_FAILED: "Engine unavailable",
  ENGINE_CRASHED: "Engine unavailable",
  MODEL_DOWNLOAD_FAILED: "Engine unavailable",
  MODEL_DOWNLOAD_STALLED: "Engine unavailable",
  MODEL_CHECKSUM_MISMATCH: "Engine unavailable",
  MODEL_MISSING: "Engine unavailable",
  OFFLINE: "Engine unavailable",
};

export function getMessageForCode(code: ErrorCode) {
  return messagesByCode[code] ?? messagesByCode.INTERNAL ?? fallbackMessage;
}

export function getErrorCode(error: unknown): ErrorCode | undefined {
  return isAppError(error) ? error.code : undefined;
}

export function getErrorMessage(error: unknown) {
  if (isAppError(error)) return getMessageForCode(error.code);
  return error instanceof Error && error.message ? error.message : fallbackMessage;
}

export function getRecordingErrorTitle(code?: ErrorCode) {
  return (code && titlesByCode[code]) || "Recording failed";
}
