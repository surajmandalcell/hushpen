import { useEffect, useRef, type Dispatch, type SetStateAction } from "react";
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
    const api = window.asrpro;
    if (!api) return undefined;

    if (api.getAppInfo) {
      Promise.resolve(api.getAppInfo()).then((info) => {
        if (!info) return;
        setAppInfo((current) => ({
          name: info.name || current.name,
          version: info.version || current.version,
        }));
      }).catch(() => {});
    }

    if (api.getRuntimeState && !runtimeStateLoadedRef.current) {
      runtimeStateLoadedRef.current = true;
      Promise.resolve(api.getRuntimeState()).then((state) => {
        if (!state) return;
        setRuntimeInfo(state);
        applyModelState(state);
        applySettingsState(state);
        restoreRecordingState(state.isRecording);
      }).catch(() => {
        runtimeStateLoadedRef.current = false;
      });
    }

    const unsubscribeRecording = api.onRecordingState?.((state) => {
      setRuntimeInfo((current) => (current ? { ...current, isRecording: state.isRecording } : current));
      applyRecordingState(state.isRecording);
    });

    const unsubscribeEngine = api.onEngineState?.((engineState) => {
      setRuntimeInfo((current) => (current ? { ...current, engine: engineState } : { isRecording: false, engine: engineState }));
      if (engineState.modelId && engineState.status === "downloading" && typeof engineState.progress === "number") {
        updateModelDownloadProgress(engineState.modelId, engineState.progress);
      } else if (engineState.modelId && engineState.status !== "downloading") {
        clearModelDownloadProgress(engineState.modelId);
      }
    });

    return () => {
      unsubscribeRecording?.();
      unsubscribeEngine?.();
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
