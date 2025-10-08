import Docker from 'dockerode';
import { EventEmitter } from 'events';
import { logger } from '../utils/logger';
import { DockerError } from '../utils/errors';
import { getConfigService } from './configService';

export interface ContainerInfo {
  id: string;
  name: string;
  description: string;
  family: string;
  size: string;
  languages: string[];
  sample_rate: number;
  gpu_memory_mb: number;
  image: string;
  port: number;
  environment?: Record<string, string>;
  volumes?: Record<string, string>;
  restart_policy?: string;
}

export interface ContainerInstance {
  containerId: string;
  dockerContainer: Docker.Container;
  modelId: string;
  status: ContainerStatus;
  createdAt: number;
  startedAt?: number;
  lastActivity: number;
  healthCheckCount: number;
  errorCount: number;
  gpuMemoryMb: number;
}

export enum ContainerStatus {
  STOPPED = 'stopped',
  STARTING = 'starting',
  RUNNING = 'running',
  STOPPING = 'stopping',
  ERROR = 'error',
  UNKNOWN = 'unknown'
}

export interface GPUAllocation {
  modelId: string;
  memoryMb: number;
  allocatedAt: number;
  lastActivity: number;
}

export interface SystemStatus {
  status: string;
  currentModel?: string;
  dockerAvailable: boolean;
  gpuAvailable: boolean;
  gpuUtilization?: Record<string, any>;
  containers?: Record<string, any>;
  connections?: Record<string, any>;
  registry?: Record<string, any>;
}

class DockerService extends EventEmitter {
  private docker: Docker;
  private configService = getConfigService();
  private activeContainers: Map<string, ContainerInstance> = new Map();
  private gpuAllocations: Map<string, GPUAllocation> = new Map();
  private isInitialized = false;
  private cleanupInterval?: NodeJS.Timeout;

  constructor() {
    super();
    this.docker = new Docker();
  }

  public async initialize(): Promise<boolean> {
    try {
      logger.info('Initializing Docker service...');
      
      // Check Docker availability
      await this.docker.ping();
      logger.info('Docker is available');
      
      // Check GPU availability
      const gpuAvailable = await this.checkGPUAvailability();
      logger.info(`GPU availability: ${gpuAvailable}`);
      
      // Start cleanup interval
      this.startCleanupInterval();
      
      this.isInitialized = true;
      this.emit('initialized');
      logger.info('Docker service initialized successfully');
      
      return true;
    } catch (error) {
      logger.error('Failed to initialize Docker service:', error);
      throw new DockerError(`Docker initialization failed: ${error.message}`);
    }
  }

  public async checkDockerAvailability(): Promise<boolean> {
    try {
      await this.docker.ping();
      return true;
    } catch (error) {
      logger.warn('Docker is not available:', error);
      return false;
    }
  }

  private async checkGPUAvailability(): Promise<boolean> {
    try {
      // Try to list GPU devices using nvidia-docker
      const result = await this.execCommand('nvidia-smi', [], true);
      return result.exitCode === 0;
    } catch (error) {
      logger.warn('GPU is not available:', error);
      return false;
    }
  }

  private async execCommand(command: string, args: string[], silent = false): Promise<{ exitCode: number; output: string }> {
    try {
      const output = await new Promise<string>((resolve, reject) => {
        const { exec } = require('child_process');
        const process = exec(`${command} ${args.join(' ')}`, (error, stdout, stderr) => {
          if (error && !silent) {
            reject(error);
          } else {
            resolve(stdout || stderr);
          }
        });
      });
      
      return { exitCode: 0, output };
    } catch (error) {
      return { exitCode: 1, output: '' };
    }
  }

