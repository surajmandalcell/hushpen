import { useEffect, useRef, type Dispatch, type SetStateAction } from "react";
import { bridge } from "../../lib/bridge";
import { readLegacyLocalStorageSettings } from "../../lib/storage";
import type { AppInfo } from "../../types/app";
import type { RuntimeInfo } from "../../types/runtime";

interface UseRuntimeBridgeOptions {
  setRuntimeInfo: Dispatch<SetStateAction<RuntimeInfo | null>>;
  setAppInfo: Dispatch<SetStateAction<AppInfo>>;
  applyModelState: (state: RuntimeInfo) => void;
  applySettingsState: (state: RuntimeInfo) => void;
  restoreRecordingState: (isRecording: boolean) => void;
  applyRecordingState: (isRecording: boolean) => void;
  updateModelDownloadProgress: (modelId: string, progress: number) => void;
  clearModelDownloadProgress: (modelId: string) => void;
}

export function useRuntimeBridge({
  setRuntimeInfo,
  setAppInfo,
  applyModelState,
  applySettingsState,
  restoreRecordingState,
  applyRecordingState,
  updateModelDownloadProgress,
  clearModelDownloadProgress,
}: UseRuntimeBridgeOptions) {
  const runtimeStateLoadedRef = useRef(false);

  useEffect(() => {
    if (!bridge.isAvailable()) return undefined;

    bridge.getAppInfo().then((info) => {
      if (!info) return;
      setAppInfo((current) => ({
        name: info.name || current.name,
        version: info.version || current.version,
      }));
    }).catch(() => {});

    if (!runtimeStateLoadedRef.current) {
      runtimeStateLoadedRef.current = true;
      const legacy = readLegacyLocalStorageSettings();
      const importLegacy = legacy.selectedModelName || legacy.audioInputId
        ? bridge.getSettings()
          .then((settings) => settings.values["migrations.legacyLocalStorage"] === true, () => false)
          .then((closed) => (closed ? undefined : bridge.importLegacySettings(legacy)))
          .catch(() => undefined)
        : Promise.resolve(undefined);
      importLegacy.then(() => bridge.getRuntimeState()).then((state) => {
        if (!state) return;
        setRuntimeInfo(state);
        applyModelState(state);
        applySettingsState(state);
        restoreRecordingState(state.isRecording);
      }).catch(() => {
        runtimeStateLoadedRef.current = false;
      });
    }

    const unsubscribeRecording = bridge.onRecordingState((state) => {
      setRuntimeInfo((current) => (current ? { ...current, isRecording: state.isRecording } : current));
      applyRecordingState(state.isRecording);
    });

    const unsubscribeEngine = bridge.onEngineState((engineState) => {
      setRuntimeInfo((current) => (current ? { ...current, engine: engineState } : { isRecording: false, engine: engineState }));
      if (engineState.modelId && engineState.status === "downloading" && typeof engineState.progress === "number") {
        updateModelDownloadProgress(engineState.modelId, engineState.progress);
      } else if (engineState.modelId && engineState.status !== "downloading") {
        clearModelDownloadProgress(engineState.modelId);
      }
    });

    return () => {
      unsubscribeRecording();
      unsubscribeEngine();
    };
  }, [
    applyModelState,
    applyRecordingState,
    applySettingsState,
    clearModelDownloadProgress,
    restoreRecordingState,
    setAppInfo,
    setRuntimeInfo,
    updateModelDownloadProgress,
  ]);
}
