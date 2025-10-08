import { Request, Response } from 'express';
import { getDockerService } from '../services/dockerService';
import { getAudioService } from '../services/audioService';
import { logger } from '../utils/logger';

export const getOptions = async (req: Request, res: Response): Promise<void> => {
  try {
    // Get available models from Docker service
    const dockerService = getDockerService();
    let availableModels: string[] = [];
    
    if (dockerService) {
      availableModels = dockerService.listAvailableModels();
    }

    // Get supported formats from audio service
    const audioService = getAudioService();
    const supportedFormats = audioService.getSupportedFormats();

    // Define supported formats with MIME types
    const supportedFormatsWithMime = {
      audio: {
        extensions: supportedFormats.audio,
        mime_types: [
          "audio/wav", "audio/wave", "audio/x-wav",
          "audio/mpeg", "audio/mp3",
          "audio/mp4", "audio/x-m4a",
          "audio/flac", "audio/x-flac",
          "audio/ogg", "audio/opus",
          "audio/aac", "audio/x-aac",
          "audio/wma", "audio/x-ms-wma",
          "audio/aiff", "audio/x-aiff"
        ]
      },
      video: {
        extensions: supportedFormats.video,
        mime_types: [
          "video/mp4", "video/x-msvideo", "video/avi",
          "video/quicktime", "video/x-matroska",
          "video/x-flv", "video/webm",
          "video/3gpp", "video/x-ms-wmv",
          "video/mpeg", "video/mp2t"
        ]
      }
    };

    // Define endpoints with their options
    const endpoints = {
      "/v1/audio/transcriptions": {
        method: "POST",
        description: "Transcribe audio or video files using AI models",
        parameters: {
          file: {
            type: "file",
            required: true,
            description: "Audio or video file to transcribe",
            supported_formats: supportedFormatsWithMime,
            max_size_mb: 100,
            validation: {
              min_size_bytes: 100,
              supported_extensions: supportedFormats.audio.concat(supportedFormats.video)
            }
          },
          model: {
            type: "string",
            required: false,
            default: "whisper-base",
            description: "AI model to use for transcription",
            options: availableModels,
            examples: ["whisper-base", "whisper-small", "whisper-medium"]
          },
          response_format: {
            type: "string",
            required: false,
            default: "json",
            description: "Output format for transcription result",
            options: ["json", "text", "srt"],
            examples: ["json", "text", "srt"]
          }
        },
        responses: {
          json: {
            description: "Complete transcription with segments and timestamps",
            schema: {
              text: "string",
              segments: "array",
              language: "string",
              language_probability: "number",
              duration: "number",
              backend: "string"
            }
          },
          text: {
            description: "Plain text transcription only",
            schema: { text: "string" }
          },
          srt: {
            description: "SubRip subtitle format with timestamps",
            schema: "string"
          }
        }
      },
      "/v1/models": {
        method: "GET",
        description: "List all available AI models",
        parameters: {},
        responses: {
          default: {
            description: "List of available models with readiness status",
            schema: {
              data: "array",
              models: availableModels
            }
          }
        }
      },
      "/v1/settings/model": {
        method: "POST",
        description: "Set the active AI model",
        parameters: {
          model_id: {
            type: "string",
            required: true,
            description: "Model ID to set as active",
            options: availableModels,
            examples: availableModels.slice(0, 3)
          }
        },
        responses: {
          default: {
            description: "Model setting result",
            schema: {
              status: "string",
              model: "string"
            }
          }
        }
      },
      "/health": {
        method: "GET",
        description: "System health check",
        parameters: {},
        responses: {
          default: {
            description: "System health and status information",
            schema: {
              status: "string",
              current_model: "string",
              device: "string"
            }
          }
        }
      }
    };

    // System capabilities
    let gpuAcceleration = false;
    let currentDevice = "Unknown";
    
    if (dockerService) {
      try {
        const systemStatus = await dockerService.getSystemStatus();
        gpuAcceleration = systemStatus.gpuAvailable || false;
        currentDevice = gpuAcceleration 
          ? "Docker container (GPU)" 
          : "Docker container (CPU)";
      } catch (e) {
        logger.warning(`Failed to get system status: ${e}`);
        currentDevice = "Docker container";
      }
    } else {
      currentDevice = "Docker container";
    }
    
    const capabilities = {
      ffmpeg_support: true,
      video_processing: true,
      audio_extraction: true,
      gpu_acceleration: gpuAcceleration,
      current_device: currentDevice,
      websocket_support: true,
      real_time_progress: true
    };

    const optionsResponse = {
      api_version: "1.0.0",
      server: "ASR Pro API",
      endpoints,
      supported_formats: supportedFormatsWithMime,
      available_models: availableModels,
      capabilities,
      examples: {
        curl_transcription: `curl -X POST 'http://127.0.0.1:8000/v1/audio/transcriptions?model=whisper-base' -F 'file=@your_audio.mp3'`,
        curl_video: `curl -X POST 'http://127.0.0.1:8000/v1/audio/transcriptions?model=whisper-base' -F 'file=@your_video.mp4'`,
        websocket: "ws://127.0.0.1:8000/ws"
      }
    };

    logger.info('Options endpoint accessed');
    res.json(optionsResponse);
  } catch (error) {
    logger.error('Failed to get options:', error);
    throw error;
  }
};