  public getContainerRegistry(): ContainerInfo[] {
    // Default container registry - in a real implementation, this would be loaded from config
    return [
      {
        id: 'whisper-tiny',
        name: 'Whisper Tiny',
        description: 'Fastest Whisper model, lower accuracy',
        family: 'whisper',
        size: 'tiny',
        languages: ['en', 'zh', 'de', 'es', 'ru', 'ko', 'fr', 'ja', 'pt', 'tr', 'pl', 'ca', 'nl', 'ar', 'sv', 'it', 'id', 'hi', 'fi', 'vi', 'he', 'uk', 'el', 'ms', 'cs', 'ro', 'da', 'hu', 'ta', 'no', 'th', 'ur', 'hr', 'bg', 'lt', 'la', 'mi', 'ml', 'cy', 'sk', 'te', 'fa', 'lv', 'bn', 'sr', 'az', 'sl', 'kn', 'et', 'mk', 'br', 'eu', 'is', 'hy', 'ne', 'mn', 'bs', 'kk', 'sq', 'sw', 'gl', 'mr', 'pa', 'si', 'km', 'sn', 'yo', 'so', 'af', 'oc', 'ka', 'be', 'tg', 'sd', 'gu', 'am', 'yi', 'lo', 'uz', 'fo', 'ht', 'ps', 'tk', 'nn', 'mt', 'sa', 'lb', 'my', 'bo', 'tl', 'mg', 'as', 'tt', 'haw', 'ln', 'ha', 'ba', 'jw', 'su'],
        sample_rate: 16000,
        gpu_memory_mb: 1024,
        image: 'asrpro/whisper-tiny:latest',
        port: 8001,
        environment: {
          'MODEL_SIZE': 'tiny',
          'SAMPLE_RATE': '16000'
        },
        volumes: {},
        restart_policy: 'unless-stopped'
      },
      {
        id: 'whisper-base',
        name: 'Whisper Base',
        description: 'Good balance of speed and accuracy',
        family: 'whisper',
        size: 'base',
        languages: ['en', 'zh', 'de', 'es', 'ru', 'ko', 'fr', 'ja', 'pt', 'tr', 'pl', 'ca', 'nl', 'ar', 'sv', 'it', 'id', 'hi', 'fi', 'vi', 'he', 'uk', 'el', 'ms', 'cs', 'ro', 'da', 'hu', 'ta', 'no', 'th', 'ur', 'hr', 'bg', 'lt', 'la', 'mi', 'ml', 'cy', 'sk', 'te', 'fa', 'lv', 'bn', 'sr', 'az', 'sl', 'kn', 'et', 'mk', 'br', 'eu', 'is', 'hy', 'ne', 'mn', 'bs', 'kk', 'sq', 'sw', 'gl', 'mr', 'pa', 'si', 'km', 'sn', 'yo', 'so', 'af', 'oc', 'ka', 'be', 'tg', 'sd', 'gu', 'am', 'yi', 'lo', 'uz', 'fo', 'ht', 'ps', 'tk', 'nn', 'mt', 'sa', 'lb', 'my', 'bo', 'tl', 'mg', 'as', 'tt', 'haw', 'ln', 'ha', 'ba', 'jw', 'su'],
        sample_rate: 16000,
        gpu_memory_mb: 2048,
        image: 'asrpro/whisper-base:latest',
        port: 8002,
        environment: {
          'MODEL_SIZE': 'base',
          'SAMPLE_RATE': '16000'
        },
        volumes: {},
        restart_policy: 'unless-stopped'
      },
      {
        id: 'whisper-small',
        name: 'Whisper Small',
        description: 'Better accuracy, slower',
        family: 'whisper',
        size: 'small',
        languages: ['en', 'zh', 'de', 'es', 'ru', 'ko', 'fr', 'ja', 'pt', 'tr', 'pl', 'ca', 'nl', 'ar', 'sv', 'it', 'id', 'hi', 'fi', 'vi', 'he', 'uk', 'el', 'ms', 'cs', 'ro', 'da', 'hu', 'ta', 'no', 'th', 'ur', 'hr', 'bg', 'lt', 'la', 'mi', 'ml', 'cy', 'sk', 'te', 'fa', 'lv', 'bn', 'sr', 'az', 'sl', 'kn', 'et', 'mk', 'br', 'eu', 'is', 'hy', 'ne', 'mn', 'bs', 'kk', 'sq', 'sw', 'gl', 'mr', 'pa', 'si', 'km', 'sn', 'yo', 'so', 'af', 'oc', 'ka', 'be', 'tg', 'sd', 'gu', 'am', 'yi', 'lo', 'uz', 'fo', 'ht', 'ps', 'tk', 'nn', 'mt', 'sa', 'lb', 'my', 'bo', 'tl', 'mg', 'as', 'tt', 'haw', 'ln', 'ha', 'ba', 'jw', 'su'],
        sample_rate: 16000,
        gpu_memory_mb: 4096,
        image: 'asrpro/whisper-small:latest',
        port: 8003,
        environment: {
          'MODEL_SIZE': 'small',
          'SAMPLE_RATE': '16000'
        },
        volumes: {},
        restart_policy: 'unless-stopped'
      }
    ];
  }

