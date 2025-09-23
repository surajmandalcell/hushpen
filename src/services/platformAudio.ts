// Platform-specific audio settings and behaviors for ASR Pro
import { platformService } from './platform';

export interface AudioSettings {
    defaultSampleRate: number;
    defaultChannels: number;
    supportedFormats: string[];
    bufferSize: number;
    echoCancellation: boolean;
    noiseSuppression: boolean;
    autoGainControl: boolean;
}

class PlatformAudioService {
    getAudioSettings(): AudioSettings {
        const platform = platformService.getPlatform();

        // Base settings
        const baseSettings: AudioSettings = {
            defaultSampleRate: 16000,
            defaultChannels: 1,
            supportedFormats: ['mp3', 'wav', 'm4a', 'flac', 'ogg'],
            bufferSize: 4096,
            echoCancellation: true,
            noiseSuppression: true,
            autoGainControl: true,
        };

        // Platform-specific adjustments
        switch (platform) {
            case 'macos':
                return {
                    ...baseSettings,
                    defaultSampleRate: 44100, // macOS typically uses higher quality
                    bufferSize: 2048, // Smaller buffer for lower latency on macOS
                    echoCancellation: true,
                    noiseSuppression: true,
                    autoGainControl: false, // macOS handles this better natively
                };

            case 'windows':
                return {
                    ...baseSettings,
                    defaultSampleRate: 16000,
                    bufferSize: 4096, // Larger buffer for Windows stability
                    echoCancellation: true,
                    noiseSuppression: true,
                    autoGainControl: true,
                };

            case 'linux':
                return {
                    ...baseSettings,
                    defaultSampleRate: 16000,
                    bufferSize: 4096,
                    echoCancellation: false, // May not be supported on all Linux systems
                    noiseSuppression: false,
                    autoGainControl: true,
                };

            default:
                return baseSettings;
        }
    }

    getRecommendedAudioConstraints() {
        const settings = this.getAudioSettings();

        return {
            sampleRate: settings.defaultSampleRate,
            channelCount: settings.defaultChannels,
            echoCancellation: settings.echoCancellation,
            noiseSuppression: settings.noiseSuppression,
            autoGainControl: settings.autoGainControl,
        };
    }

    getAudioQualityOptions() {
        const platform = platformService.getPlatform();

        const baseOptions = [
            { id: 'low', name: 'Low (8 kHz)', sampleRate: 8000, bitrate: 64 },
            { id: 'medium', name: 'Medium (16 kHz)', sampleRate: 16000, bitrate: 128 },
            { id: 'high', name: 'High (44.1 kHz)', sampleRate: 44100, bitrate: 256 },
        ];

        // Platform-specific adjustments
        if (platform === 'macos') {
            // macOS can handle higher quality better
            return [
                ...baseOptions,
                { id: 'studio', name: 'Studio (48 kHz)', sampleRate: 48000, bitrate: 320 },
            ];
        }

        return baseOptions;
    }

    getPlatformAudioHints() {
        const platform = platformService.getPlatform();

        switch (platform) {
            case 'macos':
                return {
                    title: 'macOS Audio Setup',
                    hints: [
                        'macOS provides excellent audio quality with built-in echo cancellation',
                        'Use headphones to avoid feedback during recording',
                        'System Preferences > Sound > Input for additional settings',
                        'Consider using external microphones for best quality',
                    ],
                };

            case 'windows':
                return {
                    title: 'Windows Audio Setup',
                    hints: [
                        'Windows may require additional setup for microphone access',
                        'Check Privacy settings if microphone is not detected',
                        'Use the Sound control panel for advanced audio settings',
                        'Consider using ASIO drivers for professional audio work',
                    ],
                };

            case 'linux':
                return {
                    title: 'Linux Audio Setup',
                    hints: [
                        'Linux audio may require ALSA/PulseAudio configuration',
                        'Check pavucontrol for audio device management',
                        'Some distributions may need additional audio packages',
                        'Consider using JACK for professional audio work',
                    ],
                };

            default:
                return {
                    title: 'Audio Setup',
                    hints: [
                        'Ensure your microphone is properly connected',
                        'Check system audio settings',
                        'Test recording before use',
                    ],
                };
        }
    }

    isAudioFormatSupported(format: string): boolean {
        return this.getAudioSettings().supportedFormats.includes(format.toLowerCase());
    }

    getOptimalRecordingSettings() {
        const platform = platformService.getPlatform();

        // Platform-specific optimizations
        if (platform === 'macos') {
            return {
                sampleRate: 48000,
                channelCount: 2,
                echoCancellation: true,
                noiseSuppression: true,
                autoGainControl: false,
            };
        } else if (platform === 'windows') {
            return {
                sampleRate: 44100,
                channelCount: 1,
                echoCancellation: true,
                noiseSuppression: true,
                autoGainControl: true,
            };
        } else {
            return {
                sampleRate: 16000,
                channelCount: 1,
                echoCancellation: false,
                noiseSuppression: false,
                autoGainControl: true,
            };
        }
    }
}

// Export singleton instance
export const platformAudioService = new PlatformAudioService();
