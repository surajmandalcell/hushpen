import React, { forwardRef, HTMLAttributes } from 'react';
import { cn } from '../../utils/cn';

export interface PalCardProps extends HTMLAttributes<HTMLDivElement> {
  variant?: 'default' | 'hover' | 'glow';
  withCornerMarkers?: boolean;
  withGeometricBorder?: boolean;
  padding?: 'none' | 'sm' | 'md' | 'lg';
}

const PalCard = forwardRef<HTMLDivElement, PalCardProps>(
  ({ 
    className, 
    variant = 'default', 
    withCornerMarkers = false,
    withGeometricBorder = false,
    padding = 'md',
    children, 
    ...props 
  }, ref) => {
    const baseClasses = 'pal-card font-inter';
    
    const variantClasses = {
      default: '',
      hover: 'pal-card-hover',
      glow: 'pal-card-glow',
    };
    
    const paddingClasses = {
      none: '',
      sm: 'p-3',
      md: 'p-4',
      lg: 'p-6',
    };
    
    const cornerMarkerClasses = withCornerMarkers ? 'pal-corner-markers' : '';
    const geometricBorderClasses = withGeometricBorder ? 'pal-geometric-border' : '';
    
    const classes = cn(
      baseClasses,
      variantClasses[variant],
      paddingClasses[padding],
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

PalCard.displayName = 'PalCard';

export default PalCard;