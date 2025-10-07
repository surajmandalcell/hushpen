import { forwardRef, type HTMLAttributes } from 'react';
import { cn } from '../utils/cn';

export interface TacticalToggleProps extends Omit<HTMLAttributes<HTMLInputElement>, 'type' | 'checked' | 'onChange'> {
  checked?: boolean;
  onChange?: (checked: boolean) => void;
  size?: 'sm' | 'md' | 'lg';
  disabled?: boolean;
}

export const TacticalToggle = forwardRef<HTMLInputElement, TacticalToggleProps>(
  ({ 
    className, 
    checked = false, 
    onChange, 
    size = 'md',
    disabled = false,
    ...props 
  }, ref) => {
    const baseClasses = 'relative inline-flex items-center justify-center flex-shrink-0 cursor-pointer transition-all duration-200 focus:outline-none focus:ring-2 focus:ring-palantir-accent-blue focus:ring-offset-2 focus:ring-offset-palantir-zinc-50 dark:focus:ring-offset-palantir-zinc-900';
    
    const sizeClasses = {
      sm: 'w-8 h-4',
      md: 'w-10 h-5',
      lg: 'w-12 h-6',
    };
    
    const thumbSizeClasses = {
      sm: 'w-3 h-3',
      md: 'w-4 h-4',
      lg: 'w-5 h-5',
    };
    
    const thumbTranslateClasses = {
      sm: checked ? 'translate-x-4' : 'translate-x-0.5',
      md: checked ? 'translate-x-5' : 'translate-x-0.5',
      lg: checked ? 'translate-x-6' : 'translate-x-0.5',
    };
    
    const trackClasses = cn(
      'rounded-full border transition-colors duration-200',
      checked 
        ? 'bg-palantir-accent-blue border-palantir-accent-blue' 
        : 'bg-palantir-zinc-300 dark:bg-palantir-zinc-600 border-palantir-zinc-400 dark:border-palantir-zinc-500',
      disabled && 'opacity-50 cursor-not-allowed'
    );
    
    const thumbClasses = cn(
      'inline-block rounded-full bg-white shadow-sm transition-transform duration-200',
      thumbSizeClasses[size],
      thumbTranslateClasses[size],
      disabled && 'bg-palantir-zinc-400'
    );
    
    const handleChange = (event: React.ChangeEvent<HTMLInputElement>) => {
      if (disabled) return;
      onChange?.(event.target.checked);
    };
    
    return (
      <label className={cn(baseClasses, sizeClasses[size], className)}>
        <input
          type="checkbox"
          className="sr-only"
          checked={checked}
          onChange={handleChange}
          disabled={disabled}
          ref={ref}
          {...props}
        />
        <span className={trackClasses}>
          <span className={thumbClasses} />
        </span>
      </label>
    );
  }
);

TacticalToggle.displayName = 'TacticalToggle';

export default TacticalToggle;