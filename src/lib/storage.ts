import type { EngineModelInfo } from "../types/engine";
import { defaultAudioInputId, fallbackModelCards } from "./defaults";

const audioInputDeviceStorageKey = "asrpro.audioInputDevice.v1";
const selectedModelStorageKey = "asrpro.selectedModel.v1";

export function loadSelectedAudioInputId() {
  try {
    const stored = window.localStorage.getItem(audioInputDeviceStorageKey);
    return stored && stored.trim() ? stored : defaultAudioInputId;
  } catch {
    return defaultAudioInputId;
  }
}

export function saveSelectedAudioInputId(deviceId: string) {
  try {
    window.localStorage.setItem(audioInputDeviceStorageKey, deviceId);
  } catch {
    // Local storage failures should not block recording.
  }
}

export function normalizeSelectedModelName(value: unknown, models: EngineModelInfo[] = fallbackModelCards) {
  if (typeof value !== "string" || !value.trim()) return undefined;

  const normalized = value.trim();
  const byName = models.find((model) => model.displayName === normalized);
  if (byName) return byName.displayName;

  const byId = models.find((model) => model.id === normalized);
  return byId?.displayName;
}

export function loadSelectedModelName(models: EngineModelInfo[] = fallbackModelCards) {
  try {
    return normalizeSelectedModelName(window.localStorage.getItem(selectedModelStorageKey), models);
  } catch {
    return undefined;
  }
}

export function saveSelectedModelName(modelName: string) {
  try {
    window.localStorage.setItem(selectedModelStorageKey, modelName);
  } catch {
    // Local storage failures should not block recognition.
  }
}
