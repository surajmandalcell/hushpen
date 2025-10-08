import { Request, Response } from 'express';
import multer from 'multer';
import { getDockerService } from '../services/dockerService';
import { getAudioService } from '../services/audioService';
import { logger } from '../utils/logger';
import { 
  ServiceUnavailableError, 
  ValidationError, 
  AudioError,
  NotFoundError 
} from '../utils/errors';

// Configure multer for file uploads
const upload = multer({
  storage: multer.memoryStorage(),
  limits: {
    fileSize: 100 * 1024 * 1024, // 100MB limit
  },
  fileFilter: (req, file, cb) => {
    // Check file extension
    const audioService = getAudioService();
    if (audioService.isFormatSupported(file.originalname)) {
      cb(null, true);
    } else {
      cb(new ValidationError('Unsupported file format'));
    }
  }
});

export interface TranscriptionResponse {
  text: string;
  segments?: Array<{
    id: number;
    seek: number;
    start: number;
    end: number;
    text: string;
    tokens: number[];
    temperature: number;
    avg_logprob: number;
    compression_ratio: number;
    no_speech_prob: number;
  }>;
  language?: string;
  language_probability?: number;
  duration?: number;
  model_id?: string;
  backend?: string;
  processing_time?: number;
}

// Middleware to handle file upload
export const transcribeAudio = async (req: Request, res: Response): Promise<void> => {
  try {
    // Use multer middleware to handle file upload
    upload.single('file')(req, res, async (err) => {
      if (err) {
        logger.error('File upload error:', err);
        if (err instanceof multer.MulterError) {
          if (err.code === 'LIMIT_FILE_SIZE') {
            throw new ValidationError('File size exceeds the maximum allowed limit (100MB)');
          }
          throw new ValidationError(`File upload error: ${err.message}`);
        }
        throw new ValidationError(`File upload error: ${err.message}`);
      }

      if (!req.file) {
        throw new ValidationError('No file uploaded');
      }

      try {
        const { model = 'whisper-base', response_format = 'json' } = req.query;
        const filename = req.file.originalname;
        const fileBuffer = req.file.buffer;

        logger.info(`Transcription requested for file: ${filename} with model: ${model}`);

        // Validate audio file
        const audioService = getAudioService();
        const validation = audioService.validateAudioFile(fileBuffer, filename);
        
        if (!validation.isValid) {
          throw new ValidationError(validation.error || 'Invalid audio file');
        }

        // Extract audio data if needed (for corrupted uploads)
        const cleanedBuffer = audioService.extractAudioData(fileBuffer, fileBuffer.length);

        // Get Docker service
        const dockerService = getDockerService();
        
        if (!dockerService) {
          logger.error('Docker service not initialized during transcription request');
          throw new ServiceUnavailableError('Model manager not initialized');
        }

        // Check if model is available
        const availableModels = dockerService.listAvailableModels();
        if (!availableModels.includes(model as string)) {
          logger.error(`Model ${model} not available. Available models: ${availableModels}`);
          throw new NotFoundError(`Model ${model} not available`);
        }

        // Send transcription start notification via WebSocket
        const io = (req as any).io;
        if (io) {
          io.emit('transcription_started', { filename, model });
        }

        // Progress callback for WebSocket updates
        const progressCallback = (progress: number, status: string) => {
          if (io) {
            io.emit('transcription_progress', {
              filename,
              progress: Math.round(progress * 100),
              status
            });
          }
        };

        try {
          // Transcribe the audio file
          logger.info(`Starting transcription of ${filename} with model ${model}`);
          
          const result = await audioService.transcribeAudio(
            cleanedBuffer,
            {
              model: model as string,
              responseFormat: response_format as 'json' | 'text' | 'srt'
            },
            progressCallback
          );

          // Ensure result has Docker-specific fields
          if (!result.model_id) {
            result.model_id = model as string;
          }
          if (!result.backend) {
            result.backend = 'docker';
          }

          // Get container information if available
          try {
            const container = dockerService.getContainerInstance(model as string);
            if (container) {
              (result as any).container_info = {
                status: container.status,
                gpu_allocated: container.gpuMemoryMb > 0,
                image: dockerService.getContainerInfo(model as string)?.image
              };
            }
          } catch (e) {
            logger.warning(`Failed to get container info for ${model}:`, e);
          }

          // Send transcription complete notification
          if (io) {
            io.emit('transcription_completed', {
              filename,
              result_length: result.text?.length || 0,
              model,
              processing_time: result.processing_time || 0
            });
          }

          logger.info(`Transcription completed for ${filename} with ${result.text?.length || 0} characters`);

          // Return the transcription result
          if (response_format === 'json') {
            res.json(result);
          } else if (response_format === 'text') {
            res.type('text/plain').send(result.text);
          } else if (response_format === 'srt') {
            // Convert segments to SRT format
            if (result.segments) {
              const srtContent = result.segments.map((segment, index) => {
                const startTime = new Date(segment.start * 1000).toISOString().substr(11, 12).replace('.', ',');
                const endTime = new Date(segment.end * 1000).toISOString().substr(11, 12).replace('.', ',');
                return `${index + 1}\n${startTime} --> ${endTime}\n${segment.text}\n`;
              }).join('\n');
              res.type('text/plain').send(srtContent);
            } else {
              res.type('text/plain').send(result.text);
            }
          }

        } catch (transcriptionError) {
          // Send transcription error notification
          const errorMsg = transcriptionError instanceof Error ? transcriptionError.message : 'Unknown error';
          logger.error(`Transcription failed for ${filename}: ${errorMsg}`, transcriptionError);

          if (io) {
            io.emit('transcription_error', {
              filename,
              error: errorMsg,
              model
            });
          }

          throw transcriptionError;
        }

      } catch (error) {
        logger.error('Error during transcription:', error);
        throw error;
      }
    });
  } catch (error) {
    logger.error('Failed to transcribe audio:', error);
    throw error;
  }
};