import React, { forwardRef } from "react";
import "./MacInput.css";

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
      "mac-input-container",
      fullWidth ? "mac-input-container--full-width" : "",
      disabled ? "mac-input-container--disabled" : "",
    ]
      .filter(Boolean)
      .join(" ");

    const inputWrapperClasses = [
      "mac-input-wrapper",
      `mac-input-wrapper--${size}`,
      `mac-input-wrapper--${variant}`,
      error ? "mac-input-wrapper--error" : "",
      leftIcon ? "mac-input-wrapper--has-left-icon" : "",
      rightIcon ? "mac-input-wrapper--has-right-icon" : "",
      className,
    ]
      .filter(Boolean)
      .join(" ");

    return (
      <div className={containerClasses}>
        {label && <label className="mac-input-label">{label}</label>}
        <div className={inputWrapperClasses}>
          {leftIcon && (
            <div className="mac-input-icon mac-input-icon--left">
              {leftIcon}
            </div>
          )}
          <input
            ref={ref}
            className="mac-input"
            disabled={disabled}
            {...props}
          />
          {rightIcon && (
            <div className="mac-input-icon mac-input-icon--right">
              {rightIcon}
            </div>
          )}
        </div>
        {error && <div className="mac-input-error">{error}</div>}
      </div>
    );
  }
);

MacInput.displayName = "MacInput";

export default MacInput;
