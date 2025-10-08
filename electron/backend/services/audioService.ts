import ffmpeg from 'fluent-ffmpeg';
import path from 'path';
import fs from 'fs';
import { EventEmitter } from 'events';
import { logger } from '../utils/logger';
import { AudioError } from '../utils/errors';

export interface AudioConversionOptions {
  targetSampleRate?: number;
  targetChannels?: number;
  targetBitrate?: string;
  format?: string;
}

export interface AudioMetadata {
  duration?: number;
  sampleRate?: number;
  channels?: number;
  bitrate?: number;
  format?: string;
}

export interface TranscriptionOptions {
  model?: string;
  language?: string;
  responseFormat?: 'json' | 'text' | 'srt';
  temperature?: number;
  bestOf?: number;
}

export interface TranscriptionResult {
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

class AudioService extends EventEmitter {
  private supportedAudioFormats = ['.wav', '.mp3', '.m4a', '.flac', '.ogg', '.aac', '.wma', '.aiff', '.opus'];
  private supportedVideoFormats = ['.mp4', '.avi', '.mkv', '.mov', '.flv', '.webm', '.3gp', '.wmv', '.mpg', '.mpeg', '.m4v', '.ts', '.f4v'];
  private tempDir: string;

  constructor() {
    super();
    this.tempDir = path.join(process.cwd(), 'temp');
    this.ensureTempDir();
  }

  private ensureTempDir(): void {
    if (!fs.existsSync(this.tempDir)) {
      fs.mkdirSync(this.tempDir, { recursive: true });
    }
  }

  public getSupportedFormats(): { audio: string[]; video: string[] } {
    return {
      audio: this.supportedAudioFormats,
      video: this.supportedVideoFormats
    };
  }

  public isFormatSupported(filename: string): boolean {
    const ext = path.extname(filename).toLowerCase();
    return this.supportedAudioFormats.includes(ext) || this.supportedVideoFormats.includes(ext);
  }

  public validateAudioFile(buffer: Buffer, filename?: string): { isValid: boolean; error?: string } {
    // Check minimum file size
    if (buffer.length < 100) {
      return { isValid: false, error: 'File is too small to be a valid audio file' };
    }

    // Check file extension if provided
    if (filename && !this.isFormatSupported(filename)) {
      return { 
        isValid: false, 
        error: 'Unsupported file format. Please upload audio files (WAV, MP3, M4A, FLAC, OGG, etc.) or video files (MP4, AVI, MKV, MOV, WebM, etc.)' 
      };
    }

    // Check for common audio file signatures
    const audioSignatures = [
      { signature: Buffer.from('ID3'), format: 'MP3' },
      { signature: Buffer.from([0xFF, 0xFB]), format: 'MP3' },
      { signature: Buffer.from([0xFF, 0xF3]), format: 'MP3' },
      { signature: Buffer.from([0xFF, 0xF2]), format: 'MP3' },
      { signature: Buffer.from('RIFF'), format: 'WAV' },
      { signature: Buffer.from('fLaC'), format: 'FLAC' },
      { signature: Buffer.from([0x00, 0x00, 0x00, 0x20, 0x66, 0x74, 0x79, 0x70, 0x4D, 0x34, 0x41]), format: 'M4A' }
    ];

    for (const { signature, format } of audioSignatures) {
      if (buffer.subarray(0, signature.length).equals(signature)) {
        return { isValid: true };
      }
    }

    // If no signature found, but file extension is supported, assume it's valid
    if (filename && this.isFormatSupported(filename)) {
      return { isValid: true };
    }

    return { isValid: false, error: 'Unable to determine file format' };
  }

  public extractAudioData(buffer: Buffer, originalSize: number): Buffer {
    try {
      // Audio file signatures to look for
      const audioSignatures = [
        { signature: Buffer.from('ID3'), format: 'mp3' },
        { signature: Buffer.from([0xFF, 0xFB]), format: 'mp3' },
        { signature: Buffer.from([0xFF, 0xF3]), format: 'mp3' },
        { signature: Buffer.from([0xFF, 0xF2]), format: 'mp3' },
        { signature: Buffer.from('RIFF'), format: 'wav' },
        { signature: Buffer.from('fLaC'), format: 'flac' },
        { signature: Buffer.from([0x00, 0x00, 0x00, 0x20, 0x66, 0x74, 0x79, 0x70, 0x4D, 0x34, 0x41]), format: 'm4a' }
      ];

      // First, check if the data is already clean
      for (const { signature } of audioSignatures) {
        if (buffer.subarray(0, signature.length).equals(signature)) {
          logger.info('Data appears to be clean audio file');
          return buffer;
        }
      }

      // Look for multipart form data corruption patterns
      if (buffer.includes('Content-Disposition') || buffer.includes('form-data')) {
        logger.info('Detected multipart form data corruption, attempting to extract audio data');

        // Try to find audio data within the multipart mess
        for (const { signature, format } of audioSignatures) {
          const signaturePos = buffer.indexOf(signature);
          if (signaturePos > 0) {
            logger.info(`Found ${format.toUpperCase()} signature at position ${signaturePos}, extracting audio data`);

            // Extract everything from the signature onwards
            let extractedData = buffer.subarray(signaturePos);

            // For multipart data, we might have trailing boundaries
            if (format === 'mp3') {
              // Look for the end of valid MP3 data
              const boundaryPatterns = ['--', 'Content-', 'form-data'];
              
              for (const pattern of boundaryPatterns) {
                const patternBuffer = Buffer.from(pattern);
                const boundaryPos = extractedData.indexOf(patternBuffer, Math.floor(extractedData.length / 2));
                if (boundaryPos > 0) {
                  logger.info(`Found trailing boundary at position ${boundaryPos}, truncating`);
                  extractedData = extractedData.subarray(0, boundaryPos);
                  break;
                }
              }
            }

            logger.info(`Extracted ${extractedData.length} bytes of ${format.toUpperCase()} data (from ${originalSize} bytes)`);
            return extractedData;
          }
        }
      }

      logger.warning('Could not extract audio data from corrupted upload, using original data');
      return buffer;
    } catch (error) {
      logger.warning(`Error during audio data extraction: ${error}, using original data`);
      return buffer;
    }
  }

