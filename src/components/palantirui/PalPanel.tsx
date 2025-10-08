import { forwardRef, type HTMLAttributes } from 'react';
import { cn } from '../../utils/cn';
import PalText from './PalText';

export interface PalPanelProps extends HTMLAttributes<HTMLDivElement> {
  variant?: 'default' | 'elevated' | 'bordered' | 'glass';
  padding?: 'none' | 'sm' | 'md' | 'lg';
  withGlow?: boolean;
  withCornerMarkers?: boolean;
  withGeometricBorder?: boolean;
}

const PalPanel = forwardRef<HTMLDivElement, PalPanelProps>(
  ({
    className,
    variant = 'default',
    padding = 'md',
    withGlow = false,
    withCornerMarkers = false,
    withGeometricBorder = false,
    children,
    ...props
  }, ref) => {
    const baseClasses = 'transition-all duration-200 relative';
    
    const variantClasses = {
      default: 'bg-pal-surface border border-pal-border-primary',
      elevated: 'bg-pal-surface-elevated border border-pal-border-primary shadow-lg',
      bordered: 'bg-pal-bg-secondary border-2 border-dashed border-pal-border-tertiary',
      glass: 'bg-pal-layer-1 backdrop-blur-sm border border-pal-layer-2',
    };
    
    const paddingClasses = {
      none: '',
      sm: 'p-3',
      md: 'p-4',
      lg: 'p-6',
    };
    
    const glowClasses = withGlow ? 'hover:shadow-[0_0_20px_rgba(59,130,246,0.3)]' : '';
    const cornerMarkerClasses = withCornerMarkers ? 'relative overflow-visible' : '';
    const geometricBorderClasses = withGeometricBorder ? 'relative' : '';
    
    const classes = cn(
      baseClasses,
      variantClasses[variant],
      paddingClasses[padding],
      glowClasses,
      cornerMarkerClasses,
      geometricBorderClasses,
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
            <div className="absolute top-0 left-0 w-2 h-2 border-t border-l border-pal-accent-blue" />
            <div className="absolute top-0 right-0 w-2 h-2 border-t border-r border-pal-accent-blue" />
            <div className="absolute bottom-0 left-0 w-2 h-2 border-b border-l border-pal-accent-blue" />
            <div className="absolute bottom-0 right-0 w-2 h-2 border-b border-r border-pal-accent-blue" />
          </>
        )}
        {withGeometricBorder && (
          <div className="absolute inset-0 border border-zinc-700/30 pointer-events-none" />
        )}
        {children}
      </div>
    );
  }
);

PalPanel.displayName = 'PalPanel';

export interface PalPanelHeaderProps extends HTMLAttributes<HTMLDivElement> {
  title?: string;
  subtitle?: string;
  withBorder?: boolean;
}

export const PalPanelHeader = forwardRef<HTMLDivElement, PalPanelHeaderProps>(
  ({
    className,
    title,
    subtitle,
    withBorder = true,
    children,
    ...props
  }, ref) => {
    const baseClasses = 'flex items-center justify-between';
    const borderClasses = withBorder ? 'border-b border-zinc-800/60 pb-4 mb-4' : 'pb-2 mb-2';
    
    const classes = cn(
      baseClasses,
      borderClasses,
      className
    );
    
    return (
      <div
        className={classes}
        ref={ref}
        {...props}
      >
        <div className="flex flex-col">
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
        </div>
        {children}
      </div>
    );
  }
);

PalPanelHeader.displayName = 'PalPanelHeader';

export interface PalPanelContentProps extends HTMLAttributes<HTMLDivElement> {}

export const PalPanelContent = forwardRef<HTMLDivElement, PalPanelContentProps>(
  ({ className, children, ...props }, ref) => {
    const baseClasses = 'flex-1';
    
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

PalPanelContent.displayName = 'PalPanelContent';

export interface PalPanelFooterProps extends HTMLAttributes<HTMLDivElement> {
  withBorder?: boolean;
}

export const PalPanelFooter = forwardRef<HTMLDivElement, PalPanelFooterProps>(
  ({
    className,
    withBorder = true,
    children,
    ...props
  }, ref) => {
    const baseClasses = 'flex items-center justify-between';
    const borderClasses = withBorder ? 'border-t border-zinc-800/60 pt-4 mt-4' : 'pt-2 mt-2';
    
    const classes = cn(
      baseClasses,
      borderClasses,
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

PalPanelFooter.displayName = 'PalPanelFooter';

export default PalPanel;
