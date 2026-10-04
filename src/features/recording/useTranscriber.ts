import { useCallback } from "react";
import { createTranscriptionAudioPayload } from "../../lib/wav";

export function useTranscriber(selectedModelId: string) {
  return useCallback(async (audioBlob: Blob) => {
    const transcribeAudio = window.asrpro?.transcribeAudio;
    if (!transcribeAudio) {
      throw new Error("Native Whisper engine is not available.");
    }

    const payload = await createTranscriptionAudioPayload(audioBlob);
    return transcribeAudio({
      ...payload,
      modelId: selectedModelId,
    });
  }, [selectedModelId]);
}

export type Transcribe = ReturnType<typeof useTranscriber>;