  public async convertToWav(
    inputBuffer: Buffer,
    originalFilename: string,
    options: AudioConversionOptions = {}
  ): Promise<{ buffer: Buffer; metadata: AudioMetadata }> {
    return new Promise((resolve, reject) => {
      const {
        targetSampleRate = 16000,
        targetChannels = 1,
        targetBitrate = '128k',
        format = 'wav'
      } = options;

      // Determine input file extension
      const ext = path.extname(originalFilename).toLowerCase();
      const isVideo = this.supportedVideoFormats.includes(ext);
      
      // Create temporary files
      const inputTempFile = path.join(this.tempDir, `input_${Date.now()}${ext}`);
      const outputTempFile = path.join(this.tempDir, `output_${Date.now()}.wav`);

      // Write input buffer to temporary file
      fs.writeFileSync(inputTempFile, inputBuffer);

      // Build FFmpeg command
      let command = ffmpeg(inputTempFile);

      if (isVideo) {
        // For video files, extract audio stream
        command = command
          .noVideo()
          .audioCodec('pcm_s16le')
          .audioFrequency(targetSampleRate)
          .audioChannels(targetChannels);
      } else {
        // For audio files, convert to desired format
        command = command
          .audioCodec('pcm_s16le')
          .audioFrequency(targetSampleRate)
          .audioChannels(targetChannels);
      }

      // Set output format
      command = command.format(format);

      // Event handlers
      command
        .on('start', (commandLine) => {
          logger.debug(`FFmpeg started: ${commandLine}`);
        })
        .on('progress', (progress) => {
          logger.debug(`FFmpeg progress: ${progress.percent}%`);
          this.emit('conversionProgress', { percent: progress.percent });
        })
        .on('end', () => {
          try {
            // Read the output file
            const outputBuffer = fs.readFileSync(outputTempFile);
            
            // Get metadata
            const metadata: AudioMetadata = {
              format: 'wav',
              sampleRate: targetSampleRate,
              channels: targetChannels
            };

            // Clean up temporary files
            fs.unlinkSync(inputTempFile);
            fs.unlinkSync(outputTempFile);

            logger.info(`Audio conversion completed: ${originalFilename} -> WAV`);
            resolve({ buffer: outputBuffer, metadata });
          } catch (error) {
            reject(new AudioError(`Failed to read converted file: ${error.message}`));
          }
        })
        .on('error', (err) => {
          // Clean up temporary files on error
          try {
            if (fs.existsSync(inputTempFile)) fs.unlinkSync(inputTempFile);
            if (fs.existsSync(outputTempFile)) fs.unlinkSync(outputTempFile);
          } catch (cleanupError) {
            logger.warn('Failed to clean up temporary files:', cleanupError);
          }
          
          logger.error(`FFmpeg error: ${err.message}`);
          reject(new AudioError(`Audio conversion failed: ${err.message}`));
        });

      // Start the conversion
      command.save(outputTempFile);
    });
  }

  public async getAudioMetadata(buffer: Buffer, filename: string): Promise<AudioMetadata> {
    return new Promise((resolve, reject) => {
      const ext = path.extname(filename).toLowerCase();
      const inputTempFile = path.join(this.tempDir, `metadata_${Date.now()}${ext}`);
      
      // Write buffer to temporary file
      fs.writeFileSync(inputTempFile, buffer);

      ffmpeg.ffprobe(inputTempFile, (err, metadata) => {
        // Clean up temporary file
        try {
          fs.unlinkSync(inputTempFile);
        } catch (cleanupError) {
          logger.warn('Failed to clean up temporary file:', cleanupError);
        }

        if (err) {
          logger.error(`FFprobe error: ${err.message}`);
          reject(new AudioError(`Failed to get audio metadata: ${err.message}`));
          return;
        }

        const audioStream = metadata.streams.find(stream => stream.codec_type === 'audio');
        if (!audioStream) {
          reject(new AudioError('No audio stream found in file'));
          return;
        }

        const result: AudioMetadata = {
          duration: metadata.format.duration,
          sampleRate: audioStream.sample_rate,
          channels: audioStream.channels,
          bitrate: metadata.format.bit_rate,
          format: metadata.format.format_name
        };

        resolve(result);
      });
    });
  }

