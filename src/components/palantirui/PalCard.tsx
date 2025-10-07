import { forwardRef, type HTMLAttributes } from 'react';
import { cn } from '../../utils/cn';

export interface PalCardProps extends HTMLAttributes<HTMLDivElement> {
  variant?: 'default' | 'hover' | 'glow' | 'elevated';
  withCornerMarkers?: boolean;
  withGeometricBorder?: boolean;
  withGlow?: boolean;
  padding?: 'none' | 'sm' | 'md' | 'lg';
}

const PalCard = forwardRef<HTMLDivElement, PalCardProps>(
  ({
    className,
    variant = 'default',
    withCornerMarkers = false,
    withGeometricBorder = false,
    withGlow = false,
    padding = 'md',
    children,
    ...props
  }, ref) => {
    const baseClasses = 'pal-card font-inter';

    const variantClasses = {
      default: '',
      hover: 'pal-card-hover',
      glow: 'pal-card-glow',
      elevated: 'shadow-pal-medium',
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

PalCard.displayName = 'PalCard';

export default PalCard;
