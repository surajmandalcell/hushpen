import React, { forwardRef, InputHTMLAttributes } from 'react';
import { cn } from '../../utils/cn';

export interface PalInputProps extends InputHTMLAttributes<HTMLInputElement> {
  variant?: 'default' | 'ghost';
  size?: 'sm' | 'md' | 'lg';
  withGlow?: boolean;
  withCornerMarkers?: boolean;
  error?: boolean;
  fullWidth?: boolean;
}

const PalInput = forwardRef<HTMLInputElement, PalInputProps>(
  ({ 
    className, 
    variant = 'default', 
    size = 'md', 
    withGlow = false,
    withCornerMarkers = false,
    error = false,
    fullWidth = false,
    disabled,
    ...props 
  }, ref) => {
    const baseClasses = 'pal-input font-inter transition-all duration-200 focus:outline-none focus:ring-2 focus:ring-palantir-accent-blue focus:border-transparent disabled:opacity-50 disabled:cursor-not-allowed';
    
    const variantClasses = {
      default: '',
      ghost: 'bg-transparent border-transparent hover:bg-palantir-layer-2 dark:hover:bg-palantir-dark-layer-2',
    };
    
    const sizeClasses = {
      sm: 'px-3 py-1.5 text-sm',
      md: 'px-3 py-2 text-base',
      lg: 'px-4 py-3 text-lg',
    };
    
    const stateClasses = error 
      ? 'border-palantir-accent-red focus:ring-palantir-accent-red' 
      : '';
    
    const glowClasses = withGlow ? 'pal-glow-on-hover' : '';
    const widthClasses = fullWidth ? 'w-full' : '';
    const cornerMarkerClasses = withCornerMarkers ? 'pal-corner-markers' : '';
    
    const classes = cn(
      baseClasses,
      variantClasses[variant],
      sizeClasses[size],
      stateClasses,
      glowClasses,
      widthClasses,
      cornerMarkerClasses,
      className
    );
    
    return (
      <input
        className={classes}
        ref={ref}
        disabled={disabled}
        {...props}
      />
    );
  }
);

PalInput.displayName = 'PalInput';

export default PalInput;