  public async transcribeAudio(
    audioBuffer: Buffer,
    options: TranscriptionOptions = {},
    progressCallback?: (progress: number, status: string) => void
  ): Promise<TranscriptionResult> {
    try {
      const {
        model = 'whisper-base',
        language,
        responseFormat = 'json',
        temperature = 0.0,
        bestOf = 1
      } = options;

      if (progressCallback) {
        progressCallback(0.1, 'Preparing audio for transcription...');
      }

      // Convert audio to WAV format
      const { buffer: wavBuffer, metadata } = await this.convertToWav(audioBuffer, 'audio.wav', {
        targetSampleRate: 16000,
        targetChannels: 1
      });

      if (progressCallback) {
        progressCallback(0.3, 'Connecting to transcription service...');
      }

      // Get Docker service and ensure model is available
      const { getDockerService } = await import('./dockerService');
      const dockerService = getDockerService();
      
      // Check if model container is running
      let container = dockerService.getContainerInstance(model);
      if (!container || container.status !== 'running') {
        if (progressCallback) {
          progressCallback(0.4, `Starting model container: ${model}...`);
        }
        
        container = await dockerService.startContainer(model);
        if (!container) {
          throw new AudioError(`Failed to start model container: ${model}`);
        }
      }

      if (progressCallback) {
        progressCallback(0.5, 'Sending audio to transcription service...');
      }

      // Create temporary file for transcription
      const tempAudioFile = path.join(this.tempDir, `transcribe_${Date.now()}.wav`);
      fs.writeFileSync(tempAudioFile, wavBuffer);

      try {
        // Send transcription request to container
        const result = await this.sendTranscriptionRequest(container, tempAudioFile, {
          language,
          responseFormat,
          temperature,
          bestOf
        }, progressCallback);

        if (progressCallback) {
          progressCallback(0.9, 'Processing transcription result...');
        }

        // Add metadata to result
        result.model_id = model;
        result.backend = 'docker';
        result.duration = metadata.duration;

        if (progressCallback) {
          progressCallback(1.0, 'Transcription completed');
        }

        return result;
      } finally {
        // Clean up temporary file
        try {
          fs.unlinkSync(tempAudioFile);
        } catch (error) {
          logger.warn('Failed to clean up temporary audio file:', error);
        }
      }
    } catch (error) {
      logger.error('Transcription failed:', error);
      throw new AudioError(`Transcription failed: ${error.message}`);
    }
  }

  private async sendTranscriptionRequest(
    container: any,
    audioFile: string,
    options: any,
    progressCallback?: (progress: number, status: string) => void
  ): Promise<TranscriptionResult> {
    // This is a placeholder implementation
    // In a real implementation, you would send the audio file to the container
    // via HTTP request or other communication method
    
    if (progressCallback) {
      progressCallback(0.6, 'Processing audio with model...');
    }

    // Simulate processing time
    await new Promise(resolve => setTimeout(resolve, 2000));

    if (progressCallback) {
      progressCallback(0.8, 'Generating transcription...');
    }

    // Return mock result for now
    return {
      text: "This is a mock transcription result. In a real implementation, this would be the actual transcribed text from the audio file.",
      segments: [
        {
          id: 0,
          seek: 0,
          start: 0.0,
          end: 5.0,
          text: "This is a mock transcription result.",
          tokens: [50364, 366, 366, 264, 26841, 2991, 13],
          temperature: 0.0,
          avg_logprob: -0.5,
          compression_ratio: 1.5,
          no_speech_prob: 0.1
        }
      ],
      language: "en",
      language_probability: 0.9,
      processing_time: 2.0
    };
  }

  public cleanup(): void {
    try {
      // Clean up temporary files
      if (fs.existsSync(this.tempDir)) {
        const files = fs.readdirSync(this.tempDir);
        for (const file of files) {
          const filePath = path.join(this.tempDir, file);
          fs.unlinkSync(filePath);
        }
      }
      logger.info('Audio service cleanup completed');
    } catch (error) {
      logger.error('Failed to cleanup audio service:', error);
    }
  }
}

// Singleton instance
let audioServiceInstance: AudioService | null = null;

export const getAudioService = (): AudioService => {
  if (!audioServiceInstance) {
    audioServiceInstance = new AudioService();
  }
  return audioServiceInstance;
};

export const setupAudioService = (io: any): AudioService => {
  const audioService = getAudioService();
  
  // Forward audio service events to Socket.IO
  audioService.on('conversionProgress', (data) => {
    io.emit('audio:conversionProgress', data);
  });
  
  logger.info('Audio service setup completed');
  return audioService;
};