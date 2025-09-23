import { useState, useEffect } from 'react';
import { audioRecordingService, AudioRecordingState, AudioRecordingOptions } from '../services/audioRecording';

export const useAudioRecording = () => {
    const [state, setState] = useState<AudioRecordingState>(audioRecordingService.getState());

    useEffect(() => {
        const unsubscribe = audioRecordingService.subscribe(setState);
        return unsubscribe;
    }, []);

    const startRecording = async (options?: AudioRecordingOptions) => {
        try {
            await audioRecordingService.startRecording(options);
        } catch (error) {
            console.error('Failed to start recording:', error);
            throw error;
        }
    };

    const stopRecording = (): Blob | null => {
        return audioRecordingService.stopRecording();
    };

    return {
        state,
        startRecording,
        stopRecording,
        isRecording: audioRecordingService.isRecording(),
    };
};
