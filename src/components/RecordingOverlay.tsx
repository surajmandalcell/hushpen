import React, { useEffect, useState } from "react";
import { Mic, MicOff, Square, Loader2 } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { useAudioRecording } from "../hooks/useAudioRecording";
import { useRecording } from "../services/recordingManager";
import { PalModal, PalModalHeader, PalModalContent, PalModalFooter, PalButton, PalPanel } from "./palantirui";

export interface RecordingOverlayProps {
  isActive: boolean;
  isTranscribing?: boolean;
  statusText?: string;
}

const RecordingOverlay: React.FC<RecordingOverlayProps> = ({
  isActive,
  isTranscribing = false,
  statusText = "Listening...",
}) => {
  const [showControls, setShowControls] = useState(false);
  const [mounted, setMounted] = useState(false);
  const [isTranscribingLocal, setIsTranscribingLocal] = useState(false);

  // Audio recording hook
  const {
    state: audioState,
    startRecording,
    stopRecording,
  } = useAudioRecording();

  // Recording management hook
  const {
    state: recordingState,
    start,
    stop,
    cancel,
    transcribeFile,
  } = useRecording();

  useEffect(() => {
    setMounted(true);
  }, []);

  // Use actual recording state
  const isRecording = audioState.isRecording || recordingState.isActive;

  useEffect(() => {
    // Listen for global shortcut events
    const unlistenStart = listen("recording-start", () => {
      if (!isActive) {
        handleStart();
      }
    });

    const unlistenStop = listen("recording-stop", () => {
      if (isActive) {
        handleStop();
      }
    });

    const unlistenToggle = listen("toggle-recording", () => {
      if (isRecording) {
        handleStop();
      } else {
        handleStart();
      }
    });

    return () => {
      unlistenStart.then((fn: () => void) => fn());
      unlistenStop.then((fn: () => void) => fn());
      unlistenToggle.then((fn: () => void) => fn());
    };
  }, [isActive, isRecording]);

  useEffect(() => {
    const handleKeyPress = (event: KeyboardEvent) => {
      if (!isActive) return;

      switch (event.key) {
        case "Escape":
          event.preventDefault();
          handleCancel();
          break;
        case " ":
          event.preventDefault();
          if (isTranscribing || isTranscribingLocal) {
            handleStop();
          } else {
            handleStart();
          }
          break;
      }
    };

    const handleMouseMove = () => {
      setShowControls(true);
      // Hide controls after 3 seconds of inactivity
      const timeout = setTimeout(() => setShowControls(false), 3000);
      return () => clearTimeout(timeout);
    };

    if (isActive) {
      document.addEventListener("keydown", handleKeyPress);
      document.addEventListener("mousemove", handleMouseMove);
      // Prevent body scroll when overlay is active
      document.body.style.overflow = "hidden";
    }

    return () => {
      document.removeEventListener("keydown", handleKeyPress);
      document.removeEventListener("mousemove", handleMouseMove);
      document.body.style.overflow = "unset";
    };
  }, [isActive, isTranscribing, isTranscribingLocal]);

  const handleStart = async () => {
    try {
      await startRecording({
        sampleRate: 16000,
        channelCount: 1,
        echoCancellation: true,
        noiseSuppression: true,
      });
      start();
    } catch (error) {
      console.error("Failed to start recording:", error);
    }
  };

  const handleStop = async () => {
    try {
      const audioBlob = stopRecording();
      if (audioBlob) {
        stop();

        // Start transcription
        setIsTranscribingLocal(true);
        try {
          const transcription = await transcribeFile(audioBlob);
          console.log("Transcription result:", transcription);
        } catch (error) {
          console.error("Transcription failed:", error);
        } finally {
          setIsTranscribingLocal(false);
        }
      }
    } catch (error) {
      console.error("Failed to stop recording:", error);
    }
  };

  const handleCancel = () => {
    stopRecording();
    cancel();
    setIsTranscribingLocal(false);
  };

  const formatDuration = (seconds: number): string => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins.toString().padStart(2, "0")}:${secs
      .toString()
      .padStart(2, "0")}`;
  };

  const isTranscribingFinal =
    isTranscribing || isTranscribingLocal || recordingState.isTranscribing;
  const currentDuration = audioState.duration || recordingState.duration;

  if (!isRecording || !mounted) return null;

  return (
    <PalModal
      isOpen={isRecording}
      onClose={handleCancel}
      size="md"
      withGlow={true}
      withCornerMarkers={true}
      closeOnBackdropClick={false}
      preventBodyScroll={true}
    >
      <PalModalHeader
        title={isTranscribingFinal ? "Transcribing..." : "Recording..."}
        subtitle={statusText}
        showCloseButton={false}
      />
      
      <PalModalContent>
        <div className="flex flex-col items-center space-y-6">
          {/* Status indicator */}
          <div
            className={`flex items-center justify-center w-20 h-20 rounded-full ${
              isTranscribing ? "bg-palantir-accent-orange" : "bg-palantir-accent-red"
            } animate-pulse-soft`}
          >
            {isTranscribing ? (
              <Loader2 size={32} className="text-white animate-spin" />
            ) : (
              <Mic size={32} className="text-white" />
            )}
          </div>

          <div className="text-center space-y-2">
            <p className="text-3xl font-mono font-bold text-palantir-accent-blue" aria-live="polite">
              {formatDuration(currentDuration)}
            </p>
          </div>

          {/* Transcription progress */}
          {isTranscribingFinal && (
            <div
              className="w-full max-w-md space-y-2"
              role="progressbar"
              aria-valuenow={recordingState.transcriptionProgress}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-label="Transcription progress"
            >
              <div className="w-full bg-palantir-zinc-200 dark:bg-palantir-zinc-700 rounded-full h-2">
                <div
                  className="bg-palantir-accent-blue h-2 rounded-full transition-all duration-300"
                  style={{ width: `${recordingState.transcriptionProgress}%` }}
                />
              </div>
              <span className="text-sm text-center block text-palantir-zinc-600 dark:text-palantir-zinc-400">
                {Math.round(recordingState.transcriptionProgress)}% Complete
              </span>
            </div>
          )}
        </div>
      </PalModalContent>

      <PalModalFooter>
        <div className="flex flex-col items-center space-y-4 w-full">
          <div className="flex space-x-4">
            {isTranscribingFinal ? (
              <PalButton
                variant="primary"
                onClick={handleStop}
                withGlow={true}
                withCornerMarkers={true}
                className="flex items-center gap-2"
              >
                <Square size={20} />
                <span>Stop (Space)</span>
              </PalButton>
            ) : (
              <PalButton
                variant="secondary"
                onClick={handleCancel}
                withGlow={true}
                withCornerMarkers={true}
                className="flex items-center gap-2"
              >
                <MicOff size={20} />
                <span>Cancel (Esc)</span>
              </PalButton>
            )}
          </div>

          <div className="flex space-x-6 text-sm text-palantir-zinc-600 dark:text-palantir-zinc-400">
            <span className="flex items-center space-x-1">
              <kbd className="px-2 py-1 bg-palantir-zinc-200 dark:bg-palantir-zinc-700 rounded text-xs">Space</kbd>
              <span>{isTranscribingFinal ? "Stop" : "Record"}</span>
            </span>
            <span className="flex items-center space-x-1">
              <kbd className="px-2 py-1 bg-palantir-zinc-200 dark:bg-palantir-zinc-700 rounded text-xs">Esc</kbd>
              <span>Cancel</span>
            </span>
          </div>
        </div>
      </PalModalFooter>
    </PalModal>
  );
};

export default RecordingOverlay;
