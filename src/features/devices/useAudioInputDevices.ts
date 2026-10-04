import { useCallback, useEffect, useMemo, useState } from "react";
import { buildAudioInputDeviceOptions } from "../../lib/audioDevices";
import {
  defaultAudioInputId,
  defaultAudioInputLabel,
  defaultAudioInputOptions,
} from "../../lib/defaults";
import { loadSelectedAudioInputId, saveSelectedAudioInputId } from "../../lib/storage";
import type { AudioInputDeviceOption } from "../../types/audio";

export function useAudioInputDevices() {
  const [devices, setDevices] = useState<AudioInputDeviceOption[]>(defaultAudioInputOptions);
  const [selectedDeviceId, setSelectedDeviceId] = useState(loadSelectedAudioInputId);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const mediaDevices = navigator.mediaDevices;

    if (!mediaDevices?.enumerateDevices) {
      setDevices(defaultAudioInputOptions);
      setError("Microphone list is not available.");
      setSelectedDeviceId(defaultAudioInputId);
      saveSelectedAudioInputId(defaultAudioInputId);
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
          saveSelectedAudioInputId(nextDeviceId);
        }
        return nextDeviceId;
      });
    } catch {
      setDevices(defaultAudioInputOptions);
      setError("Microphone list could not be loaded.");
      setSelectedDeviceId(defaultAudioInputId);
      saveSelectedAudioInputId(defaultAudioInputId);
    } finally {
      setLoading(false);
    }
  }, []);

  const select = useCallback((deviceId: string) => {
    setSelectedDeviceId(deviceId);
    saveSelectedAudioInputId(deviceId);
  }, []);

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

  return { devices, selectedDeviceId, selectedLabel, loading, error, refresh, select };
}

export type AudioInputDevices = ReturnType<typeof useAudioInputDevices>;