  public listAvailableModels(): string[] {
    const registry = this.getContainerRegistry();
    return registry.map(container => container.id);
  }

  public getContainerInfo(modelId: string): ContainerInfo | null {
    const registry = this.getContainerRegistry();
    return registry.find(container => container.id === modelId) || null;
  }

  public async startContainer(modelId: string): Promise<ContainerInstance | null> {
    if (!this.isInitialized) {
      throw new DockerError('Docker service not initialized');
    }

    const containerInfo = this.getContainerInfo(modelId);
    if (!containerInfo) {
      throw new DockerError(`Model ${modelId} not found in registry`);
    }

    // Check if container is already running
    if (this.activeContainers.has(modelId)) {
      const existingContainer = this.activeContainers.get(modelId)!;
      if (existingContainer.status === ContainerStatus.RUNNING) {
        logger.info(`Container for model ${modelId} is already running`);
        return existingContainer;
      }
    }

    // Allocate GPU memory if needed
    if (containerInfo.gpu_memory_mb > 0) {
      if (!this.allocateGPUMemory(modelId, containerInfo.gpu_memory_mb)) {
        throw new DockerError(`Failed to allocate GPU memory for model ${modelId}`);
      }
    }

    try {
      // Pull image if not available locally
      await this.ensureImageAvailable(containerInfo.image);

      // Create container configuration
      const containerConfig: Docker.ContainerCreateOptions = {
        Image: containerInfo.image,
        ExposedPorts: { [`${containerInfo.port}/tcp`]: {} },
        Env: Object.entries(containerInfo.environment || {}).map(([key, value]) => `${key}=${value}`),
        HostConfig: {
          PortBindings: { [`${containerInfo.port}/tcp`]: [{ HostPort: `${containerInfo.port}` }] },
          RestartPolicy: { Name: containerInfo.restart_policy || 'unless-stopped' }
        }
      };

      // Add GPU configuration if available
      const gpuAvailable = await this.checkGPUAvailability();
      if (gpuAvailable && containerInfo.gpu_memory_mb > 0) {
        containerConfig.HostConfig!.DeviceRequests = [
          {
            Driver: 'nvidia',
            Count: -1, // All GPUs
            Capabilities: [['gpu', 'utility']]
          }
        ];
      }

      // Create and start container
      const container = await this.docker.createContainer(containerConfig);
      await container.start();

      // Create container instance
      const containerInstance: ContainerInstance = {
        containerId: container.id,
        dockerContainer: container,
        modelId,
        status: ContainerStatus.STARTING,
        createdAt: Date.now(),
        lastActivity: Date.now(),
        healthCheckCount: 0,
        errorCount: 0,
        gpuMemoryMb: containerInfo.gpu_memory_mb
      };

      this.activeContainers.set(modelId, containerInstance);

      // Wait for container to be ready
      const isReady = await this.waitForContainerReady(containerInstance);
      if (isReady) {
        containerInstance.status = ContainerStatus.RUNNING;
        containerInstance.startedAt = Date.now();
        logger.info(`Container for model ${modelId} started successfully`);
        this.emit('containerStarted', { modelId, containerInstance });
        return containerInstance;
      } else {
        // Failed to start, cleanup
        await this.stopContainer(modelId);
        throw new DockerError(`Container for model ${modelId} failed to become ready`);
      }
    } catch (error) {
      // Release GPU memory on failure
      if (containerInfo.gpu_memory_mb > 0) {
        this.releaseGPUMemory(modelId);
      }
      logger.error(`Failed to start container for model ${modelId}:`, error);
      throw new DockerError(`Failed to start container: ${error.message}`);
    }
  }

