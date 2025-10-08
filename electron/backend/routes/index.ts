import { Router } from 'express';
import { healthCheck } from './health';
import { getModels, setModel } from './models';
import { transcribeAudio } from './transcription';
import { getOptions } from './options';
import { asyncHandler } from '../utils/errors';

const router = Router();

// Health check endpoint
router.get('/health', asyncHandler(healthCheck));

// Model management endpoints
router.get('/v1/models', asyncHandler(getModels));
router.post('/v1/settings/model', asyncHandler(setModel));

// Transcription endpoint
router.post('/v1/audio/transcriptions', asyncHandler(transcribeAudio));

// Options endpoint
router.get('/v1/options', asyncHandler(getOptions));

export default router;