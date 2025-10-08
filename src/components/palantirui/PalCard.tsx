import { forwardRef, type HTMLAttributes, type ReactNode } from 'react';
import { cn } from '../../utils/cn';

export interface PalCardProps extends HTMLAttributes<HTMLDivElement> {
  variant?: 'default' | 'bordered' | 'glass' | 'elevated' | 'hover';
  withCornerMarkers?: boolean;
  withGeometricBorder?: boolean;
  withGlow?: boolean;
  padding?: 'none' | 'sm' | 'md' | 'lg';
  hover?: boolean;
  glow?: boolean;
  corners?: boolean;
  animate?: boolean;
  animationDelay?: number;
  children?: ReactNode;
}

const PalCard = forwardRef<HTMLDivElement, PalCardProps>(
  ({
    className,
    variant = 'default',
    withCornerMarkers = false,
    withGeometricBorder = false,
    withGlow = false,
    padding = 'md',
    hover = false,
    glow = false,
    corners = false,
    animate = false,
    animationDelay = 0,
    children,
    ...props
  }, ref) => {
    const baseStyles = "relative transition-all duration-300 font-inter";

    const variantStyles = {
      default: "bg-pal-surface border border-pal-border-primary",
      bordered: "bg-transparent border-dashed border-pal-border-tertiary",
      glass: "bg-pal-layer-1 backdrop-blur-sm border border-pal-layer-2",
      elevated: "bg-pal-surface-elevated border border-pal-border-primary shadow-lg",
      hover: "bg-pal-surface border border-pal-border-primary hover:shadow-lg hover:-translate-y-0.5",
    };

    const paddingClasses = {
      none: '',
      sm: 'p-3',
      md: 'p-4',
      lg: 'p-6',
    };

    const hoverStyles = hover
      ? "hover:shadow-lg hover:-translate-y-0.5"
      : "";

    const glowStyles = glow ? "hover:shadow-[0_0_20px_rgba(59,130,246,0.3)]" : "";
    const cornerMarkerStyles = withCornerMarkers || corners ? "relative overflow-visible" : "";
    const geometricBorderStyles = withGeometricBorder ? "relative" : "";
    
    const animationStyles = animate ? "animate-fade-in" : "";
    const delayClass =
      animate && animationDelay > 0 ? `animation-delay-${animationDelay}` : "";
    
    const classes = cn(
      baseStyles,
      variantStyles[variant],
      paddingClasses[padding],
      hoverStyles,
      glowStyles,
      cornerMarkerStyles,
      geometricBorderStyles,
      animationStyles,
      delayClass,
      className
    );
    
    return (
      <div
        className={classes}
        ref={ref}
        {...props}
      >
        {(withCornerMarkers || corners) && (
          <>
            {/* Soft white border */}
            <div className="absolute inset-0 border border-white/10 pointer-events-none z-5" />

            {/* Corner Plus Markers - positioned outside padding */}
            <div className="absolute inset-0 pointer-events-none !mt-0">
              <div className="absolute top-0 left-0 w-2 h-2 border-t border-l border-zinc-600" />
              <div className="absolute top-0 right-0 w-2 h-2 border-t border-r border-zinc-600" />
              <div className="absolute bottom-0 left-0 w-2 h-2 border-b border-l border-zinc-600" />
              <div className="absolute bottom-0 right-0 w-2 h-2 border-b border-r border-zinc-600" />
            </div>
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

PalCard.displayName = 'PalCard';

export default PalCard;
