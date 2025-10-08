// Error handling utilities will be implemented when migrating backend
// This file will contain custom error classes and error handling functions

export class ASRProError extends Error {
  constructor(
    message: string,
    public code: string,
    public statusCode: number = 500
  ) {
    super(message);
    this.name = 'ASRProError';
  }
}

export class DockerError extends ASRProError {
  constructor(message: string, code: string = 'DOCKER_ERROR') {
    super(message, code, 500);
    this.name = 'DockerError';
  }
}

export class AudioError extends ASRProError {
  constructor(message: string, code: string = 'AUDIO_ERROR') {
    super(message, code, 500);
    this.name = 'AudioError';
  }
}

export class ConfigError extends ASRProError {
  constructor(message: string, code: string = 'CONFIG_ERROR') {
    super(message, code, 500);
    this.name = 'ConfigError';
  }
}

// Error handler middleware
export const errorHandler = (err: any, req: any, res: any, next: any) => {
  if (err instanceof ASRProError) {
    return res.status(err.statusCode).json({
      error: {
        code: err.code,
        message: err.message
      }
    });
  }

  // Default error
  return res.status(500).json({
    error: {
      code: 'INTERNAL_ERROR',
      message: 'An unexpected error occurred'
    }
  });
};