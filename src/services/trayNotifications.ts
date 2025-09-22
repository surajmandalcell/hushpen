// Tray notification service for ASR Pro
export interface TrayNotification {
  id: string;
  type: 'info' | 'success' | 'warning' | 'error';
  title: string;
  message?: string;
  timestamp: number;
  persistent?: boolean;
  actions?: TrayNotificationAction[];
}

export interface TrayNotificationAction {
  label: string;
  onClick: () => void;
}

export type TrayNotificationEvent =
  | 'recording-started'
  | 'recording-stopped'
  | 'transcription-started'
  | 'transcription-completed'
  | 'transcription-error'
  | 'file-processing-started'
  | 'file-processing-completed'
  | 'file-processing-error'
  | 'model-loaded'
  | 'model-unloaded'
  | 'error';

class TrayNotificationManager {
  private notifications: TrayNotification[] = [];
  private maxNotifications = 10;
  private listeners: ((notifications: TrayNotification[]) => void)[] = [];

  /**
   * Subscribe to notification changes
   */
  subscribe(listener: (notifications: TrayNotification[]) => void): () => void {
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
    this.listeners.forEach(listener => listener([...this.notifications]));
  }

  /**
   * Add a notification
   */
  addNotification(
    type: TrayNotification['type'],
    title: string,
    message?: string,
    options?: {
      persistent?: boolean;
      actions?: TrayNotificationAction[];
    }
  ): string {
    const id = Math.random().toString(36).substr(2, 9);
    const notification: TrayNotification = {
      id,
      type,
      title,
      message,
      timestamp: Date.now(),
      persistent: options?.persistent || false,
      actions: options?.actions,
    };

    this.notifications.unshift(notification);

    // Limit the number of stored notifications
    if (this.notifications.length > this.maxNotifications) {
      this.notifications = this.notifications.slice(0, this.maxNotifications);
    }

    this.notifyListeners();

    // Show system tray notification if available
    this.showSystemTrayNotification(notification);

    return id;
  }

  /**
   * Remove a notification
   */
  removeNotification(id: string): void {
    const index = this.notifications.findIndex(n => n.id === id);
    if (index > -1) {
      this.notifications.splice(index, 1);
      this.notifyListeners();
    }
  }

  /**
   * Clear all notifications
   */
  clearNotifications(): void {
    this.notifications = [];
    this.notifyListeners();
  }

  /**
   * Get all notifications
   */
  getNotifications(): TrayNotification[] {
    return [...this.notifications];
  }

  /**
   * Show system tray notification (Tauri API)
   */
  private async showSystemTrayNotification(notification: TrayNotification): Promise<void> {
    try {
      // Use Tauri's notification API if available
      if (typeof window !== 'undefined' && (window as any).__TAURI__) {
        const { invoke } = await import('@tauri-apps/api/core');

        await invoke('show_tray_notification', {
          title: notification.title,
          message: notification.message || '',
          type: notification.type,
        });
      }
    } catch (error) {
      console.error('Failed to show tray notification:', error);
    }
  }

  /**
   * Handle system events and create appropriate notifications
   */
  handleEvent(event: TrayNotificationEvent, data?: any): string {
    switch (event) {
      case 'recording-started':
        return this.addNotification(
          'info',
          'Recording Started',
          'Audio recording has begun',
          { persistent: false }
        );

      case 'recording-stopped':
        return this.addNotification(
          'success',
          'Recording Stopped',
          'Audio recording completed successfully',
          { persistent: false }
        );

      case 'transcription-started':
        return this.addNotification(
          'info',
          'Transcription Started',
          data?.fileName ? `Processing ${data.fileName}...` : 'Processing audio...',
          { persistent: false }
        );

      case 'transcription-completed':
        return this.addNotification(
          'success',
          'Transcription Completed',
          data?.fileName ? `Successfully transcribed ${data.fileName}` : 'Transcription completed successfully',
          { persistent: false }
        );

      case 'transcription-error':
        return this.addNotification(
          'error',
          'Transcription Failed',
          data?.error || 'An error occurred during transcription',
          { persistent: true }
        );

      case 'file-processing-started':
        return this.addNotification(
          'info',
          'File Processing Started',
          `Started processing ${data?.files?.length || 1} file(s)`,
          { persistent: false }
        );

      case 'file-processing-completed':
        return this.addNotification(
          'success',
          'File Processing Completed',
          `Successfully processed ${data?.completedCount || 1} file(s)`,
          { persistent: false }
        );

      case 'file-processing-error':
        return this.addNotification(
          'error',
          'File Processing Failed',
          `Failed to process ${data?.failedFiles?.join(', ') || 'some files'}`,
          { persistent: true }
        );

      case 'model-loaded':
        return this.addNotification(
          'success',
          'Model Loaded',
          `Successfully loaded ${data?.modelName || 'AI model'}`,
          { persistent: false }
        );

      case 'model-unloaded':
        return this.addNotification(
          'info',
          'Model Unloaded',
          `Unloaded ${data?.modelName || 'AI model'} to free memory`,
          { persistent: false }
        );

      case 'error':
        return this.addNotification(
          'error',
          'System Error',
          data?.message || 'An unexpected error occurred',
          { persistent: true }
        );

      default:
        return this.addNotification(
          'info',
          'System Event',
          `Event: ${event}`,
          { persistent: false }
        );
    }
  }

  /**
   * Get notifications by type
   */
  getNotificationsByType(type: TrayNotification['type']): TrayNotification[] {
    return this.notifications.filter(n => n.type === type);
  }

  /**
   * Get unread notification count
   */
  getUnreadCount(): number {
    return this.notifications.filter(n => !n.persistent).length;
  }

  /**
   * Mark notification as read (remove if not persistent)
   */
  markAsRead(id: string): void {
    const notification = this.notifications.find(n => n.id === id);
    if (notification && !notification.persistent) {
      this.removeNotification(id);
    }
  }
}

// Export singleton instance
export const trayNotificationManager = new TrayNotificationManager();

// Export hook for React components
export const useTrayNotifications = () => {
  const [notifications, setNotifications] = React.useState<TrayNotification[]>([]);

  React.useEffect(() => {
    const unsubscribe = trayNotificationManager.subscribe(setNotifications);
    return unsubscribe;
  }, []);

  return {
    notifications,
    addNotification: trayNotificationManager.addNotification.bind(trayNotificationManager),
    removeNotification: trayNotificationManager.removeNotification.bind(trayNotificationManager),
    clearNotifications: trayNotificationManager.clearNotifications.bind(trayNotificationManager),
    handleEvent: trayNotificationManager.handleEvent.bind(trayNotificationManager),
    getNotificationsByType: trayNotificationManager.getNotificationsByType.bind(trayNotificationManager),
    getUnreadCount: trayNotificationManager.getUnreadCount.bind(trayNotificationManager),
    markAsRead: trayNotificationManager.markAsRead.bind(trayNotificationManager),
  };
};

// Import React for the hook
import React from 'react';
