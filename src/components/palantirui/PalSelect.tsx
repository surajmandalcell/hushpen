import { forwardRef, type SelectHTMLAttributes } from 'react';
import { cn } from '../../utils/cn';

export interface PalSelectProps extends SelectHTMLAttributes<HTMLSelectElement> {
  label?: string;
  error?: string;
}

const PalSelect = forwardRef<HTMLSelectElement, PalSelectProps>(
  ({ className, label, error, children, ...props }, ref) => {
    const baseClasses = 'px-3 py-2 rounded-pal border transition-all duration-200 focus:outline-none focus:ring-2 focus:ring-palantir-accent-blue focus:border-transparent';

    const colorClasses = error
      ? 'border-palantir-accent-red bg-palantir-zinc-50 dark:bg-palantir-zinc-800'
      : 'border-palantir-zinc-300 dark:border-palantir-zinc-600 bg-palantir-zinc-50 dark:bg-palantir-zinc-800';

    const textClasses = 'text-palantir-zinc-900 dark:text-palantir-zinc-100';

    const classes = cn(
      baseClasses,
      colorClasses,
      textClasses,
      className
    );

    return (
      <div className="flex flex-col">
        {label && (
          <label className="mb-1 text-sm font-medium text-palantir-zinc-700 dark:text-palantir-zinc-300">
            {label}
          </label>
        )}
        <select
          className={classes}
          ref={ref}
          {...props}
        >
          {children}
        </select>
        {error && (
          <span className="mt-1 text-sm text-palantir-accent-red">{error}</span>
        )}
      </div>
    );
  }
);

PalSelect.displayName = 'PalSelect';

export default PalSelect;
