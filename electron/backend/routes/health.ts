import { Request, Response } from 'express';
import { getDockerService } from '../services/dockerService';
import { logger } from '../utils/logger';
import { ServiceUnavailableError } from '../utils/errors';

export interface HealthResponse {
  status: 'healthy' | 'degraded' | 'ready' | 'unhealthy' | 'initializing';
  current_model?: string;
  device: string;
}

export const healthCheck = async (req: Request, res: Response): Promise<void> => {
  try {
    logger.debug('Health check requested');
    
    const dockerService = getDockerService();
    
    if (!dockerService) {
      logger.warning('Docker service not initialized in health check');
      res.json({
        status: 'initializing',
        current_model: undefined,
        device: 'Docker container'
      } as HealthResponse);
      return;
    }
    
    // Get system status from Docker service
    const systemStatus = await dockerService.getSystemStatus();
    
    // Determine overall health status
    let status: HealthResponse['status'] = 'healthy';
    if (!systemStatus.dockerAvailable) {
      status = 'degraded'; // Docker not available
    } else if (!systemStatus.currentModel) {
      status = 'ready'; // Docker available but no model loaded
    }
    
    const device = systemStatus.gpuAvailable 
      ? 'Docker container (GPU)' 
      : 'Docker container (CPU)';
    
    logger.debug(`Health check result: status=${status}, model=${systemStatus.currentModel}, device=${device}`);
    
    res.json({
      status,
      current_model: systemStatus.currentModel,
      device
    } as HealthResponse);
  } catch (error) {
    logger.error('Health check failed:', error);
    res.json({
      status: 'unhealthy'
    } as HealthResponse);
  }
};