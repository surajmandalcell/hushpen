// Audio recording service for ASR Pro
export interface AudioRecordingState {
    isRecording: boolean;
    duration: number;
    audioLevel: number;
    error?: string;
}

export interface AudioRecordingOptions {
    sampleRate?: number;
    channelCount?: number;
    echoCancellation?: boolean;
    noiseSuppression?: boolean;
}

class AudioRecordingService {
    private mediaRecorder: MediaRecorder | null = null;
    private audioContext: AudioContext | null = null;
    private analyser: AnalyserNode | null = null;
    private microphone: MediaStreamAudioSourceNode | null = null;
    private stream: MediaStream | null = null;
    private chunks: Blob[] = [];
    private animationFrame: number | null = null;

    private state: AudioRecordingState = {
        isRecording: false,
        duration: 0,
        audioLevel: 0,
    };

    private listeners: ((state: AudioRecordingState) => void)[] = [];

    /**
     * Subscribe to recording state changes
     */
    subscribe(listener: (state: AudioRecordingState) => void): () => void {
        this.listeners.push(listener);
        return () => {
            const index = this.listeners.indexOf(listener);
            if (index > -1) {
                this.listeners.splice(index, 1);
            }
        };
    }

    private notifyListeners(): void {
        this.listeners.forEach(listener => listener({ ...this.state }));
    }

    private updateAudioLevel(): void {
        if (!this.analyser || !this.state.isRecording) return;

        const bufferLength = this.analyser.frequencyBinCount;
        const dataArray = new Uint8Array(bufferLength);
        this.analyser.getByteFrequencyData(dataArray);

        // Calculate average audio level
        let sum = 0;
        for (let i = 0; i < bufferLength; i++) {
            sum += dataArray[i];
        }
        this.state.audioLevel = sum / bufferLength / 255; // Normalize to 0-1

        this.notifyListeners();

        // Continue monitoring
        this.animationFrame = requestAnimationFrame(() => this.updateAudioLevel());
    }

    async startRecording(options: AudioRecordingOptions = {}): Promise<void> {
        try {
            // Request microphone access
            const constraints: MediaStreamConstraints = {
                audio: {
                    sampleRate: options.sampleRate || 16000,
                    channelCount: options.channelCount || 1,
                    echoCancellation: options.echoCancellation ?? true,
                    noiseSuppression: options.noiseSuppression ?? true,
                },
            };

            this.stream = await navigator.mediaDevices.getUserMedia(constraints);

            // Set up audio context for monitoring
            this.audioContext = new AudioContext({ sampleRate: options.sampleRate || 16000 });
            this.microphone = this.audioContext.createMediaStreamSource(this.stream);
            this.analyser = this.audioContext.createAnalyser();
            this.analyser.fftSize = 256;
            this.microphone.connect(this.analyser);

            // Set up MediaRecorder
            const mimeType = this.getSupportedMimeType();
            this.mediaRecorder = new MediaRecorder(this.stream, {
                mimeType,
            });

            this.chunks = [];

            this.mediaRecorder.ondataavailable = (event) => {
                if (event.data.size > 0) {
                    this.chunks.push(event.data);
                }
            };

            this.mediaRecorder.onstop = () => {
                this.state.isRecording = false;
                this.updateAudioLevel();
            };

            // Start recording
            this.mediaRecorder.start(100); // Collect data every 100ms
            this.state.isRecording = true;
            this.state.duration = 0;
            this.state.error = undefined;

            // Start audio level monitoring
            this.updateAudioLevel();

            this.notifyListeners();

        } catch (error) {
            this.state.error = error instanceof Error ? error.message : 'Failed to start recording';
            this.notifyListeners();
            throw error;
        }
    }

    stopRecording(): Blob | null {
        if (!this.mediaRecorder || !this.state.isRecording) {
            return null;
        }

        this.mediaRecorder.stop();

        // Stop all tracks
        if (this.stream) {
            this.stream.getTracks().forEach(track => track.stop());
        }

        // Clean up audio context
        if (this.animationFrame) {
            cancelAnimationFrame(this.animationFrame);
        }
        if (this.audioContext) {
            this.audioContext.close();
        }

        // Create audio blob
        const mimeType = this.getSupportedMimeType();
        const audioBlob = new Blob(this.chunks, { type: mimeType });

        this.state.isRecording = false;
        this.notifyListeners();

        return audioBlob;
    }

    private getSupportedMimeType(): string {
        const possibleTypes = [
            'audio/webm;codecs=opus',
            'audio/webm',
            'audio/mp4',
            'audio/mpeg',
            'audio/wav',
        ];

        for (const type of possibleTypes) {
            if (MediaRecorder.isTypeSupported(type)) {
                return type;
            }
        }

        return 'audio/webm'; // Fallback
    }

    getState(): AudioRecordingState {
        return { ...this.state };
    }

    isRecording(): boolean {
        return this.state.isRecording;
    }
}

// Export singleton instance
export const audioRecordingService = new AudioRecordingService();
