import { forwardRef, type HTMLAttributes } from 'react';
import { cn } from '../../utils/cn';
import PalText from './PalText';

export interface PalHeaderProps extends HTMLAttributes<HTMLDivElement> {
  height?: 'sm' | 'md' | 'lg';
  withWindowControls?: boolean;
  withGlow?: boolean;
  withCornerMarkers?: boolean;
}

const PalHeader = forwardRef<HTMLDivElement, PalHeaderProps>(
  ({
    className,
    height = 'md',
    withWindowControls = false,
    withGlow = false,
    withCornerMarkers = false,
    children,
    ...props
  }, ref) => {
    const baseClasses = 'flex items-center transition-all duration-200 bg-zinc-900/50 border-b border-zinc-800/60 relative';
    
    const heightClasses = {
      sm: 'h-12',
      md: 'h-16',
      lg: 'h-20',
    };
    
    const glowClasses = withGlow ? 'shadow-[0_0_20px_rgba(59,130,246,0.3)]' : '';
    const cornerMarkerClasses = withCornerMarkers ? 'relative overflow-visible' : '';
    
    const classes = cn(
      baseClasses,
      heightClasses[height],
      glowClasses,
      cornerMarkerClasses,
      className
    );
    
    return (
      <div
        className={classes}
        ref={ref}
        {...props}
      >
        {withCornerMarkers && (
          <>
            <div className="absolute top-0 left-0 w-2 h-2 border-t border-l border-zinc-600" />
            <div className="absolute top-0 right-0 w-2 h-2 border-t border-r border-zinc-600" />
            <div className="absolute bottom-0 left-0 w-2 h-2 border-b border-l border-zinc-600" />
            <div className="absolute bottom-0 right-0 w-2 h-2 border-b border-r border-zinc-600" />
          </>
        )}
        {children}
      </div>
    );
  }
);

PalHeader.displayName = 'PalHeader';

export interface PalWindowControlsProps extends HTMLAttributes<HTMLDivElement> {}

export const PalWindowControls = forwardRef<HTMLDivElement, PalWindowControlsProps>(
  ({ className, ...props }, ref) => {
    const baseClasses = 'flex items-center gap-2 ml-4';
    
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
        <PalWindowControl variant="close" />
        <PalWindowControl variant="minimize" />
        <PalWindowControl variant="maximize" />
      </div>
    );
  }
);

PalWindowControls.displayName = 'PalWindowControls';

export interface PalWindowControlProps extends HTMLAttributes<HTMLButtonElement> {
  variant: 'close' | 'minimize' | 'maximize';
}

export const PalWindowControl = forwardRef<HTMLButtonElement, PalWindowControlProps>(
  ({ className, variant, ...props }, ref) => {
    const baseClasses = 'w-3 h-3 rounded-full transition-all duration-200 hover:scale-110';
    
    const variantClasses = {
      close: 'bg-red-500 hover:bg-red-600',
      minimize: 'bg-orange-500 hover:bg-orange-600',
      maximize: 'bg-green-500 hover:bg-green-600',
    };
    
    const handleClick = async () => {
      console.log(`Window control clicked: ${variant}`);
      try {
        console.log(`Importing Tauri window API...`);
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        console.log(`Getting current window...`);
        const window = getCurrentWindow();
        console.log(`Current window obtained:`, window);
        
        switch (variant) {
          case 'close':
            console.log(`Attempting to close window...`);
            await window.close();
            console.log(`Window close command sent`);
            break;
          case 'minimize':
            console.log(`Attempting to minimize window...`);
            await window.minimize();
            console.log(`Window minimize command sent`);
            break;
          case 'maximize': {
            console.log(`Checking if window is maximized...`);
            const isMaximized = await window.isMaximized();
            console.log(`Window maximized state:`, isMaximized);
            if (isMaximized) {
              console.log(`Attempting to unmaximize window...`);
              await window.unmaximize();
              console.log(`Window unmaximize command sent`);
            } else {
              console.log(`Attempting to maximize window...`);
              await window.maximize();
              console.log(`Window maximize command sent`);
            }
            break;
          }
        }
      } catch (error) {
        console.error(`Failed to ${variant} window:`, error);
        console.error(`Error details:`, JSON.stringify(error, null, 2));
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
      />
    );
  }
);

PalWindowControl.displayName = 'PalWindowControl';

export interface PalHeaderTitleProps extends HTMLAttributes<HTMLDivElement> {
  title?: string;
  subtitle?: string;
}

export const PalHeaderTitle = forwardRef<HTMLDivElement, PalHeaderTitleProps>(
  ({ className, title, subtitle, children, ...props }, ref) => {
    const baseClasses = 'flex flex-col';
    
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
        {title && (
          <PalText size="lg" weight="semibold">
            {title}
          </PalText>
        )}
        {subtitle && (
          <PalText size="sm" variant="muted">
            {subtitle}
          </PalText>
        )}
        {children}
      </div>
    );
  }
);

PalHeaderTitle.displayName = 'PalHeaderTitle';

export interface PalHeaderActionsProps extends HTMLAttributes<HTMLDivElement> {}

export const PalHeaderActions = forwardRef<HTMLDivElement, PalHeaderActionsProps>(
  ({ className, children, ...props }, ref) => {
    const baseClasses = 'flex items-center gap-2 ml-auto';
    
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
        {children}
      </div>
    );
  }
);

PalHeaderActions.displayName = 'PalHeaderActions';

export default PalHeader;
