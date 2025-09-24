import React from "react";

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
  const baseClasses = "font-medium transition-all duration-150 focus:outline-none active:scale-95 disabled:opacity-50 disabled:cursor-not-allowed";

  const variantClasses = {
    primary: "btn-macos",
    secondary: "btn-macos-secondary",
    destructive: "btn-macos-danger"
  };

  const sizeClasses = {
    small: "px-3 py-1.5 text-sm rounded",
    medium: "px-4 py-2 text-sm rounded-macos",
    large: "px-6 py-3 text-base rounded-macos"
  };

  const widthClasses = fullWidth ? "w-full" : "";

  const buttonClasses = [
    baseClasses,
    variantClasses[variant],
    sizeClasses[size],
    widthClasses,
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
      <span className="flex items-center justify-center">{children}</span>
    </button>
  );
};

export default MacButton;
