import React, { forwardRef, HTMLAttributes } from 'react';
import { cn } from '../../utils/cn';

export interface PalIconProps extends HTMLAttributes<HTMLSpanElement> {
  size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl';
  variant?: 'default' | 'muted' | 'accent';
  withGlow?: boolean;
  children: React.ReactNode;
}

const PalIcon = forwardRef<HTMLSpanElement, PalIconProps>(
  ({ 
    className, 
    size = 'md', 
    variant = 'default',
    withGlow = false,
    children, 
    ...props 
  }, ref) => {
    const baseClasses = 'inline-flex items-center justify-center transition-all duration-200';
    
    const sizeClasses = {
      xs: 'w-3 h-3 text-xs',
      sm: 'w-4 h-4 text-sm',
      md: 'w-5 h-5 text-base',
      lg: 'w-6 h-6 text-lg',
      xl: 'w-8 h-8 text-xl',
    };
    
    const variantClasses = {
      default: 'text-palantir-zinc-700 dark:text-palantir-zinc-300',
      muted: 'text-palantir-zinc-500 dark:text-palantir-zinc-400',
      accent: 'text-palantir-accent-blue',
    };
    
    const glowClasses = withGlow ? 'pal-glow-on-hover' : '';
    
    const classes = cn(
      baseClasses,
      sizeClasses[size],
      variantClasses[variant],
      glowClasses,
      className
    );
    
    return (
      <span
        className={classes}
        ref={ref}
        {...props}
      >
        {children}
      </span>
    );
  }
);

PalIcon.displayName = 'PalIcon';

export default PalIcon;