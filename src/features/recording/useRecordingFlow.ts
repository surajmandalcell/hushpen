import { useCallback, useEffect, useRef, useState, type Dispatch, type SetStateAction } from "react";
import { defaultAudioInputId } from "../../lib/defaults";
import { writeTextToClipboard } from "../../lib/clipboard";
import { getErrorMessage } from "../../lib/errors";
import { createTranscriptHistoryRow } from "../../lib/history";
import { readBlobAsDataUrl } from "../../lib/wav";
import { audioRecordingService } from "../../services/audioRecording";
import type { RecordingStatus } from "../../types/app";
import type { TranscriptHistoryRow } from "../../types/history";
import type { RuntimeInfo } from "../../types/runtime";
import { useMicrophoneWaveform } from "./useMicrophoneWaveform";
import type { Transcribe } from "./useTranscriber";

interface UseRecordingFlowOptions {
  selectedAudioInputId: string;
  selectedModel: string;
  autoCopyTranscripts: boolean;
  transcribe: Transcribe;
  addHistoryRow: (row: TranscriptHistoryRow) => void;
  setRuntimeInfo: Dispatch<SetStateAction<RuntimeInfo | null>>;
}

export function useRecordingFlow({
  selectedAudioInputId,
  selectedModel,
  autoCopyTranscripts,
  transcribe,
  addHistoryRow,
  setRuntimeInfo,
}: UseRecordingFlowOptions) {
  const [isRecording, setIsRecording] = useState(false);
  const [status, setStatus] = useState<RecordingStatus>("idle");
  const [error, setError] = useState<string | null>(null);
  const [durationSeconds, setDurationSeconds] = useState(0);
  const recordingStartedAtRef = useRef<number | null>(null);
  const recordingTransitionRef = useRef<"starting" | "stopping" | null>(null);
  useMicrophoneWaveform(isRecording);

  const syncRecordingBridge = useCallback(async (active: boolean) => {
    const api = window.asrpro;
    if (!api?.setRecording) return;

    try {
      const state = await api.setRecording(active);
      setRuntimeInfo((current) => (current ? { ...current, isRecording: state.isRecording } : current));
    } catch {
      setRuntimeInfo((current) => (current ? { ...current, isRecording: active } : current));
    }
  }, [setRuntimeInfo]);

  const startRecordingFlow = useCallback(async (syncBridge = true) => {
    if (recordingTransitionRef.current || audioRecordingService.isRecording()) {
      return;
    }

    recordingTransitionRef.current = "starting";
    setStatus("starting");
    setError(null);

    try {
      await audioRecordingService.startRecording({
        sampleRate: 16000,
        channelCount: 1,
        deviceId: selectedAudioInputId === defaultAudioInputId ? undefined : selectedAudioInputId,
        echoCancellation: true,
        noiseSuppression: true,
      });
      const startedAt = Date.now();
      recordingStartedAtRef.current = startedAt;
      setDurationSeconds(0);
      setIsRecording(true);
      setStatus("recording");
      setRuntimeInfo((current) => (current ? { ...current, isRecording: true } : current));
      if (syncBridge) {
        await syncRecordingBridge(true);
      }
    } catch (caught) {
      const message = getErrorMessage(caught);
      setIsRecording(false);
      setStatus("error");
      setError(message);
      setRuntimeInfo((current) => (current ? { ...current, isRecording: false } : current));
      if (syncBridge) {
        await syncRecordingBridge(false);
      }
    } finally {
      recordingTransitionRef.current = null;
    }
  }, [selectedAudioInputId, setRuntimeInfo, syncRecordingBridge]);

  const stopRecordingFlow = useCallback(async (syncBridge = true) => {
    if (recordingTransitionRef.current === "stopping") {
      return;
    }

    const wasRecording = audioRecordingService.isRecording();
    const startedAt = recordingStartedAtRef.current ?? Date.now();
    const elapsedSeconds = Math.max(0, Math.round((Date.now() - startedAt) / 1000));

    recordingTransitionRef.current = "stopping";
    setIsRecording(false);
    setDurationSeconds(elapsedSeconds);
    setStatus(wasRecording ? "preparing-engine" : "idle");
    setRuntimeInfo((current) => (current ? { ...current, isRecording: false } : current));

    let recordingUrl: string | undefined;

    try {
      if (syncBridge) {
        await syncRecordingBridge(false);
      }

      if (!wasRecording) {
        return;
      }

      const audioBlob = await audioRecordingService.stopRecording();
      if (!audioBlob || audioBlob.size === 0) {
        throw new Error("No audio was captured");
      }

      recordingUrl = await readBlobAsDataUrl(audioBlob);
      setStatus("preparing-engine");
      setStatus("transcribing");
      const result = await transcribe(audioBlob);
      const text = typeof result === "string" ? result : result?.text;
      if (!text || !text.trim()) {
        throw new Error("No transcription text returned");
      }
      const normalizedText = text.replace(/\s+/g, " ").trim();

      addHistoryRow(createTranscriptHistoryRow({
        text: normalizedText,
        model: selectedModel,
        durationSeconds: elapsedSeconds,
        startedAt,
        recordingUrl,
      }));
      if (autoCopyTranscripts) {
        writeTextToClipboard(normalizedText);
      }
      setStatus("idle");
      setError(null);
    } catch (caught) {
      const message = getErrorMessage(caught);
      setStatus("error");
      setError(message);
      addHistoryRow({
        id: `dictation-error-${startedAt}`,
        title: "Recording failed to transcribe",
        text: message,
        kind: "Dictation",
        model: selectedModel,
        durationSeconds: elapsedSeconds,
        createdAt: Date.now(),
        status: "failed",
        error: message,
        recordingUrl,
      });
    } finally {
      recordingStartedAtRef.current = null;
      recordingTransitionRef.current = null;
    }
  }, [addHistoryRow, autoCopyTranscripts, selectedModel, setRuntimeInfo, syncRecordingBridge, transcribe]);

  const setRecording = useCallback((active: boolean) => {
    if (active) {
      void startRecordingFlow(true);
    } else {
      void stopRecordingFlow(true);
    }
  }, [startRecordingFlow, stopRecordingFlow]);

  const applyBridgeState = useCallback((nextIsRecording: boolean) => {
    if (recordingTransitionRef.current) {
      setIsRecording(nextIsRecording);
      return;
    }

    if (nextIsRecording) {
      void startRecordingFlow(false);
    } else {
      void stopRecordingFlow(false);
    }
  }, [startRecordingFlow, stopRecordingFlow]);

  const restoreFromRuntime = useCallback((runtimeIsRecording: boolean) => {
    if (runtimeIsRecording) {
      void startRecordingFlow(false);
    } else if (!recordingTransitionRef.current && !audioRecordingService.isRecording()) {
      setIsRecording(false);
    }
  }, [startRecordingFlow]);

  useEffect(() => {
    if (!isRecording || status !== "recording") {
      return undefined;
    }

    const updateDuration = () => {
      const startedAt = recordingStartedAtRef.current;
      if (!startedAt) return;
      setDurationSeconds(Math.max(0, Math.floor((Date.now() - startedAt) / 1000)));
    };

    updateDuration();
    const interval = window.setInterval(updateDuration, 1000);
    return () => window.clearInterval(interval);
  }, [isRecording, status]);

  useEffect(() => () => {
    if (audioRecordingService.isRecording()) {
      void audioRecordingService.stopRecording();
    }
  }, []);

  return {
    isRecording,
    status,
    error,
    durationSeconds,
    setRecording,
    applyBridgeState,
    restoreFromRuntime,
  };
}

export type RecordingFlow = ReturnType<typeof useRecordingFlow>;
