import Store from 'electron-store';
import path from 'path';
import os from 'os';
import { logger } from '../utils/logger';
import { ConfigError } from '../utils/errors';

export interface ServerConfig {
  host: string;
  port: number;
}

export interface DockerConfig {
  integration_pending: boolean;
  inactivity_timeout?: number;
  container_timeout?: number;
  gpu_memory_total?: number;
}

export interface AppConfig {
  server: ServerConfig;
  docker: DockerConfig;
}

class ConfigService {
  private store: Store<AppConfig>;
  private configPath: string;

  constructor() {
    this.configPath = this.getConfigPath();
    this.store = new Store<AppConfig>({
      name: 'asrpro-config',
      cwd: path.dirname(this.configPath)
    });
    
    this.initializeDefaultConfig();
    logger.info(`Configuration service initialized with path: ${this.configPath}`);
  }

  private getConfigPath(): string {
    const system = os.type();
    let configDir: string;

    if (system === 'Darwin') {
      // macOS
      configDir = path.join(os.homedir(), 'Library', 'Application Support', 'asrpro');
    } else if (system === 'Windows_NT') {
      // Windows
      configDir = path.join(process.env.APPDATA || '', 'asrpro');
    } else {
      // Linux and others
      configDir = path.join(os.homedir(), '.config', 'asrpro');
    }

    return path.join(configDir, 'config.json');
  }

  private initializeDefaultConfig(): void {
    const defaultConfig: AppConfig = {
      server: {
        host: '127.0.0.1',
        port: 8000
      },
      docker: {
        integration_pending: true,
        inactivity_timeout: 300, // 5 minutes
        container_timeout: 60,   // 1 minute
        gpu_memory_total: 8192   // 8GB
      }
    };

    // Initialize with defaults if not exist
    if (!this.store.has('server')) {
      this.store.set('server', defaultConfig.server);
    }
    if (!this.store.has('docker')) {
      this.store.set('docker', defaultConfig.docker);
    }

    logger.info('Default configuration initialized');
  }

  public getServerConfig(): ServerConfig {
    try {
      return this.store.get('server') as ServerConfig;
    } catch (error) {
      logger.error('Failed to get server config:', error);
      throw new ConfigError('Failed to retrieve server configuration');
    }
  }

  public getDockerConfig(): DockerConfig {
    try {
      return this.store.get('docker') as DockerConfig;
    } catch (error) {
      logger.error('Failed to get docker config:', error);
      throw new ConfigError('Failed to retrieve Docker configuration');
    }
  }

  public getConfig(key: string): any {
    try {
      const keys = key.split('.');
      let value: any = this.store.store;

      for (const k of keys) {
        if (value && typeof value === 'object' && k in value) {
          value = value[k];
        } else {
          return null;
        }
      }

      return value;
    } catch (error) {
      logger.error(`Failed to get config key '${key}':`, error);
      throw new ConfigError(`Failed to retrieve configuration key: ${key}`);
    }
  }

  public setConfig(key: string, value: any): void {
    try {
      this.store.set(key as any, value);
      logger.info(`Configuration updated: ${key}`);
    } catch (error) {
      logger.error(`Failed to set config key '${key}':`, error);
      throw new ConfigError(`Failed to update configuration key: ${key}`);
    }
  }

  public updateConfig(updates: Partial<AppConfig>): void {
    try {
      if (updates.server) {
        const currentServer = this.getServerConfig();
        this.store.set('server', { ...currentServer, ...updates.server });
      }
      if (updates.docker) {
        const currentDocker = this.getDockerConfig();
        this.store.set('docker', { ...currentDocker, ...updates.docker });
      }
      logger.info('Configuration updated successfully');
    } catch (error) {
      logger.error('Failed to update configuration:', error);
      throw new ConfigError('Failed to update configuration');
    }
  }

  public resetToDefaults(): void {
    try {
      this.store.clear();
      this.initializeDefaultConfig();
      logger.info('Configuration reset to defaults');
    } catch (error) {
      logger.error('Failed to reset configuration:', error);
      throw new ConfigError('Failed to reset configuration');
    }
  }

  public exportConfig(): string {
    try {
      const config = this.store.store;
      return JSON.stringify(config, null, 2);
    } catch (error) {
      logger.error('Failed to export configuration:', error);
      throw new ConfigError('Failed to export configuration');
    }
  }

  public importConfig(configJson: string): void {
    try {
      const config = JSON.parse(configJson);
      this.store.clear();
      this.store.store = config;
      logger.info('Configuration imported successfully');
    } catch (error) {
      logger.error('Failed to import configuration:', error);
      throw new ConfigError('Failed to import configuration. Invalid JSON format.');
    }
  }
}

// Singleton instance
let configServiceInstance: ConfigService | null = null;

export const getConfigService = (): ConfigService => {
  if (!configServiceInstance) {
    configServiceInstance = new ConfigService();
  }
  return configServiceInstance;
};

export const setupConfigService = (): ConfigService => {
  const configService = getConfigService();
  logger.info('Configuration service setup completed');
  return configService;
};