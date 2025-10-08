import { logger } from './logger';

export class ASRProError extends Error {
  constructor(
    message: string,
    public code: string,
    public statusCode: number = 500,
    public details?: any
  ) {
    super(message);
    this.name = 'ASRProError';
    
    // Log the error
    logger.error(`[${this.code}] ${message}`, { details, stack: this.stack });
  }

  public toJSON() {
    return {
      name: this.name,
      code: this.code,
      message: this.message,
      statusCode: this.statusCode,
      details: this.details
    };
  }
}

export class DockerError extends ASRProError {
  constructor(message: string, code: string = 'DOCKER_ERROR', details?: any) {
    super(message, code, 500, details);
    this.name = 'DockerError';
  }
}

export class AudioError extends ASRProError {
  constructor(message: string, code: string = 'AUDIO_ERROR', details?: any) {
    super(message, code, 500, details);
    this.name = 'AudioError';
  }
}

export class ConfigError extends ASRProError {
  constructor(message: string, code: string = 'CONFIG_ERROR', details?: any) {
    super(message, code, 500, details);
    this.name = 'ConfigError';
  }
}

export class ValidationError extends ASRProError {
  constructor(message: string, details?: any) {
    super(message, 'VALIDATION_ERROR', 400, details);
    this.name = 'ValidationError';
  }
}

export class NotFoundError extends ASRProError {
  constructor(message: string, details?: any) {
    super(message, 'NOT_FOUND', 404, details);
    this.name = 'NotFoundError';
  }
}

export class ServiceUnavailableError extends ASRProError {
  constructor(message: string, details?: any) {
    super(message, 'SERVICE_UNAVAILABLE', 503, details);
    this.name = 'ServiceUnavailableError';
  }
}

// Error handler middleware for Express
export const errorHandler = (err: any, req: any, res: any, next: any) => {
  // If the error is already an ASRProError, use it directly
  if (err instanceof ASRProError) {
    return res.status(err.statusCode).json({
      error: {
        code: err.code,
        message: err.message,
        ...(err.details && { details: err.details })
      }
    });
  }

  // Handle Multer errors
  if (err.code === 'LIMIT_FILE_SIZE') {
    return res.status(413).json({
      error: {
        code: 'FILE_TOO_LARGE',
        message: 'File size exceeds the maximum allowed limit'
      }
    });
  }

  if (err.code === 'LIMIT_FILE_COUNT') {
    return res.status(413).json({
      error: {
        code: 'TOO_MANY_FILES',
        message: 'Too many files uploaded'
      }
    });
  }

  if (err.code === 'LIMIT_UNEXPECTED_FILE') {
    return res.status(400).json({
      error: {
        code: 'UNEXPECTED_FILE',
        message: 'Unexpected file field'
      }
    });
  }

  // Handle JSON parsing errors
  if (err.type === 'entity.parse.failed') {
    return res.status(400).json({
      error: {
        code: 'INVALID_JSON',
        message: 'Invalid JSON in request body'
      }
    });
  }

  // Handle syntax errors
  if (err instanceof SyntaxError) {
    return res.status(400).json({
      error: {
        code: 'SYNTAX_ERROR',
        message: 'Syntax error in request'
      }
    });
  }

  // Default error
  logger.error('Unhandled error:', err);
  
  return res.status(500).json({
    error: {
      code: 'INTERNAL_ERROR',
      message: process.env.NODE_ENV === 'production' 
        ? 'An unexpected error occurred' 
        : err.message || 'An unexpected error occurred',
      ...(process.env.NODE_ENV !== 'production' && { stack: err.stack })
    }
  });
};

// Async error wrapper for route handlers
export const asyncHandler = (fn: Function) => {
  return (req: any, res: any, next: any) => {
    Promise.resolve(fn(req, res, next)).catch(next);
  };
};

// Error types for different scenarios
export const ErrorTypes = {
  // Docker errors
  DOCKER_NOT_AVAILABLE: 'DOCKER_NOT_AVAILABLE',
  CONTAINER_START_FAILED: 'CONTAINER_START_FAILED',
  CONTAINER_STOP_FAILED: 'CONTAINER_STOP_FAILED',
  MODEL_NOT_FOUND: 'MODEL_NOT_FOUND',
  GPU_ALLOCATION_FAILED: 'GPU_ALLOCATION_FAILED',
  
  // Audio errors
  AUDIO_CONVERSION_FAILED: 'AUDIO_CONVERSION_FAILED',
  UNSUPPORTED_AUDIO_FORMAT: 'UNSUPPORTED_AUDIO_FORMAT',
  AUDIO_FILE_TOO_LARGE: 'AUDIO_FILE_TOO_LARGE',
  AUDIO_FILE_CORRUPTED: 'AUDIO_FILE_CORRUPTED',
  TRANSCRIPTION_FAILED: 'TRANSCRIPTION_FAILED',
  
  // Configuration errors
  CONFIG_LOAD_FAILED: 'CONFIG_LOAD_FAILED',
  CONFIG_SAVE_FAILED: 'CONFIG_SAVE_FAILED',
  INVALID_CONFIG: 'INVALID_CONFIG',
  
  // Validation errors
  MISSING_REQUIRED_FIELD: 'MISSING_REQUIRED_FIELD',
  INVALID_FILE_TYPE: 'INVALID_FILE_TYPE',
  INVALID_PARAMETER: 'INVALID_PARAMETER',
  
  // System errors
  SERVICE_UNAVAILABLE: 'SERVICE_UNAVAILABLE',
  INTERNAL_ERROR: 'INTERNAL_ERROR',
  NOT_FOUND: 'NOT_FOUND'
} as const;

// Error factory functions
export const createDockerError = (message: string, details?: any): DockerError => {
  return new DockerError(message, ErrorTypes.DOCKER_NOT_AVAILABLE, details);
};

export const createAudioError = (message: string, details?: any): AudioError => {
  return new AudioError(message, ErrorTypes.AUDIO_CONVERSION_FAILED, details);
};

export const createConfigError = (message: string, details?: any): ConfigError => {
  return new ConfigError(message, ErrorTypes.CONFIG_LOAD_FAILED, details);
};

export const createValidationError = (message: string, details?: any): ValidationError => {
  return new ValidationError(message, details);
};

export const createNotFoundError = (message: string, details?: any): NotFoundError => {
  return new NotFoundError(message, details);
};

export const createServiceUnavailableError = (message: string, details?: any): ServiceUnavailableError => {
  return new ServiceUnavailableError(message, details);
};