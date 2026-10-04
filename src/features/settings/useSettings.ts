import { useCallback, useMemo, useRef, useState, type Dispatch, type SetStateAction } from "react";
import {
  defaultAutoCopyTranscripts,
  defaultTextEditorId,
  defaultTextEditorOptions,
} from "../../lib/defaults";
import {
  mergeOverlaySettings,
  normalizeAutoCopyTranscripts,
  normalizeLaunchAtStartup,
  normalizeOverlayPlacement,
  normalizeTextEditorId,
  normalizeTextEditorOptions,
} from "../../lib/runtime";
import { bridge } from "../../lib/bridge";
import type { RuntimeInfo } from "../../types/runtime";
import type { OverlayPlacement, TextEditorOption } from "../../types/settings";

interface UseSettingsOptions {
  setRuntimeInfo: Dispatch<SetStateAction<RuntimeInfo | null>>;
}

export function useSettings({ setRuntimeInfo }: UseSettingsOptions) {
  const [overlayPlacement, setOverlayPlacement] = useState<OverlayPlacement>("top");
  const [textEditorOptions, setTextEditorOptions] = useState<TextEditorOption[]>(defaultTextEditorOptions);
  const [selectedTextEditorId, setSelectedTextEditorId] = useState(defaultTextEditorId);
  const [autoCopyTranscripts, setAutoCopyTranscripts] = useState(defaultAutoCopyTranscripts);
  const [launchAtStartup, setLaunchAtStartup] = useState(false);
  const overlayPlacementTouchedRef = useRef(false);

  const selectedTextEditorLabel = useMemo(() => (
    textEditorOptions.find((editor) => editor.id === selectedTextEditorId)?.label ?? defaultTextEditorOptions[0].label
  ), [selectedTextEditorId, textEditorOptions]);

  const applyRuntimeState = useCallback((state: RuntimeInfo) => {
    const nextTextEditorOptions = normalizeTextEditorOptions(state.textEditors);
    setTextEditorOptions(nextTextEditorOptions);
    setSelectedTextEditorId(normalizeTextEditorId(state.defaultTextEditor, nextTextEditorOptions));
    setAutoCopyTranscripts(normalizeAutoCopyTranscripts(state.autoCopyTranscripts));
    setLaunchAtStartup(normalizeLaunchAtStartup(state.startup, state.launchAtStartup));
    if (!overlayPlacementTouchedRef.current) {
      setOverlayPlacement(normalizeOverlayPlacement(state.overlaySettings?.placement));
    }
  }, []);

  const changeTextEditor = useCallback((editorId: string) => {
    const normalizedEditorId = normalizeTextEditorId(editorId, textEditorOptions);
    setSelectedTextEditorId(normalizedEditorId);
    setRuntimeInfo((current) => (current ? { ...current, defaultTextEditor: normalizedEditorId } : current));

    bridge.setSetting("editor.defaultTextEditor", normalizedEditorId).then(({ values }) => {
      const nextEditorId = normalizeTextEditorId(values["editor.defaultTextEditor"], textEditorOptions);
      setSelectedTextEditorId(nextEditorId);
      setRuntimeInfo((current) => (current ? { ...current, defaultTextEditor: nextEditorId } : current));
    }).catch(() => {});
  }, [setRuntimeInfo, textEditorOptions]);

  const changeAutoCopyTranscripts = useCallback((enabled: boolean) => {
    setAutoCopyTranscripts(enabled);
    setRuntimeInfo((current) => (current ? { ...current, autoCopyTranscripts: enabled } : current));

    bridge.setSetting("output.autoCopy", enabled).then(({ values }) => {
      const nextAutoCopy = normalizeAutoCopyTranscripts(values["output.autoCopy"]);
      setAutoCopyTranscripts(nextAutoCopy);
      setRuntimeInfo((current) => (current ? { ...current, autoCopyTranscripts: nextAutoCopy } : current));
    }).catch(() => {});
  }, [setRuntimeInfo]);

  const changeLaunchAtStartup = useCallback((enabled: boolean) => {
    setLaunchAtStartup(enabled);
    setRuntimeInfo((current) => (current ? {
      ...current,
      launchAtStartup: enabled,
      startup: current.startup ? { ...current.startup, enabled } : current.startup,
    } : current));

    bridge.setSetting("startup.launchAtLogin", enabled).then(({ values, startup }) => {
      const nextLaunchAtStartup = normalizeLaunchAtStartup(startup, values["startup.launchAtLogin"] === true);
      setLaunchAtStartup(nextLaunchAtStartup);
      setRuntimeInfo((current) => (current ? {
        ...current,
        launchAtStartup: nextLaunchAtStartup,
        startup: startup ?? current.startup,
      } : current));
    }).catch(() => {
      setLaunchAtStartup(!enabled);
      setRuntimeInfo((current) => (current ? {
        ...current,
        launchAtStartup: !enabled,
        startup: current.startup ? { ...current.startup, enabled: !enabled } : current.startup,
      } : current));
    });
  }, [setRuntimeInfo]);

  const changeOverlayPlacement = useCallback((placement: OverlayPlacement) => {
    overlayPlacementTouchedRef.current = true;
    setOverlayPlacement(placement);
    setRuntimeInfo((current) => mergeOverlaySettings(current, { placement, customBounds: null }));

    bridge.setSetting("overlay.placement", placement).then(({ values }) => {
      const nextPlacement = normalizeOverlayPlacement(values["overlay.placement"]);
      setOverlayPlacement(nextPlacement);
      setRuntimeInfo((current) => mergeOverlaySettings(current, { placement: nextPlacement, customBounds: null }));
    }).catch(() => {});
  }, [setRuntimeInfo]);

  return {
    overlayPlacement,
    textEditorOptions,
    selectedTextEditorId,
    selectedTextEditorLabel,
    autoCopyTranscripts,
    launchAtStartup,
    applyRuntimeState,
    changeTextEditor,
    changeAutoCopyTranscripts,
    changeLaunchAtStartup,
    changeOverlayPlacement,
  };
}

export type Settings = ReturnType<typeof useSettings>;
