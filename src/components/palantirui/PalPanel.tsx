import React, { forwardRef, HTMLAttributes } from 'react';
import { cn } from '../../utils/cn';
import PalText from './PalText';

export interface PalPanelProps extends HTMLAttributes<HTMLDivElement> {
  variant?: 'default' | 'elevated' | 'bordered';
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
    const baseClasses = 'pal-panel transition-all duration-200';
    
    const variantClasses = {
      default: '',
      elevated: 'shadow-pal-medium',
      bordered: 'border-2 border-palantir-zinc-300 dark:border-palantir-zinc-600',
    };
    
    const paddingClasses = {
      none: '',
      sm: 'p-3',
      md: 'p-4',
      lg: 'p-6',
    };
    
    const glowClasses = withGlow ? 'pal-glow-subtle' : '';
    const cornerMarkerClasses = withCornerMarkers ? 'pal-corner-markers' : '';
    const geometricBorderClasses = withGeometricBorder ? 'pal-geometric-border' : '';
    
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
    const borderClasses = withBorder ? 'border-b border-palantir-zinc-200 dark:border-palantir-zinc-700 pb-4 mb-4' : 'pb-2 mb-2';
    
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
    const borderClasses = withBorder ? 'border-t border-palantir-zinc-200 dark:border-palantir-zinc-700 pt-4 mt-4' : 'pt-2 mt-2';
    
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