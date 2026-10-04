import type { EngineModelInfo } from "../types/engine";
import { fallbackModelCards } from "./defaults";

const audioInputDeviceStorageKey = "asrpro.audioInputDevice.v1";
const selectedModelStorageKey = "asrpro.selectedModel.v1";

export interface LegacyLocalStorageSettings {
  selectedModelName?: string;
  audioInputId?: string;
}

// The 1.x renderer kept these two choices in localStorage. They are only read
// (never written or removed) so the main process can import them once.
export function readLegacyLocalStorageSettings(): LegacyLocalStorageSettings {
  const legacy: LegacyLocalStorageSettings = {};
  try {
    const model = window.localStorage.getItem(selectedModelStorageKey);
    if (model && model.trim()) legacy.selectedModelName = model.trim();
    const device = window.localStorage.getItem(audioInputDeviceStorageKey);
    if (device && device.trim()) legacy.audioInputId = device.trim();
  } catch {
    // Blocked storage means there is nothing to import.
  }
  return legacy;
}

export function normalizeSelectedModelName(value: unknown, models: EngineModelInfo[] = fallbackModelCards) {
  if (typeof value !== "string" || !value.trim()) return undefined;

  const normalized = value.trim();
  const byName = models.find((model) => model.displayName === normalized);
  if (byName) return byName.displayName;

  const byId = models.find((model) => model.id === normalized);
  return byId?.displayName;
}