  private async ensureImageAvailable(imageName: string): Promise<void> {
    try {
      await this.docker.getImage(imageName).inspect();
      logger.debug(`Image ${imageName} is available locally`);
    } catch (error) {
      logger.info(`Pulling Docker image ${imageName}...`);
      await new Promise<void>((resolve, reject) => {
        this.docker.pull(imageName, (err: any, stream: any) => {
          if (err) {
            reject(err);
            return;
          }
          
          this.docker.modem.followProgress(stream, (err: any) => {
            if (err) {
              reject(err);
            } else {
              resolve();
            }
          });
        });
      });
      logger.info(`Successfully pulled image ${imageName}`);
    }
  }

  private async waitForContainerReady(container: ContainerInstance, timeout = 60000): Promise<boolean> {
    const startTime = Date.now();
    
    while (Date.now() - startTime < timeout) {
      try {
        const data = await container.dockerContainer.inspect();
        const status = data.State.Status;
        
        if (status === 'running') {
          // Additional health check could be added here
          return true;
        } else if (['exited', 'dead', 'removing'].includes(status)) {
          logger.error(`Container ${container.containerId} exited with status: ${status}`);
          return false;
        }
        
        await new Promise(resolve => setTimeout(resolve, 1000));
      } catch (error) {
        logger.warn(`Error checking container readiness: ${error}`);
        await new Promise(resolve => setTimeout(resolve, 1000));
      }
    }
    
    logger.error(`Container ${container.containerId} did not become ready within ${timeout}ms`);
    return false;
  }

  public async stopContainer(modelId: string): Promise<boolean> {
    const container = this.activeContainers.get(modelId);
    if (!container) {
      logger.warning(`No active container found for model ${modelId}`);
      return false;
    }

    try {
      container.status = ContainerStatus.STOPPING;
      
      // Stop and remove Docker container
      await container.dockerContainer.stop();
      await container.dockerContainer.remove();
      
      // Release GPU memory
      if (container.gpuMemoryMb > 0) {
        this.releaseGPUMemory(modelId);
      }
      
      container.status = ContainerStatus.STOPPED;
      this.activeContainers.delete(modelId);
      
      logger.info(`Container for model ${modelId} stopped successfully`);
      this.emit('containerStopped', { modelId });
      return true;
    } catch (error) {
      container.status = ContainerStatus.ERROR;
      logger.error(`Failed to stop container for model ${modelId}:`, error);
      return false;
    }
  }

  private allocateGPUMemory(modelId: string, memoryMb: number): boolean {
    const totalMemory = this.configService.getDockerConfig().gpu_memory_total || 8192;
    const allocatedMemory = Array.from(this.gpuAllocations.values())
      .reduce((sum, allocation) => sum + allocation.memoryMb, 0);
    
    if (allocatedMemory + memoryMb > totalMemory) {
      logger.warn(`Insufficient GPU memory. Required: ${memoryMb}MB, Available: ${totalMemory - allocatedMemory}MB`);
      return false;
    }
    
    this.gpuAllocations.set(modelId, {
      modelId,
      memoryMb,
      allocatedAt: Date.now(),
      lastActivity: Date.now()
    });
    
    logger.info(`Allocated ${memoryMb}MB GPU memory for model ${modelId}`);
    return true;
  }

  private releaseGPUMemory(modelId: string): void {
    if (this.gpuAllocations.has(modelId)) {
      this.gpuAllocations.delete(modelId);
      logger.info(`Released GPU memory for model ${modelId}`);
    }
  }

  public updateContainerActivity(modelId: string): void {
    const container = this.activeContainers.get(modelId);
    if (container) {
      container.lastActivity = Date.now();
    }
    
    const allocation = this.gpuAllocations.get(modelId);
    if (allocation) {
      allocation.lastActivity = Date.now();
    }
  }

  public getContainerInstance(modelId: string): ContainerInstance | null {
    return this.activeContainers.get(modelId) || null;
  }

  public async getSystemStatus(): Promise<SystemStatus> {
    const dockerAvailable = await this.checkDockerAvailability();
    const gpuAvailable = await this.checkGPUAvailability();
    
    return {
      status: this.isInitialized ? 'initialized' : 'not_initialized',
      currentModel: this.getCurrentModel(),
      dockerAvailable,
      gpuAvailable,
      gpuUtilization: this.getGPUUtilization(),
      containers: this.getLifecycleSummary(),
      connections: {}, // TODO: Implement connection tracking
      registry: { availableModels: this.listAvailableModels() }
    };
  }

