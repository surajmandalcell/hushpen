// File queue management for ASR Pro transcription
export interface QueuedFile {
  id: string;
  file: File;
  name: string;
  size: number;
  status: 'pending' | 'processing' | 'completed' | 'error';
  progress: number; // 0-100
  result?: string;
  error?: string;
  addedAt: number;
  startedAt?: number;
  completedAt?: number;
}

export interface QueueStats {
  total: number;
  pending: number;
  processing: number;
  completed: number;
  errors: number;
  totalSize: number;
}

export type QueueEvent =
  | 'file-added'
  | 'file-removed'
  | 'processing-started'
  | 'processing-progress'
  | 'processing-completed'
  | 'processing-error'
  | 'queue-cleared';

class FileQueueManager {
  private files: QueuedFile[] = [];
  private listeners: ((files: QueuedFile[], stats: QueueStats) => void)[] = [];
  private processingFile: QueuedFile | null = null;
  // Process one file at a time

  /**
   * Subscribe to queue changes
   */
  subscribe(listener: (files: QueuedFile[], stats: QueueStats) => void): () => void {
    this.listeners.push(listener);
    return () => {
      const index = this.listeners.indexOf(listener);
      if (index > -1) {
        this.listeners.splice(index, 1);
      }
    };
  }

  /**
   * Notify all listeners of changes
   */
  private notifyListeners(): void {
    const stats = this.getStats();
    this.listeners.forEach(listener => listener([...this.files], stats));
  }

  /**
   * Add files to the queue
   */
  addFiles(files: File[]): string[] {
    const ids: string[] = [];

    files.forEach(file => {
      // Check if file already exists
      const existing = this.files.find(f => f.name === file.name && f.size === file.size);
      if (existing) {
        return; // Skip duplicates
      }

      const queuedFile: QueuedFile = {
        id: Math.random().toString(36).substr(2, 9),
        file,
        name: file.name,
        size: file.size,
        status: 'pending',
        progress: 0,
        addedAt: Date.now(),
      };

      this.files.push(queuedFile);
      ids.push(queuedFile.id);
    });

    this.notifyListeners();
    return ids;
  }

  /**
   * Remove a file from the queue
   */
  removeFile(id: string): boolean {
    const index = this.files.findIndex(f => f.id === id);
    if (index === -1) return false;

    const file = this.files[index];

    // Can't remove if currently processing
    if (file.status === 'processing') return false;

    this.files.splice(index, 1);
    this.notifyListeners();
    return true;
  }

  /**
   * Clear the entire queue
   */
  clearQueue(): void {
    this.files = [];
    this.processingFile = null;
    this.notifyListeners();
  }

  /**
   * Start processing the next file in queue
   */
  async processNext(): Promise<void> {
    if (this.processingFile || this.files.length === 0) return;

    const nextFile = this.files.find(f => f.status === 'pending');
    if (!nextFile) return;

    this.processingFile = nextFile;
    nextFile.status = 'processing';
    nextFile.startedAt = Date.now();

    this.notifyListeners();

    try {
      // Simulate processing with progress updates
      await this.processFile(nextFile);
    } catch (error) {
      nextFile.status = 'error';
      nextFile.error = error instanceof Error ? error.message : 'Unknown error';
      this.notifyListeners();
    } finally {
      this.processingFile = null;
      nextFile.completedAt = Date.now();

      // Process next file if available
      setTimeout(() => {
        this.processNext();
      }, 1000);
    }
  }

  /**
   * Process a single file (simulate transcription)
   */
  private async processFile(file: QueuedFile): Promise<void> {
    const steps = ['Loading', 'Converting', 'Transcribing', 'Saving'];
    const stepDuration = 2000; // 2 seconds per step

    for (let i = 0; i < steps.length; i++) {
      const progress = ((i + 1) / steps.length) * 100;

      file.progress = Math.min(progress, 100);
      file.status = 'processing';

      this.notifyListeners();

      await new Promise(resolve => setTimeout(resolve, stepDuration));

      // Allow cancellation - check if status changed to error
      if (file.status !== 'processing') break;
    }

    if (file.status === 'processing') {
      file.status = 'completed';
      file.progress = 100;
      file.result = `Transcription completed for ${file.name}`;

      this.notifyListeners();
    }
  }

  /**
   * Cancel processing of current file
   */
  cancelProcessing(): void {
    if (this.processingFile) {
      this.processingFile.status = 'error';
      this.processingFile.error = 'Cancelled by user';
      this.processingFile = null;
    }
  }

  /**
   * Get queue statistics
   */
  getStats(): QueueStats {
    const stats = {
      total: this.files.length,
      pending: this.files.filter(f => f.status === 'pending').length,
      processing: this.files.filter(f => f.status === 'processing').length,
      completed: this.files.filter(f => f.status === 'completed').length,
      errors: this.files.filter(f => f.status === 'error').length,
      totalSize: this.files.reduce((sum, f) => sum + f.size, 0),
    };

    return stats;
  }

  /**
   * Get all queued files
   */
  getFiles(): QueuedFile[] {
    return [...this.files];
  }

  /**
   * Check if queue is empty
   */
  isEmpty(): boolean {
    return this.files.length === 0;
  }

  /**
   * Check if any files are currently processing
   */
  isProcessing(): boolean {
    return this.processingFile !== null;
  }

  /**
   * Get currently processing file
   */
  getCurrentFile(): QueuedFile | null {
    return this.processingFile;
  }
}

// Export singleton instance
export const fileQueueManager = new FileQueueManager();

// Export hook for React components
export const useFileQueue = () => {
  const [files, setFiles] = React.useState<QueuedFile[]>([]);
  const [stats, setStats] = React.useState<QueueStats>({
    total: 0,
    pending: 0,
    processing: 0,
    completed: 0,
    errors: 0,
    totalSize: 0,
  });

  React.useEffect(() => {
    const unsubscribe = fileQueueManager.subscribe((files, stats) => {
      setFiles(files);
      setStats(stats);
    });
    return unsubscribe;
  }, []);

  return {
    files,
    stats,
    addFiles: fileQueueManager.addFiles.bind(fileQueueManager),
    removeFile: fileQueueManager.removeFile.bind(fileQueueManager),
    clearQueue: fileQueueManager.clearQueue.bind(fileQueueManager),
    processNext: fileQueueManager.processNext.bind(fileQueueManager),
    cancelProcessing: fileQueueManager.cancelProcessing.bind(fileQueueManager),
    isEmpty: fileQueueManager.isEmpty.bind(fileQueueManager),
    isProcessing: fileQueueManager.isProcessing.bind(fileQueueManager),
    getCurrentFile: fileQueueManager.getCurrentFile.bind(fileQueueManager),
  };
};

// Import React for the hook
import React from 'react';
