import React, { useEffect, useState } from "react";
import { Mic, MicOff, Square, Loader2 } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { useAudioRecording } from "../hooks/useAudioRecording";
import { useRecording } from "../services/recordingManager";
import "./RecordingOverlay.css";

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

      <div className="recording-overlay__content">
        {/* Status indicator */}
        <div className="recording-overlay__status">
          <div
            className={`recording-overlay__indicator ${
              isTranscribing ? "transcribing" : "recording"
            }`}
          >
            {isTranscribing ? (
              <Loader2 size={32} className="recording-overlay__spinner" />
            ) : (
              <Mic size={32} className="recording-overlay__mic" />
            )}
          </div>

          <div className="recording-overlay__text">
            <h2 id="recording-status" className="recording-overlay__title">
              {isTranscribingFinal ? "Transcribing..." : "Recording..."}
            </h2>
            <p className="recording-overlay__subtitle" aria-live="polite">
              {statusText}
            </p>
            <p className="recording-overlay__duration" aria-live="polite">
              {formatDuration(currentDuration)}
            </p>
          </div>
        </div>

        {/* Transcription progress */}
        {isTranscribingFinal && (
          <div
            className="recording-overlay__progress"
            role="progressbar"
            aria-valuenow={recordingState.transcriptionProgress}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label="Transcription progress"
          >
            <div className="recording-overlay__progress-bar">
              <div
                className="recording-overlay__progress-fill"
                style={{ width: `${recordingState.transcriptionProgress}%` }}
              />
            </div>
            <span className="recording-overlay__progress-text">
              {Math.round(recordingState.transcriptionProgress)}% Complete
            </span>
          </div>
        )}

        {/* Controls */}
        <div
          className={`recording-overlay__controls ${
            showControls ? "visible" : ""
          }`}
        >
          <div className="recording-overlay__control-group">
            {isTranscribingFinal ? (
              <button
                className="recording-overlay__control-button stop"
                onClick={handleStop}
                aria-label="Stop transcription"
                aria-describedby="stop-instructions"
              >
                <Square size={24} />
                <span>Stop (Space)</span>
              </button>
            ) : (
              <button
                className="recording-overlay__control-button cancel"
                onClick={handleCancel}
                aria-label="Cancel recording"
                aria-describedby="cancel-instructions"
              >
                <MicOff size={24} />
                <span>Cancel (Esc)</span>
              </button>
            )}
          </div>

          <div className="recording-overlay__keyboard-hints">
            <span className="recording-overlay__hint">
              <kbd>Space</kbd> {isTranscribingFinal ? "Stop" : "Record"}
            </span>
            <span className="recording-overlay__hint">
              <kbd>Esc</kbd> Cancel
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

        {/* Pulse animation */}
        <div className="recording-overlay__pulse" />
      </div>
    </div>
  );
};

export default RecordingOverlay;
