import React from "react";
import "./MacButton.css";

export interface MacButtonProps {
  children: React.ReactNode;
  onClick?: () => void;
  variant?: "primary" | "secondary" | "destructive";
  size?: "small" | "medium" | "large";
  disabled?: boolean;
  className?: string;
  type?: "button" | "submit" | "reset";
  fullWidth?: boolean;
}

const MacButton: React.FC<MacButtonProps> = ({
  children,
  onClick,
  variant = "primary",
  size = "medium",
  disabled = false,
  className = "",
  type = "button",
  fullWidth = false,
}) => {
  const buttonClasses = [
    "mac-button",
    `mac-button--${variant}`,
    `mac-button--${size}`,
    fullWidth ? "mac-button--full-width" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <button
      type={type}
      className={buttonClasses}
      onClick={onClick}
      disabled={disabled}
    >
      <span className="mac-button__content">{children}</span>
    </button>
  );
};

export default MacButton;
