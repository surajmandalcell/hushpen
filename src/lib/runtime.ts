import type { EngineModelInfo } from "../types/engine";
import type { RuntimeInfo } from "../types/runtime";
import type { OverlayPlacement, OverlaySettings, StartupSettings, TextEditorOption } from "../types/settings";
import { defaultAutoCopyTranscripts, defaultTextEditorId, defaultTextEditorOptions, fallbackModelCards } from "./defaults";

export function getRuntimeModels(models?: EngineModelInfo[]) {
  return models?.length ? models : fallbackModelCards;
}

export function mergeRuntimeInfo(current: RuntimeInfo | null, next?: Partial<RuntimeInfo> | null): RuntimeInfo | null {
  if (!next) return current;

  return {
    ...(current ?? { isRecording: false }),
    ...next,
    isRecording: next.isRecording ?? current?.isRecording ?? false,
  };
}

export function normalizeTextEditorId(editorId: unknown, options: TextEditorOption[] = defaultTextEditorOptions) {
  const normalized = typeof editorId === "string" ? editorId : defaultTextEditorId;
  return options.some((option) => option.id === normalized) ? normalized : defaultTextEditorId;
}

export function normalizeTextEditorOptions(options: unknown): TextEditorOption[] {
  if (!Array.isArray(options)) return defaultTextEditorOptions;

  const normalized: TextEditorOption[] = [];
  for (const option of options) {
    if (!option || typeof option !== "object") continue;
    const candidate = option as Partial<TextEditorOption>;
    if (!candidate.id || !candidate.label) continue;
    normalized.push({
      id: String(candidate.id),
      label: String(candidate.label),
      detail: candidate.detail ? String(candidate.detail) : "",
      iconDataUrl: typeof candidate.iconDataUrl === "string" ? candidate.iconDataUrl : undefined,
    });
  }

  return normalized.length ? normalized : defaultTextEditorOptions;
}

export function normalizeOverlayPlacement(value: unknown): OverlayPlacement {
  return value === "bottom" ? "bottom" : "top";
}

export function normalizeAutoCopyTranscripts(value: unknown) {
  return typeof value === "boolean" ? value : defaultAutoCopyTranscripts;
}

export function normalizeLaunchAtStartup(startup?: StartupSettings, fallback?: boolean) {
  if (typeof startup?.enabled === "boolean") return startup.enabled;
  return typeof fallback === "boolean" ? fallback : false;
}

export function mergeOverlaySettings(runtimeInfo: RuntimeInfo | null, overlaySettings: OverlaySettings): RuntimeInfo | null {
  if (!runtimeInfo) return runtimeInfo;
  return {
    ...runtimeInfo,
    overlaySettings,
  };
}
