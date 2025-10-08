import fs from 'fs';
import path from 'path';

export enum LogLevel {
  DEBUG = 0,
  INFO = 1,
  WARN = 2,
  ERROR = 3
}

export interface LoggerConfig {
  level: LogLevel;
  enableConsole: boolean;
  enableFile: boolean;
  logFile?: string;
  maxFileSize?: number;
  maxFiles?: number;
}

class Logger {
  private config: LoggerConfig;
  private logDir: string;

  constructor(config: Partial<LoggerConfig> = {}) {
    this.config = {
      level: LogLevel.INFO,
      enableConsole: true,
      enableFile: false,
      maxFileSize: 10 * 1024 * 1024, // 10MB
      maxFiles: 5,
      ...config
    };

    // Set up log directory
    this.logDir = path.join(process.cwd(), 'logs');
    this.ensureLogDirectory();

    // Set up file logging if enabled
    if (this.config.enableFile) {
      this.config.logFile = path.join(this.logDir, 'asrpro.log');
    }
  }

  private ensureLogDirectory(): void {
    if (!fs.existsSync(this.logDir)) {
      fs.mkdirSync(this.logDir, { recursive: true });
    }
  }

  private shouldLog(level: LogLevel): boolean {
    return level >= this.config.level;
  }

  private formatMessage(level: string, message: string, ...args: any[]): string {
    const timestamp = new Date().toISOString();
    const pid = process.pid;
    const argsStr = args.length > 0 ? ` ${args.map(arg => 
      typeof arg === 'object' ? JSON.stringify(arg) : String(arg)
    ).join(' ')}` : '';
    
    return `[${timestamp}] [${level}] [PID:${pid}] ${message}${argsStr}`;
  }

  private writeLog(level: LogLevel, levelName: string, message: string, ...args: any[]): void {
    if (!this.shouldLog(level)) {
      return;
    }

    const formattedMessage = this.formatMessage(levelName, message, ...args);

    // Console logging
    if (this.config.enableConsole) {
      switch (level) {
        case LogLevel.DEBUG:
          console.debug(formattedMessage);
          break;
        case LogLevel.INFO:
          console.log(formattedMessage);
          break;
        case LogLevel.WARN:
          console.warn(formattedMessage);
          break;
        case LogLevel.ERROR:
          console.error(formattedMessage);
          break;
      }
    }

    // File logging
    if (this.config.enableFile && this.config.logFile) {
      try {
        this.rotateLogFileIfNeeded();
        fs.appendFileSync(this.config.logFile, formattedMessage + '\n');
      } catch (error) {
        console.error('Failed to write to log file:', error);
      }
    }
  }

  private rotateLogFileIfNeeded(): void {
    if (!this.config.logFile || !fs.existsSync(this.config.logFile)) {
      return;
    }

    try {
      const stats = fs.statSync(this.config.logFile);
      if (stats.size > (this.config.maxFileSize || 10 * 1024 * 1024)) {
        // Rotate log files
        const baseName = path.basename(this.config.logFile, '.log');
        const dirName = path.dirname(this.config.logFile);
        
        // Remove oldest log file if we have too many
        const maxFiles = this.config.maxFiles || 5;
        for (let i = maxFiles - 1; i > 0; i--) {
          const oldFile = path.join(dirName, `${baseName}.${i}.log`);
          const newFile = path.join(dirName, `${baseName}.${i + 1}.log`);
          
          if (fs.existsSync(oldFile)) {
            if (i === maxFiles - 1) {
              fs.unlinkSync(oldFile);
            } else {
              fs.renameSync(oldFile, newFile);
            }
          }
        }
        
        // Move current log file to .1.log
        const rotatedFile = path.join(dirName, `${baseName}.1.log`);
        fs.renameSync(this.config.logFile, rotatedFile);
      }
    } catch (error) {
      console.error('Failed to rotate log file:', error);
    }
  }

  public debug(message: string, ...args: any[]): void {
    this.writeLog(LogLevel.DEBUG, 'DEBUG', message, ...args);
  }

  public info(message: string, ...args: any[]): void {
    this.writeLog(LogLevel.INFO, 'INFO', message, ...args);
  }

  public warn(message: string, ...args: any[]): void {
    this.writeLog(LogLevel.WARN, 'WARN', message, ...args);
  }

  public warning(message: string, ...args: any[]): void {
    this.writeLog(LogLevel.WARN, 'WARN', message, ...args);
  }

  public error(message: string, ...args: any[]): void {
    this.writeLog(LogLevel.ERROR, 'ERROR', message, ...args);
  }

  public setLevel(level: LogLevel): void {
    this.config.level = level;
  }

  public setConfig(config: Partial<LoggerConfig>): void {
    this.config = { ...this.config, ...config };
    
    if (this.config.enableFile && !this.config.logFile) {
      this.config.logFile = path.join(this.logDir, 'asrpro.log');
    }
  }

  public getConfig(): LoggerConfig {
    return { ...this.config };
  }
}

// Create default logger instance
const defaultLogger = new Logger({
  level: process.env.NODE_ENV === 'development' ? LogLevel.DEBUG : LogLevel.INFO,
  enableConsole: true,
  enableFile: process.env.NODE_ENV === 'production'
});

// Export the default logger instance and class
export { Logger };
export const logger = defaultLogger;

// Convenience exports for backward compatibility
export const getLogger = (config?: Partial<LoggerConfig>): Logger => {
  return config ? new Logger(config) : defaultLogger;
};