import React, { forwardRef, ButtonHTMLAttributes } from 'react';
import { cn } from '../../utils/cn';

export interface PalButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost';
  size?: 'sm' | 'md' | 'lg';
  withGlow?: boolean;
  withCornerMarkers?: boolean;
  fullWidth?: boolean;
}

const PalButton = forwardRef<HTMLButtonElement, PalButtonProps>(
  ({ 
    className, 
    variant = 'primary', 
    size = 'md', 
    withGlow = false,
    withCornerMarkers = false,
    fullWidth = false,
    children, 
    disabled,
    ...props 
  }, ref) => {
    const baseClasses = 'pal-btn font-inter font-medium transition-all duration-200 focus:outline-none focus:ring-2 focus:ring-palantir-accent-blue focus:ring-opacity-50 active:scale-95 disabled:opacity-50 disabled:cursor-not-allowed disabled:active:scale-100';
    
    const variantClasses = {
      primary: 'pal-btn-primary',
      secondary: 'pal-btn-secondary',
      ghost: 'pal-btn-ghost',
    };
    
    const sizeClasses = {
      sm: 'px-3 py-1.5 text-sm',
      md: 'px-4 py-2 text-base',
      lg: 'px-6 py-3 text-lg',
    };
    
    const glowClasses = withGlow ? 'pal-glow-on-hover' : '';
    const widthClasses = fullWidth ? 'w-full' : '';
    const cornerMarkerClasses = withCornerMarkers ? 'pal-corner-markers' : '';
    
    const classes = cn(
      baseClasses,
      variantClasses[variant],
      sizeClasses[size],
      glowClasses,
      widthClasses,
      cornerMarkerClasses,
      className
    );
    
    return (
      <button
        className={classes}
        ref={ref}
        disabled={disabled}
        {...props}
      >
        {children}
      </button>
    );
  }
);

PalButton.displayName = 'PalButton';

export default PalButton;