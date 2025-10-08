import { forwardRef, type HTMLAttributes } from 'react';
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
      xs: 'text-xs',
      sm: 'text-sm',
      base: 'text-base',
      lg: 'text-lg',
      xl: 'text-xl',
      '2xl': 'text-2xl',
    };
    
    const weightClasses = {
      normal: 'font-normal',
      medium: 'font-medium',
      semibold: 'font-semibold',
      bold: 'font-bold',
    };
    
    const variantClasses = {
      default: 'text-pal-text-primary',
      muted: 'text-pal-text-tertiary',
      accent: 'text-pal-accent-cyan',
      gradient: 'bg-gradient-to-r from-pal-accent-cyan to-pal-accent-blue bg-clip-text text-transparent',
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
