// Window position and state management for ASR Pro
export interface WindowState {
  x: number;
  y: number;
  width: number;
  height: number;
  maximized: boolean;
  minimized: boolean;
}

const STORAGE_KEY = 'asrpro-window-state';
const DEBOUNCE_DELAY = 500; // milliseconds

class WindowManager {
  private currentState: WindowState | null = null;
  private saveTimeout: ReturnType<typeof setTimeout> | null = null;

  constructor() {
    this.loadWindowState();
  }

  /**
   * Load window state from localStorage
   */
  private loadWindowState(): void {
    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (stored) {
        this.currentState = JSON.parse(stored);
      }
    } catch (error) {
      console.error('Failed to load window state:', error);
      this.currentState = null;
    }
  }

  /**
   * Save window state to localStorage with debouncing
   */
  private saveWindowState(state: WindowState): void {
    // Clear existing timeout
    if (this.saveTimeout !== null) {
      clearTimeout(this.saveTimeout);
    }

    // Debounce the save operation
    this.saveTimeout = setTimeout(() => {
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
        this.currentState = state;
      } catch (error) {
        console.error('Failed to save window state:', error);
      }
    }, DEBOUNCE_DELAY);
  }

  /**
   * Get current window state
   */
  getWindowState(): WindowState | null {
    return this.currentState ? { ...this.currentState } : null;
  }

  /**
   * Update window position
   */
  updatePosition(x: number, y: number): void {
    if (this.currentState) {
      this.currentState.x = x;
      this.currentState.y = y;
      this.saveWindowState(this.currentState);
    }
  }

  /**
   * Update window size
   */
  updateSize(width: number, height: number): void {
    if (this.currentState) {
      this.currentState.width = width;
      this.currentState.height = height;
      this.saveWindowState(this.currentState);
    }
  }

  /**
   * Update window state (position and size)
   */
  updateWindowState(x: number, y: number, width: number, height: number): void {
    const newState: WindowState = {
      x,
      y,
      width,
      height,
      maximized: this.currentState?.maximized || false,
      minimized: this.currentState?.minimized || false,
    };

    this.currentState = newState;
    this.saveWindowState(newState);
  }

  /**
   * Update maximized state
   */
  updateMaximized(maximized: boolean): void {
    if (this.currentState) {
      this.currentState.maximized = maximized;
      this.saveWindowState(this.currentState);
    }
  }

  /**
   * Update minimized state
   */
  updateMinimized(minimized: boolean): void {
    if (this.currentState) {
      this.currentState.minimized = minimized;
      this.saveWindowState(this.currentState);
    }
  }

  /**
   * Reset window state to defaults
   */
  resetWindowState(): void {
    this.currentState = null;
    try {
      localStorage.removeItem(STORAGE_KEY);
    } catch (error) {
      console.error('Failed to reset window state:', error);
    }
  }

  /**
   * Check if window state is valid for restoration
   */
  isValidWindowState(state: WindowState): boolean {
    const minWidth = 600;
    const minHeight = 400;

    return (
      state.width >= minWidth &&
      state.height >= minHeight &&
      state.x >= 0 &&
      state.y >= 0
    );
  }

  /**
   * Get default window state
   */
  getDefaultWindowState(): WindowState {
    return {
      x: 100,
      y: 100,
      width: 800,
      height: 600,
      maximized: false,
      minimized: false,
    };
  }
}

// Export singleton instance
export const windowManager = new WindowManager();
