import { useCallback, useMemo, useRef, useState, type Dispatch, type SetStateAction } from "react";
import { bridge } from "../../lib/bridge";
import { defaultModelName, modelIdsByName } from "../../lib/defaults";
import { getErrorMessage } from "../../lib/errors";
import { clampNumber } from "../../lib/math";
import { getRuntimeModels, mergeRuntimeInfo } from "../../lib/runtime";
import { normalizeSelectedModelName } from "../../lib/storage";
import type { RuntimeInfo } from "../../types/runtime";

interface UseModelLibraryOptions {
  runtimeInfo: RuntimeInfo | null;
  setRuntimeInfo: Dispatch<SetStateAction<RuntimeInfo | null>>;
}

export function useModelLibrary({ runtimeInfo, setRuntimeInfo }: UseModelLibraryOptions) {
  const [selectedModel, setSelectedModel] = useState(defaultModelName);
  const [busyModelIds, setBusyModelIds] = useState<Set<string>>(() => new Set());
  const [progressById, setProgressById] = useState<Record<string, number>>({});
  const [error, setError] = useState<string | null>(null);
  const busyModelIdsRef = useRef<Set<string>>(new Set());

  const models = useMemo(() => getRuntimeModels(runtimeInfo?.models), [runtimeInfo?.models]);
  const selectedModelId = useMemo(() => (
    models.find((model) => model.displayName === selectedModel)?.id ?? modelIdsByName[selectedModel] ?? "whisper-base-en"
  ), [models, selectedModel]);

  const selectModel = useCallback((modelName: string) => {
    setSelectedModel(modelName);
    const modelId = models.find((model) => model.displayName === modelName)?.id ?? modelIdsByName[modelName];
    if (modelId && bridge.isAvailable()) {
      bridge.setSetting("transcription.modelId", modelId).catch(() => {});
    }
  }, [models]);

  const applyRuntimeState = useCallback((state: RuntimeInfo) => {
    const nextModels = getRuntimeModels(state.models);
    const nextSelectedModel = normalizeSelectedModelName(state.defaultModelId, nextModels)
      ?? normalizeSelectedModelName(state.defaultModel, nextModels)
      ?? defaultModelName;
    setSelectedModel(nextSelectedModel);
  }, []);

  const beginModelAction = useCallback((modelId: string) => {
    if (busyModelIdsRef.current.has(modelId)) return false;
    const nextIds = new Set(busyModelIdsRef.current);
    nextIds.add(modelId);
    busyModelIdsRef.current = nextIds;
    setBusyModelIds(nextIds);
    return true;
  }, []);

  const endModelAction = useCallback((modelId: string) => {
    if (!busyModelIdsRef.current.has(modelId)) return;
    const nextIds = new Set(busyModelIdsRef.current);
    nextIds.delete(modelId);
    busyModelIdsRef.current = nextIds;
    setBusyModelIds(nextIds);
  }, []);

  const updateDownloadProgress = useCallback((modelId: string, progress: number) => {
    const nextProgress = clampNumber(progress, 0, 100);
    setProgressById((current) => (
      current[modelId] === nextProgress ? current : { ...current, [modelId]: nextProgress }
    ));
  }, []);

  const clearDownloadProgress = useCallback((modelId: string) => {
    setProgressById((current) => {
      if (!(modelId in current)) return current;
      const next = { ...current };
      delete next[modelId];
      return next;
    });
  }, []);

  const downloadModel = useCallback(async (modelId: string) => {
    if (!bridge.isAvailable()) return;
    if (!beginModelAction(modelId)) return;

    updateDownloadProgress(modelId, 0);
    setError(null);

    try {
      const state = await bridge.downloadModel(modelId);
      setRuntimeInfo((current) => mergeRuntimeInfo(current, state));
    } catch (caught) {
      setError(getErrorMessage(caught));
    } finally {
      endModelAction(modelId);
      clearDownloadProgress(modelId);
    }
  }, [beginModelAction, clearDownloadProgress, endModelAction, setRuntimeInfo, updateDownloadProgress]);

  const deleteModel = useCallback(async (modelId: string) => {
    if (!bridge.isAvailable()) return;
    if (!beginModelAction(modelId)) return;

    setError(null);

    try {
      const state = await bridge.deleteModel(modelId);
      setRuntimeInfo((current) => mergeRuntimeInfo(current, state));
    } catch (caught) {
      setError(getErrorMessage(caught));
    } finally {
      endModelAction(modelId);
      clearDownloadProgress(modelId);
    }
  }, [beginModelAction, clearDownloadProgress, endModelAction, setRuntimeInfo]);

  return {
    selectedModel,
    selectedModelId,
    models,
    busyModelIds,
    progressById,
    error,
    selectModel,
    applyRuntimeState,
    updateDownloadProgress,
    clearDownloadProgress,
    downloadModel,
    deleteModel,
  };
}

export type ModelLibrary = ReturnType<typeof useModelLibrary>;
