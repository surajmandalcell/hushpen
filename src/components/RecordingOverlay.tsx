import React, { useEffect, useState } from "react";
import { Mic, MicOff, Square, Loader2 } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { useAudioRecording } from "../hooks/useAudioRecording";
import { useRecording } from "../services/recordingManager";

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
    <div
      className="recording-overlay"
      role="dialog"
      aria-modal="true"
      aria-labelledby="recording-status"
      aria-describedby="recording-instructions"
    >
      <div className="recording-overlay__backdrop" />

      <div className="recording-content">
        {/* Status indicator */}
        <div className="flex flex-col items-center space-y-4">
          <div
            className={`flex items-center justify-center w-20 h-20 rounded-full ${
              isTranscribing ? "bg-macos-orange" : "bg-macos-red"
            } animate-pulse-soft`}
          >
            {isTranscribing ? (
              <Loader2 size={32} className="text-white animate-spin" />
            ) : (
              <Mic size={32} className="text-white" />
            )}
          </div>

          <div className="text-center space-y-2">
            <h2 id="recording-status" className="text-2xl font-semibold text-macos-text dark:text-macos-text-dark">
              {isTranscribingFinal ? "Transcribing..." : "Recording..."}
            </h2>
            <p className="text-macos-text-secondary text-lg" aria-live="polite">
              {statusText}
            </p>
            <p className="text-3xl font-mono font-bold text-macos-blue" aria-live="polite">
              {formatDuration(currentDuration)}
            </p>
          </div>
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
            <div className="progress-macos">
              <div
                className="progress-macos-bar"
                style={{ width: `${recordingState.transcriptionProgress}%` }}
              />
            </div>
            <span className="text-sm text-center block text-macos-text-secondary">
              {Math.round(recordingState.transcriptionProgress)}% Complete
            </span>
          </div>
        )}

        {/* Controls */}
        <div
          className={`flex flex-col items-center space-y-4 transition-all duration-300 ${
            showControls ? "opacity-100" : "opacity-30"
          }`}
        >
          <div className="flex space-x-4">
            {isTranscribingFinal ? (
              <button
                className="btn-macos-danger flex items-center space-x-2"
                onClick={handleStop}
                aria-label="Stop transcription"
                aria-describedby="stop-instructions"
              >
                <Square size={20} />
                <span>Stop (Space)</span>
              </button>
            ) : (
              <button
                className="btn-macos-secondary flex items-center space-x-2"
                onClick={handleCancel}
                aria-label="Cancel recording"
                aria-describedby="cancel-instructions"
              >
                <MicOff size={20} />
                <span>Cancel (Esc)</span>
              </button>
            )}
          </div>

          <div className="flex space-x-6 text-sm text-macos-text-secondary">
            <span className="flex items-center space-x-1">
              <kbd className="px-2 py-1 bg-gray-200 dark:bg-gray-700 rounded text-xs">Space</kbd>
              <span>{isTranscribingFinal ? "Stop" : "Record"}</span>
            </span>
            <span className="flex items-center space-x-1">
              <kbd className="px-2 py-1 bg-gray-200 dark:bg-gray-700 rounded text-xs">Esc</kbd>
              <span>Cancel</span>
            </span>
          </div>
        </div>

        {/* Hidden instructions for screen readers */}
        <div id="recording-instructions" className="sr-only">
          Use Space to{" "}
          {isTranscribingFinal ? "stop transcription" : "start recording"}. Use
          Escape to cancel. This overlay shows recording status and controls.
        </div>

        <div id="stop-instructions" className="sr-only">
          Stop the current transcription process
        </div>

        <div id="cancel-instructions" className="sr-only">
          Cancel recording and close overlay
        </div>
      </div>
    </div>
  );
};

export default RecordingOverlay;
