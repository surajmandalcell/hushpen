import { useEffect, useState } from 'react';
import { getCurrentWindow, LogicalSize, LogicalPosition } from '@tauri-apps/api/window';
import { windowManager, WindowState } from '../services/windowManager';

export const useWindowState = () => {
  const [windowState, setWindowState] = useState<WindowState | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const initializeWindowState = async () => {
      try {
        // Get current window state
        const currentState = await getCurrentWindowState();

        // Load saved state or use current state
        const savedState = windowManager.getWindowState();
        const finalState = savedState && windowManager.isValidWindowState(savedState)
          ? savedState
          : currentState;

        // Apply the state to the window
        await applyWindowState(finalState);

        setWindowState(finalState);
      } catch (error) {
        console.error('Failed to initialize window state:', error);
      } finally {
        setIsLoading(false);
      }
    };

    initializeWindowState();

    // Set up window event listeners
    const setupEventListeners = async () => {
      try {
        const window = getCurrentWindow();

        // Listen for window move events
        await window.onMoved(async () => {
          if (!isLoading) {
            const currentState = await getCurrentWindowState();
            windowManager.updateWindowState(
              currentState.x,
              currentState.y,
              currentState.width,
              currentState.height
            );
            setWindowState(currentState);
          }
        });

        // Listen for window resize events
        await window.onResized(async () => {
          if (!isLoading) {
            const currentState = await getCurrentWindowState();
            windowManager.updateWindowState(
              currentState.x,
              currentState.y,
              currentState.width,
              currentState.height
            );
            setWindowState(currentState);
          }
        });

        // Listen for window close events
        await window.onCloseRequested(() => {
          // Save current state before closing
          getCurrentWindowState().then(currentState => {
            windowManager.updateWindowState(
              currentState.x,
              currentState.y,
              currentState.width,
              currentState.height
            );
          });
        });

      } catch (error) {
        console.error('Failed to setup window event listeners:', error);
      }
    };

    setupEventListeners();

    return () => {
      // Cleanup will be handled automatically by Tauri
    };
  }, [isLoading]);

  const getCurrentWindowState = async (): Promise<WindowState> => {
    try {
      const window = getCurrentWindow();
      const position = await window.outerPosition();
      const size = await window.outerSize();
      const maximized = await window.isMaximized();
      const minimized = await window.isMinimized();

      return {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        maximized,
        minimized,
      };
    } catch (error) {
      console.error('Failed to get current window state:', error);
      return windowManager.getDefaultWindowState();
    }
  };

  const applyWindowState = async (state: WindowState): Promise<void> => {
    try {
      const window = getCurrentWindow();

      // Only restore position and size if not maximized
      if (!state.maximized) {
        await window.setSize(new LogicalSize(
          Math.max(state.width, 600),
          Math.max(state.height, 400)
        ));

        await window.setPosition(new LogicalPosition(
          Math.max(state.x, 0),
          Math.max(state.y, 0)
        ));
      }

      // Restore window state
      if (state.maximized) {
        await window.maximize();
      } else if (state.minimized) {
        await window.minimize();
      } else {
        await window.unmaximize();
      }
    } catch (error) {
      console.error('Failed to apply window state:', error);
    }
  };

  const updateWindowPosition = async (x: number, y: number): Promise<void> => {
    try {
      const window = getCurrentWindow();
      await window.setPosition(new LogicalPosition(x, y));
      windowManager.updatePosition(x, y);
      const currentState = await getCurrentWindowState();
      setWindowState(currentState);
    } catch (error) {
      console.error('Failed to update window position:', error);
    }
  };

  const updateWindowSize = async (width: number, height: number): Promise<void> => {
    try {
      const window = getCurrentWindow();
      await window.setSize(new LogicalSize(width, height));
      windowManager.updateSize(width, height);
      const currentState = await getCurrentWindowState();
      setWindowState(currentState);
    } catch (error) {
      console.error('Failed to update window size:', error);
    }
  };

  const maximizeWindow = async (): Promise<void> => {
    try {
      const window = getCurrentWindow();
      await window.maximize();
      windowManager.updateMaximized(true);
      const currentState = await getCurrentWindowState();
      setWindowState(currentState);
    } catch (error) {
      console.error('Failed to maximize window:', error);
    }
  };

  const minimizeWindow = async (): Promise<void> => {
    try {
      const window = getCurrentWindow();
      await window.minimize();
      windowManager.updateMinimized(true);
      const currentState = await getCurrentWindowState();
      setWindowState(currentState);
    } catch (error) {
      console.error('Failed to minimize window:', error);
    }
  };

  const restoreWindow = async (): Promise<void> => {
    try {
      const window = getCurrentWindow();
      await window.unmaximize();
      windowManager.updateMaximized(false);
      windowManager.updateMinimized(false);
      const currentState = await getCurrentWindowState();
      setWindowState(currentState);
    } catch (error) {
      console.error('Failed to restore window:', error);
    }
  };

  const resetWindowState = (): void => {
    windowManager.resetWindowState();
    setWindowState(null);
  };

  return {
    windowState,
    isLoading,
    updateWindowPosition,
    updateWindowSize,
    maximizeWindow,
    minimizeWindow,
    restoreWindow,
    resetWindowState,
  };
};
