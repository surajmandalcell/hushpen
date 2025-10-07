import React, { forwardRef, HTMLAttributes } from 'react';
import { cn } from '../../utils/cn';

export interface PalTextProps extends HTMLAttributes<HTMLParagraphElement> {
  size?: 'xs' | 'sm' | 'base' | 'lg' | 'xl' | '2xl';
  weight?: 'normal' | 'medium' | 'semibold' | 'bold';
  variant?: 'default' | 'muted' | 'accent' | 'gradient';
  as?: 'p' | 'span' | 'div' | 'h1' | 'h2' | 'h3' | 'h4' | 'h5' | 'h6';
}

const PalText = forwardRef<HTMLParagraphElement, PalTextProps>(
  ({ 
    className, 
    size = 'base', 
    weight = 'normal',
    variant = 'default',
    as: Component = 'p',
    children, 
    ...props 
  }, ref) => {
    const baseClasses = 'font-inter transition-colors duration-200';
    
    const sizeClasses = {
      xs: 'pal-text-xs',
      sm: 'pal-text-sm',
      base: 'pal-text-base',
      lg: 'pal-text-lg',
      xl: 'pal-text-xl',
      '2xl': 'pal-text-2xl',
    };
    
    const weightClasses = {
      normal: 'font-normal',
      medium: 'font-medium',
      semibold: 'font-semibold',
      bold: 'font-bold',
    };
    
    const variantClasses = {
      default: 'text-palantir-zinc-900 dark:text-palantir-zinc-50',
      muted: 'pal-text-muted',
      accent: 'text-palantir-accent-blue',
      gradient: 'text-gradient',
    };
    
    const classes = cn(
      baseClasses,
      sizeClasses[size],
      weightClasses[weight],
      variantClasses[variant],
      className
    );
    
    return (
      <Component
        className={classes}
        ref={ref}
        {...props}
      >
        {children}
      </Component>
    );
  }
);

PalText.displayName = 'PalText';

export default PalText;