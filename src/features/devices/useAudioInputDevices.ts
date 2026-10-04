import { useCallback, useEffect, useMemo, useState } from "react";
import { buildAudioInputDeviceOptions } from "../../lib/audioDevices";
import {
  defaultAudioInputId,
  defaultAudioInputLabel,
  defaultAudioInputOptions,
} from "../../lib/defaults";
import { bridge } from "../../lib/bridge";
import type { RuntimeInfo } from "../../types/runtime";
import type { AudioInputDeviceOption } from "../../types/audio";

function persistAudioInputId(deviceId: string) {
  if (bridge.isAvailable()) {
    bridge.setSetting("recording.audioInputId", deviceId).catch(() => {});
  }
}

export function useAudioInputDevices() {
  const [devices, setDevices] = useState<AudioInputDeviceOption[]>(defaultAudioInputOptions);
  const [selectedDeviceId, setSelectedDeviceId] = useState(defaultAudioInputId);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const mediaDevices = navigator.mediaDevices;

    if (!mediaDevices?.enumerateDevices) {
      setDevices(defaultAudioInputOptions);
      setError("Microphone list is not available.");
      setSelectedDeviceId(defaultAudioInputId);
      persistAudioInputId(defaultAudioInputId);
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const found = await mediaDevices.enumerateDevices();
      const nextOptions = buildAudioInputDeviceOptions(found);

      setDevices(nextOptions);
      setSelectedDeviceId((current) => {
        const nextDeviceId = nextOptions.some((device) => device.id === current) ? current : defaultAudioInputId;
        if (nextDeviceId !== current) {
          persistAudioInputId(nextDeviceId);
        }
        return nextDeviceId;
      });
    } catch {
      setDevices(defaultAudioInputOptions);
      setError("Microphone list could not be loaded.");
      setSelectedDeviceId(defaultAudioInputId);
      persistAudioInputId(defaultAudioInputId);
    } finally {
      setLoading(false);
    }
  }, []);

  const select = useCallback((deviceId: string) => {
    setSelectedDeviceId(deviceId);
    persistAudioInputId(deviceId);
  }, []);

  const applyRuntimeState = useCallback((state: RuntimeInfo) => {
    if (!state.audioInputId) return;
    setSelectedDeviceId(state.audioInputId);
    // The saved device may have been unplugged; refresh drops it in favor of the default.
    void refresh();
  }, [refresh]);

  const selectedLabel = useMemo(() => (
    devices.find((device) => device.id === selectedDeviceId)?.label ?? defaultAudioInputLabel
  ), [devices, selectedDeviceId]);

  useEffect(() => {
    void refresh();

    const mediaDevices = navigator.mediaDevices;
    if (!mediaDevices?.addEventListener) {
      return undefined;
    }

    const handleDeviceChange = () => {
      void refresh();
    };

    mediaDevices.addEventListener("devicechange", handleDeviceChange);
    return () => mediaDevices.removeEventListener("devicechange", handleDeviceChange);
  }, [refresh]);

  return { devices, selectedDeviceId, selectedLabel, loading, error, refresh, select, applyRuntimeState };
}

export type AudioInputDevices = ReturnType<typeof useAudioInputDevices>;
