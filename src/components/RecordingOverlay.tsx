import React, { useEffect, useState } from 'react';
import { Mic, MicOff, Square, Loader2 } from 'lucide-react';
import './RecordingOverlay.css';

export interface RecordingOverlayProps {
  isActive: boolean;
  isTranscribing?: boolean;
  transcriptionProgress?: number; // 0-100
  onCancel: () => void;
  onStop: () => void;
  statusText?: string;
  duration?: number; // in seconds
}

const RecordingOverlay: React.FC<RecordingOverlayProps> = ({
  isActive,
  isTranscribing = false,
  transcriptionProgress = 0,
  onCancel,
  onStop,
  statusText = "Listening...",
  duration = 0,
}) => {
  const [showControls, setShowControls] = useState(false);
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  useEffect(() => {
    const handleKeyPress = (event: KeyboardEvent) => {
      if (!isActive) return;

      switch (event.key) {
        case 'Escape':
          event.preventDefault();
          onCancel();
          break;
        case ' ':
          event.preventDefault();
          if (isTranscribing) {
            onStop();
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
      document.addEventListener('keydown', handleKeyPress);
      document.addEventListener('mousemove', handleMouseMove);
      // Prevent body scroll when overlay is active
      document.body.style.overflow = 'hidden';
    }

    return () => {
      document.removeEventListener('keydown', handleKeyPress);
      document.removeEventListener('mousemove', handleMouseMove);
      document.body.style.overflow = 'unset';
    };
  }, [isActive, isTranscribing, onCancel, onStop]);

  const formatDuration = (seconds: number): string => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  };

  if (!isActive || !mounted) return null;

  return (
    <div className="recording-overlay">
      <div className="recording-overlay__backdrop" />

      <div className="recording-overlay__content">
        {/* Status indicator */}
        <div className="recording-overlay__status">
          <div className={`recording-overlay__indicator ${isTranscribing ? 'transcribing' : 'recording'}`}>
            {isTranscribing ? (
              <Loader2 size={32} className="recording-overlay__spinner" />
            ) : (
              <Mic size={32} className="recording-overlay__mic" />
            )}
          </div>

          <div className="recording-overlay__text">
            <h2 className="recording-overlay__title">
              {isTranscribing ? 'Transcribing...' : 'Recording...'}
            </h2>
            <p className="recording-overlay__subtitle">{statusText}</p>
            <p className="recording-overlay__duration">
              {formatDuration(duration)}
            </p>
          </div>
        </div>

        {/* Transcription progress */}
        {isTranscribing && (
          <div className="recording-overlay__progress">
            <div className="recording-overlay__progress-bar">
              <div
                className="recording-overlay__progress-fill"
                style={{ width: `${transcriptionProgress}%` }}
              />
            </div>
            <span className="recording-overlay__progress-text">
              {Math.round(transcriptionProgress)}% Complete
            </span>
          </div>
        )}

        {/* Controls */}
        <div className={`recording-overlay__controls ${showControls ? 'visible' : ''}`}>
          <div className="recording-overlay__control-group">
            {isTranscribing ? (
              <button
                className="recording-overlay__control-button stop"
                onClick={onStop}
                aria-label="Stop transcription"
              >
                <Square size={24} />
                <span>Stop (Space)</span>
              </button>
            ) : (
              <button
                className="recording-overlay__control-button cancel"
                onClick={onCancel}
                aria-label="Cancel recording"
              >
                <MicOff size={24} />
                <span>Cancel (Esc)</span>
              </button>
            )}
          </div>

          <div className="recording-overlay__keyboard-hints">
            <span className="recording-overlay__hint">
              <kbd>Space</kbd> {isTranscribing ? 'Stop' : 'Record'}
            </span>
            <span className="recording-overlay__hint">
              <kbd>Esc</kbd> Cancel
            </span>
          </div>
        </div>

        {/* Pulse animation */}
        <div className="recording-overlay__pulse" />
      </div>
    </div>
  );
};

export default RecordingOverlay;
