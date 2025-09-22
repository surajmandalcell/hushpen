// Toast notification service for ASR Pro
export type ToastType = 'info' | 'success' | 'warning' | 'error';

export interface Toast {
  id: string;
  type: ToastType;
  title: string;
  message?: string;
  duration?: number; // in milliseconds
  persistent?: boolean; // won't auto-dismiss
  actions?: ToastAction[];
}

export interface ToastAction {
  label: string;
  onClick: () => void;
  variant?: 'primary' | 'secondary' | 'destructive';
}

class ToastService {
  private toasts: Toast[] = [];
  private listeners: ((toasts: Toast[]) => void)[] = [];

  /**
   * Subscribe to toast changes
   */
  subscribe(listener: (toasts: Toast[]) => void): () => void {
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
    this.listeners.forEach(listener => listener([...this.toasts]));
  }

  /**
   * Show an info toast
   */
  info(title: string, message?: string, options?: Partial<Toast>): string {
    return this.show({ ...options, type: 'info', title, message });
  }

  /**
   * Show a success toast
   */
  success(title: string, message?: string, options?: Partial<Toast>): string {
    return this.show({ ...options, type: 'success', title, message });
  }

  /**
   * Show a warning toast
   */
  warning(title: string, message?: string, options?: Partial<Toast>): string {
    return this.show({ ...options, type: 'warning', title, message });
  }

  /**
   * Show an error toast
   */
  error(title: string, message?: string, options?: Partial<Toast>): string {
    return this.show({ ...options, type: 'error', title, message });
  }

  /**
   * Show a toast with custom options
   */
  show(options: Omit<Toast, 'id'> & { id?: string }): string {
    const id = options.id || Math.random().toString(36).substr(2, 9);
    const { type, ...restOptions } = options;
    const toast: Toast = {
      id,
      type: type || 'info',
      duration: 5000,
      persistent: false,
      ...restOptions,
    };

    this.toasts.push(toast);
    this.notifyListeners();

    // Auto-dismiss if not persistent
    if (!toast.persistent && toast.duration && toast.duration > 0) {
      setTimeout(() => {
        this.dismiss(id);
      }, toast.duration);
    }

    return id;
  }

  /**
   * Dismiss a toast by ID
   */
  dismiss(id: string): void {
    const index = this.toasts.findIndex(toast => toast.id === id);
    if (index > -1) {
      this.toasts.splice(index, 1);
      this.notifyListeners();
    }
  }

  /**
   * Dismiss all toasts
   */
  dismissAll(): void {
    this.toasts = [];
    this.notifyListeners();
  }

  /**
   * Get all current toasts
   */
  getToasts(): Toast[] {
    return [...this.toasts];
  }
}

// Export singleton instance
export const toastService = new ToastService();

// Export hook for React components
export const useToast = () => {
  const [toasts, setToasts] = React.useState<Toast[]>([]);

  React.useEffect(() => {
    const unsubscribe = toastService.subscribe(setToasts);
    return unsubscribe;
  }, []);

  return {
    toasts,
    show: toastService.show.bind(toastService),
    dismiss: toastService.dismiss.bind(toastService),
    dismissAll: toastService.dismissAll.bind(toastService),
    info: toastService.info.bind(toastService),
    success: toastService.success.bind(toastService),
    warning: toastService.warning.bind(toastService),
    error: toastService.error.bind(toastService),
  };
};

// Import React for the hook
import React from 'react';
