import React, { forwardRef } from "react";

export interface MacInputProps
  extends Omit<React.InputHTMLAttributes<HTMLInputElement>, "size"> {
  label?: string;
  error?: string;
  size?: "small" | "medium" | "large";
  variant?: "default" | "search" | "rounded";
  leftIcon?: React.ReactNode;
  rightIcon?: React.ReactNode;
  fullWidth?: boolean;
}

const MacInput = forwardRef<HTMLInputElement, MacInputProps>(
  (
    {
      label,
      error,
      size = "medium",
      variant = "default",
      leftIcon,
      rightIcon,
      fullWidth = false,
      className = "",
      disabled = false,
      ...props
    },
    ref
  ) => {
    const containerClasses = [
      "space-y-2",
      fullWidth ? "w-full" : "",
      disabled ? "opacity-50" : "",
    ]
      .filter(Boolean)
      .join(" ");

    const sizeClasses = {
      small: "px-2 py-1 text-sm",
      medium: "px-3 py-2 text-sm",
      large: "px-4 py-3 text-base"
    };

    const variantClasses = {
      default: "rounded-macos",
      search: "rounded-full",
      rounded: "rounded-macos-lg"
    };

    const inputWrapperClasses = [
      "relative flex items-center",
      sizeClasses[size],
      variantClasses[variant],
      "bg-white border border-macos-border",
      "dark:bg-gray-800 dark:border-macos-border-dark",
      "focus-within:ring-2 focus-within:ring-macos-blue focus-within:border-transparent",
      "transition-all duration-150",
      error ? "border-macos-red focus-within:ring-macos-red" : "",
      className,
    ]
      .filter(Boolean)
      .join(" ");

    return (
      <div className={containerClasses}>
        {label && (
          <label className="block text-sm font-medium text-macos-text dark:text-macos-text-dark">
            {label}
          </label>
        )}
        <div className={inputWrapperClasses}>
          {leftIcon && (
            <div className="absolute left-3 text-macos-text-secondary">
              {leftIcon}
            </div>
          )}
          <input
            ref={ref}
            className={`flex-1 bg-transparent outline-none text-macos-text dark:text-macos-text-dark placeholder-macos-text-secondary ${
              leftIcon ? "pl-8" : ""
            } ${rightIcon ? "pr-8" : ""}`}
            disabled={disabled}
            {...props}
          />
          {rightIcon && (
            <div className="absolute right-3 text-macos-text-secondary">
              {rightIcon}
            </div>
          )}
        </div>
        {error && (
          <div className="text-sm text-macos-red">{error}</div>
        )}
      </div>
    );
  }
);

MacInput.displayName = "MacInput";

export default MacInput;
