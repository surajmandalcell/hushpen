// Global shortcut service for ASR Pro
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface GlobalShortcutService {
    startRecording: () => Promise<void>;
    stopRecording: () => Promise<void>;
    initialize: () => Promise<void>;
    cleanup: () => void;
}

class GlobalShortcutServiceImpl implements GlobalShortcutService {
    private recordingActive = false;
    private unlistenFunctions: (() => void)[] = [];

    async startRecording(): Promise<void> {
        try {
            await invoke('start_recording');
        } catch (error) {
            console.error('Failed to start recording via global shortcut:', error);
            throw error;
        }
    }

    async stopRecording(): Promise<void> {
        try {
            await invoke('stop_recording');
        } catch (error) {
            console.error('Failed to stop recording via global shortcut:', error);
            throw error;
        }
    }

    async initialize(): Promise<void> {
        try {
            // Listen for toggle recording events from the global shortcut handler
            const unlistenToggle = await listen('toggle-recording', () => {
                if (this.recordingActive) {
                    this.stopRecording();
                    this.recordingActive = false;
                } else {
                    this.startRecording();
                    this.recordingActive = true;
                }
            });

            // Listen for recording start events
            const unlistenStart = await listen('recording-start', () => {
                this.recordingActive = true;
            });

            // Listen for recording stop events
            const unlistenStop = await listen('recording-stop', () => {
                this.recordingActive = false;
            });

            this.unlistenFunctions = [unlistenToggle, unlistenStart, unlistenStop];

        } catch (error) {
            console.error('Failed to initialize global shortcuts:', error);
            throw error;
        }
    }

    cleanup(): void {
        this.unlistenFunctions.forEach(unlisten => unlisten());
        this.unlistenFunctions = [];
        this.recordingActive = false;
    }

    isRecording(): boolean {
        return this.recordingActive;
    }
}

// Export singleton instance
export const globalShortcutService = new GlobalShortcutServiceImpl();
