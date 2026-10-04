import { useCallback } from "react";
import { AppError, bridge } from "../../lib/bridge";
import { createTranscriptionAudioPayload } from "../../lib/wav";

export function useTranscriber(selectedModelId: string) {
  return useCallback(async (audioBlob: Blob) => {
    if (!bridge.isAvailable()) {
      throw new AppError("ENGINE_NOT_READY");
    }

    const payload = await createTranscriptionAudioPayload(audioBlob);
    return bridge.transcribeAudio({
      ...payload,
      modelId: selectedModelId,
    });
  }, [selectedModelId]);
}

export type Transcribe = ReturnType<typeof useTranscriber>;
