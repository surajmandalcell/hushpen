import { forwardRef, type HTMLAttributes } from 'react';
import { cn } from '../utils/cn';

export interface TacticalWindowControlsProps extends HTMLAttributes<HTMLDivElement> {}

export const TacticalWindowControls = forwardRef<HTMLDivElement, TacticalWindowControlsProps>(
  ({ className, ...props }, ref) => {
    const baseClasses = 'flex items-center gap-1';
    
    const classes = cn(
      baseClasses,
      className
    );
    
    return (
      <div
        className={classes}
        ref={ref}
        {...props}
      >
        <TacticalWindowControl variant="close" />
        <TacticalWindowControl variant="minimize" />
        <TacticalWindowControl variant="maximize" />
      </div>
    );
  }
);

TacticalWindowControls.displayName = 'TacticalWindowControls';

export interface TacticalWindowControlProps extends HTMLAttributes<HTMLButtonElement> {
  variant: 'close' | 'minimize' | 'maximize';
}

export const TacticalWindowControl = forwardRef<HTMLButtonElement, TacticalWindowControlProps>(
  ({ className, variant, ...props }, ref) => {
    const baseClasses = 'w-6 h-6 flex items-center justify-center transition-all duration-150 border focus:outline-none focus:ring-1 focus:ring-palantir-accent-blue';

    const variantClasses = {
      close: 'bg-palantir-zinc-200 hover:bg-palantir-zinc-300 text-palantir-zinc-600 hover:text-palantir-accent-red border-palantir-zinc-300 hover:border-palantir-accent-red dark:bg-palantir-zinc-800 dark:hover:bg-palantir-zinc-700 dark:text-palantir-zinc-400 dark:border-palantir-zinc-600 dark:hover:border-palantir-accent-red',
      minimize: 'bg-palantir-zinc-200 hover:bg-palantir-zinc-300 text-palantir-zinc-600 hover:text-palantir-accent-orange border-palantir-zinc-300 hover:border-palantir-accent-orange dark:bg-palantir-zinc-800 dark:hover:bg-palantir-zinc-700 dark:text-palantir-zinc-400 dark:border-palantir-zinc-600 dark:hover:border-palantir-accent-orange',
      maximize: 'bg-palantir-zinc-200 hover:bg-palantir-zinc-300 text-palantir-zinc-600 hover:text-palantir-accent-green border-palantir-zinc-300 hover:border-palantir-accent-green dark:bg-palantir-zinc-800 dark:hover:bg-palantir-zinc-700 dark:text-palantir-zinc-400 dark:border-palantir-zinc-600 dark:hover:border-palantir-accent-green',
    };
    
    const handleClick = async () => {
      console.log(`Tactical window control clicked: ${variant}`);
      try {
        // Use the Tauri API directly
        const { invoke } = await import('@tauri-apps/api/core');
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const window = getCurrentWindow();
        
        switch (variant) {
          case 'close':
            console.log(`Attempting to close window...`);
            // Try multiple approaches
            try {
              await invoke('quit_app');
            } catch (invokeError) {
              console.log('Invoke failed, trying direct window API');
              await window.close();
            }
            break;
          case 'minimize':
            console.log(`Attempting to minimize window...`);
            await window.minimize();
            break;
          case 'maximize': {
            console.log(`Checking if window is maximized...`);
            const isMaximized = await window.isMaximized();
            console.log(`Window maximized state:`, isMaximized);
            if (isMaximized) {
              await window.unmaximize();
            } else {
              await window.maximize();
            }
            break;
          }
        }
      } catch (error) {
        console.error(`Failed to ${variant} window:`, error);
        console.error(`Error details:`, JSON.stringify(error, null, 2));
      }
    };
    
    const getIcon = () => {
      switch (variant) {
        case 'close':
          return (
            <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor" aria-label="Close">
              <title>Close</title>
              <path d="M0 0L10 10M10 0L0 10" stroke="currentColor" strokeWidth="1.5"/>
            </svg>
          );
        case 'minimize':
          return (
            <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor" aria-label="Minimize">
              <title>Minimize</title>
              <path d="M2 5h6" stroke="currentColor" strokeWidth="1.5"/>
            </svg>
          );
        case 'maximize':
          return (
            <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor" aria-label="Maximize">
              <title>Maximize</title>
              <rect x="2" y="2" width="6" height="6" stroke="currentColor" strokeWidth="1.5" fill="none"/>
            </svg>
          );
      }
    };
    
    const classes = cn(
      baseClasses,
      variantClasses[variant],
      className
    );
    
    return (
      <button
        className={classes}
        onClick={handleClick}
        ref={ref}
        aria-label={variant}
        {...props}
      >
        {getIcon()}
      </button>
    );
  }
);

TacticalWindowControl.displayName = 'TacticalWindowControl';

export default TacticalWindowControls;