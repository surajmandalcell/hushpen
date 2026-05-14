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
    deviceId?: string;
    echoCancellation?: boolean;
    noiseSuppression?: boolean;
}

function scheduleAudioFrame(callback: FrameRequestCallback): number {
    if (typeof window.requestAnimationFrame === 'function') {
        return window.requestAnimationFrame(callback);
    }

    return window.setTimeout(() => callback(Date.now()), 16);
}

function cancelAudioFrame(id: number): void {
    if (typeof window.cancelAnimationFrame === 'function') {
        window.cancelAnimationFrame(id);
        return;
    }

    window.clearTimeout(id);
}

class AudioRecordingService {
    private mediaRecorder: MediaRecorder | null = null;
    private audioContext: AudioContext | null = null;
    private analyser: AnalyserNode | null = null;
    private microphone: MediaStreamAudioSourceNode | null = null;
    private stream: MediaStream | null = null;
    private chunks: Blob[] = [];
    private animationFrame: number | null = null;
    private startTime: number | null = null;

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

        const bufferLength = this.analyser.fftSize;
        const dataArray = new Uint8Array(bufferLength);

        if (typeof this.analyser.getByteTimeDomainData === 'function') {
            this.analyser.getByteTimeDomainData(dataArray);

            let sum = 0;
            for (let i = 0; i < bufferLength; i++) {
                const centered = (dataArray[i] - 128) / 128;
                sum += centered * centered;
            }

            const rms = Math.sqrt(sum / bufferLength);
            this.state.audioLevel = Math.min(Math.max((rms - 0.016) / 0.13, 0), 1);
        } else {
            this.analyser.getByteFrequencyData(dataArray);
            let sum = 0;
            for (let i = 0; i < bufferLength; i++) {
                sum += dataArray[i];
            }
            this.state.audioLevel = sum / bufferLength / 255;
        }
        if (this.startTime) {
            this.state.duration = Math.max(0, Math.floor((Date.now() - this.startTime) / 1000));
        }

        this.notifyListeners();

        // Continue monitoring
        this.animationFrame = scheduleAudioFrame(() => this.updateAudioLevel());
    }

    async startRecording(options: AudioRecordingOptions = {}): Promise<void> {
        try {
            if (this.state.isRecording) {
                return;
            }

            // Request microphone access
            const audioConstraints: MediaTrackConstraints = {
                sampleRate: options.sampleRate || 16000,
                channelCount: options.channelCount || 1,
                echoCancellation: options.echoCancellation ?? true,
                noiseSuppression: options.noiseSuppression ?? true,
            };

            if (options.deviceId) {
                audioConstraints.deviceId = { exact: options.deviceId };
            }

            const constraints: MediaStreamConstraints = {
                audio: audioConstraints,
            };

            this.stream = await navigator.mediaDevices.getUserMedia(constraints);

            // Set up audio context for monitoring
            this.audioContext = new AudioContext({ sampleRate: options.sampleRate || 16000 });
            if (typeof this.audioContext.resume === 'function') {
                await this.audioContext.resume().catch(() => {});
            }
            this.microphone = this.audioContext.createMediaStreamSource(this.stream);
            this.analyser = this.audioContext.createAnalyser();
            this.analyser.fftSize = 1024;
            this.analyser.smoothingTimeConstant = 0.72;
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
            this.startTime = Date.now();
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

    stopRecording(): Promise<Blob | null> {
        if (!this.mediaRecorder || !this.state.isRecording) {
            return Promise.resolve(null);
        }

        const recorder = this.mediaRecorder;
        const mimeType = recorder.mimeType || this.getSupportedMimeType();

        return new Promise((resolve, reject) => {
            const finish = () => {
                // Stop all tracks
                if (this.stream) {
                    this.stream.getTracks().forEach(track => track.stop());
                }

                // Clean up audio context
                if (this.animationFrame) {
                    cancelAudioFrame(this.animationFrame);
                    this.animationFrame = null;
                }
                if (this.audioContext) {
                    this.audioContext.close();
                }

                const audioBlob = new Blob(this.chunks, { type: mimeType });

                this.mediaRecorder = null;
                this.audioContext = null;
                this.analyser = null;
                this.microphone = null;
                this.stream = null;
                this.chunks = [];
                this.startTime = null;
                this.state.isRecording = false;
                this.state.audioLevel = 0;
                this.notifyListeners();

                resolve(audioBlob);
            };

            recorder.ondataavailable = (event) => {
                if (event.data.size > 0) {
                    this.chunks.push(event.data);
                }
            };
            recorder.onstop = finish;
            recorder.onerror = () => {
                this.state.isRecording = false;
                this.notifyListeners();
                reject(new Error('Failed to stop recording'));
            };

            if (recorder.state === 'inactive') {
                finish();
            } else {
                recorder.stop();
            }
        });
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
