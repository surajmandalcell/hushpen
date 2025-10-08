import { Request, Response } from 'express';
import { getDockerService } from '../services/dockerService';
import { logger } from '../utils/logger';
import { ServiceUnavailableError, NotFoundError, ValidationError } from '../utils/errors';

export interface ModelResponse {
  id: string;
  ready: boolean;
}

export interface ModelListResponse {
  data: ModelResponse[];
}

export interface ModelSettingRequest {
  model_id: string;
}

export interface ModelSettingResponse {
  status: 'success' | 'error';
  model: string;
}

export const getModels = async (req: Request, res: Response): Promise<void> => {
  try {
    logger.debug('Listing available models');
    
    const dockerService = getDockerService();
    
    if (!dockerService) {
      logger.warning('Docker service not initialized when listing models');
      throw new ServiceUnavailableError('Model manager not initialized');
    }
    
    // Get available models from Docker service
    const availableModels = dockerService.listAvailableModels();
    logger.info(`Found ${availableModels.length} available models`);
    
    // Get model info for each available model
    const modelResponses: ModelResponse[] = [];
    for (const modelId of availableModels) {
      try {
        const container = dockerService.getContainerInstance(modelId);
        const isRunning = container?.status === 'running';
        
        modelResponses.push({
          id: modelId,
          ready: isRunning
        });
        
        logger.debug(`Model ${modelId}: ready=${isRunning}`);
      } catch (error) {
        logger.error(`Error getting info for model ${modelId}:`, error);
        // Continue with other models even if one fails
      }
    }
    
    logger.info(`Returning ${modelResponses.length} models in response`);
    res.json({
      data: modelResponses
    } as ModelListResponse);
  } catch (error) {
    logger.error('Failed to list models:', error);
    throw error;
  }
};

export const setModel = async (req: Request, res: Response): Promise<void> => {
  try {
    const { model_id } = req.body as ModelSettingRequest;
    
    if (!model_id) {
      throw new ValidationError('Model ID is required');
    }
    
    logger.info(`Setting model to: ${model_id}`);
    
    const dockerService = getDockerService();
    
    if (!dockerService) {
      logger.error('Docker service not initialized when setting model');
      throw new ServiceUnavailableError('Model manager not initialized');
    }
    
    // Validate model exists
    const availableModels = dockerService.listAvailableModels();
    if (!availableModels.includes(model_id)) {
      logger.warning(`Model ${model_id} not found in available models: ${availableModels}`);
      throw new NotFoundError(`Model ${model_id} not found`);
    }
    
    // Start the container for the model
    const container = await dockerService.startContainer(model_id);
    
    if (container) {
      logger.info(`Successfully set model to ${model_id}`);
      res.json({
        status: 'success',
        model: model_id
      } as ModelSettingResponse);
    } else {
      logger.error(`Failed to set model to ${model_id}`);
      res.json({
        status: 'error',
        model: model_id
      } as ModelSettingResponse);
    }
  } catch (error) {
    logger.error('Failed to set model:', error);
    throw error;
  }
};