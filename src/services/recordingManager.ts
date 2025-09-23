// Recording state management for ASR Pro
import { apiClient } from './api';

export interface RecordingState {
  isActive: boolean;
  isTranscribing: boolean;
  transcriptionProgress: number;
  statusText: string;
  duration: number;
  startTime: number | null;
}

export type RecordingEvent =
  | 'start'
  | 'stop'
  | 'cancel'
  | 'transcribing-start'
  | 'transcribing-progress'
  | 'transcribing-complete'
  | 'error';

class RecordingManager {
  private state: RecordingState = {
    isActive: false,
    isTranscribing: false,
    transcriptionProgress: 0,
    statusText: 'Ready',
    duration: 0,
    startTime: null,
  };

  private listeners: ((state: RecordingState) => void)[] = [];
  private intervalId: number | null = null;

  /**
   * Subscribe to recording state changes
   */
  subscribe(listener: (state: RecordingState) => void): () => void {
    this.listeners.push(listener);
    return () => {
      const index = this.listeners.indexOf(listener);
      if (index > -1) {
        this.listeners.splice(index, 1);
      }
    };
  }

  /**
   * Notify all listeners of state changes
   */
  private notifyListeners(): void {
    this.listeners.forEach(listener => listener({ ...this.state }));
  }

  /**
   * Start recording timer
   */
  private startTimer(): void {
    this.state.startTime = Date.now();
    this.state.duration = 0;

    this.intervalId = window.setInterval(() => {
      if (this.state.startTime) {
        this.state.duration = Math.floor((Date.now() - this.state.startTime) / 1000);
        this.notifyListeners();
      }
    }, 1000);
  }

  /**
   * Stop recording timer
   */
  private stopTimer(): void {
    if (this.intervalId) {
      clearInterval(this.intervalId);
      this.intervalId = null;
    }
  }

  /**
   * Start recording
   */
  start(): void {
    this.state = {
      ...this.state,
      isActive: true,
      isTranscribing: false,
      transcriptionProgress: 0,
      statusText: 'Recording... Press Space to stop, Esc to cancel',
      duration: 0,
      startTime: Date.now(),
    };

    this.startTimer();
    this.notifyListeners();
  }

  /**
   * Stop recording and start transcription
   */
  stop(): void {
    this.state = {
      ...this.state,
      isActive: false,
      isTranscribing: true,
      statusText: 'Processing audio...',
      transcriptionProgress: 0,
    };

    this.stopTimer();
    this.notifyListeners();
  }

  /**
   * Cancel recording
   */
  cancel(): void {
    this.state = {
      ...this.state,
      isActive: false,
      isTranscribing: false,
      transcriptionProgress: 0,
      statusText: 'Ready',
      duration: 0,
      startTime: null,
    };

    this.stopTimer();
    this.notifyListeners();
  }

  /**
   * Start transcription progress
   */
  startTranscribing(): void {
    this.state = {
      ...this.state,
      isTranscribing: true,
      statusText: 'Transcribing audio...',
      transcriptionProgress: 0,
    };

    this.notifyListeners();
  }

  /**
   * Update transcription progress
   */
  updateProgress(progress: number, statusText?: string): void {
    this.state.transcriptionProgress = Math.min(Math.max(progress, 0), 100);
    if (statusText) {
      this.state.statusText = statusText;
    }
    this.notifyListeners();
  }

  /**
   * Complete transcription
   */
  completeTranscription(): void {
    this.state = {
      ...this.state,
      isActive: false,
      isTranscribing: false,
      transcriptionProgress: 100,
      statusText: 'Transcription completed',
      duration: 0,
      startTime: null,
    };

    this.stopTimer();
    this.notifyListeners();

    // Auto-hide success state after 2 seconds
    setTimeout(() => {
      this.state.statusText = 'Ready';
      this.notifyListeners();
    }, 2000);
  }

  /**
   * Handle recording error
   */
  error(message: string = 'An error occurred'): void {
    this.state = {
      ...this.state,
      isActive: false,
      isTranscribing: false,
      transcriptionProgress: 0,
      statusText: message,
      duration: 0,
      startTime: null,
    };

    this.stopTimer();
    this.notifyListeners();
  }

  /**
   * Transcribe a recorded audio file
   */
  async transcribeFile(audioBlob: Blob): Promise<string> {
    try {
      this.startTranscribing();

      // Convert blob to file
      const audioFile = new File([audioBlob], 'recording.wav', { type: 'audio/wav' });

      // Use the API client to transcribe
      const result = await apiClient.transcribeFile(audioFile);

      this.completeTranscription();

      return result.text || 'No transcription result';
    } catch (error) {
      this.error(`Transcription failed: ${error instanceof Error ? error.message : 'Unknown error'}`);
      throw error;
    }
  }

  /**
   * Get current recording state
   */
  getState(): RecordingState {
    return { ...this.state };
  }

  /**
   * Check if currently recording or transcribing
   */
  isActive(): boolean {
    return this.state.isActive || this.state.isTranscribing;
  }
}

// Export singleton instance
export const recordingManager = new RecordingManager();

// Export hook for React components
export const useRecording = () => {
  const [state, setState] = React.useState<RecordingState>(recordingManager.getState());

  React.useEffect(() => {
    const unsubscribe = recordingManager.subscribe(setState);
    return unsubscribe;
  }, []);

  return {
    state,
    start: recordingManager.start.bind(recordingManager),
    stop: recordingManager.stop.bind(recordingManager),
    cancel: recordingManager.cancel.bind(recordingManager),
    startTranscribing: recordingManager.startTranscribing.bind(recordingManager),
    updateProgress: recordingManager.updateProgress.bind(recordingManager),
    completeTranscription: recordingManager.completeTranscription.bind(recordingManager),
    error: recordingManager.error.bind(recordingManager),
    isActive: recordingManager.isActive.bind(recordingManager),
    transcribeFile: recordingManager.transcribeFile.bind(recordingManager),
  };
};

// Import React for the hook
import React from 'react';