  private getCurrentModel(): string | undefined {
    for (const [modelId, container] of this.activeContainers) {
      if (container.status === ContainerStatus.RUNNING) {
        return modelId;
      }
    }
    return undefined;
  }

  private getGPUUtilization(): Record<string, any> {
    const allocations = Array.from(this.gpuAllocations.values());
    const totalAllocated = allocations.reduce((sum, alloc) => sum + alloc.memoryMb, 0);
    const totalMemory = this.configService.getDockerConfig().gpu_memory_total || 8192;
    
    return {
      total_memory_mb: totalMemory,
      allocated_memory_mb: totalAllocated,
      available_memory_mb: totalMemory - totalAllocated,
      allocations: allocations.map(alloc => ({
        model_id: alloc.modelId,
        memory_mb: alloc.memoryMb,
        allocated_at: alloc.allocatedAt,
        last_activity: alloc.lastActivity
      }))
    };
  }

  private getLifecycleSummary(): Record<string, any> {
    const containers = Array.from(this.activeContainers.values());
    const runningContainers = containers.filter(c => c.status === ContainerStatus.RUNNING);
    
    return {
      total_containers: containers.length,
      running_containers: runningContainers.length,
      total_gpu_memory_mb: containers.reduce((sum, c) => sum + c.gpuMemoryMb, 0),
      inactivity_timeout: this.configService.getDockerConfig().inactivity_timeout || 300,
      containers: containers.reduce((acc, container) => {
        acc[container.modelId] = {
          container_id: container.containerId,
          status: container.status,
          created_at: container.createdAt,
          started_at: container.startedAt,
          last_activity: container.lastActivity,
          uptime: container.startedAt ? Date.now() - container.startedAt : 0,
          gpu_memory_mb: container.gpuMemoryMb,
          health_check_count: container.healthCheckCount,
          error_count: container.errorCount
        };
        return acc;
      }, {} as Record<string, any>)
    };
  }

  private startCleanupInterval(): void {
    const inactivityTimeout = (this.configService.getDockerConfig().inactivity_timeout || 300) * 1000;
    
    this.cleanupInterval = setInterval(() => {
      this.cleanupInactiveContainers(inactivityTimeout);
    }, 60000); // Check every minute
  }

  private async cleanupInactiveContainers(inactivityTimeout: number): Promise<void> {
    const now = Date.now();
    const inactiveContainers: string[] = [];
    
    for (const [modelId, container] of this.activeContainers) {
      if (now - container.lastActivity > inactivityTimeout) {
        inactiveContainers.push(modelId);
      }
    }
    
    for (const modelId of inactiveContainers) {
      logger.info(`Stopping inactive container for model ${modelId}`);
      await this.stopContainer(modelId);
    }
  }

  public async cleanup(): Promise<void> {
    logger.info('Cleaning up Docker service...');
    
    // Stop cleanup interval
    if (this.cleanupInterval) {
      clearInterval(this.cleanupInterval);
    }
    
    // Stop all active containers
    const modelIds = Array.from(this.activeContainers.keys());
    for (const modelId of modelIds) {
      await this.stopContainer(modelId);
    }
    
    // Clear GPU allocations
    this.gpuAllocations.clear();
    
    this.isInitialized = false;
    logger.info('Docker service cleanup completed');
  }
}

// Singleton instance
let dockerServiceInstance: DockerService | null = null;

export const getDockerService = (): DockerService => {
  if (!dockerServiceInstance) {
    dockerServiceInstance = new DockerService();
  }
  return dockerServiceInstance;
};

export const setupDockerService = (io: any): DockerService => {
  const dockerService = getDockerService();
  
  // Forward Docker service events to Socket.IO
  dockerService.on('initialized', () => {
    io.emit('docker:initialized');
  });
  
  dockerService.on('containerStarted', (data) => {
    io.emit('docker:containerStarted', data);
  });
  
  dockerService.on('containerStopped', (data) => {
    io.emit('docker:containerStopped', data);
  });
  
  // Initialize the service
  dockerService.initialize().catch(error => {
    logger.error('Failed to initialize Docker service:', error);
  });
  
  logger.info('Docker service setup completed');
  return dockerService;
};