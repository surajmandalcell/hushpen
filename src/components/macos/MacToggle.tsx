import React from "react";
import "./MacToggle.css";

export interface MacToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  label?: string;
  className?: string;
  size?: "small" | "medium" | "large";
}

const MacToggle: React.FC<MacToggleProps> = ({
  checked,
  onChange,
  disabled = false,
  label,
  className = "",
  size = "medium",
}) => {
  const handleToggle = () => {
    if (!disabled) {
      onChange(!checked);
    }
  };

  const handleKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === " " || event.key === "Enter") {
      event.preventDefault();
      handleToggle();
    }
  };

  const toggleClasses = [
    "mac-toggle",
    `mac-toggle--${size}`,
    checked ? "mac-toggle--checked" : "mac-toggle--unchecked",
    disabled ? "mac-toggle--disabled" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className="mac-toggle-container">
      <div
        className={toggleClasses}
        onClick={handleToggle}
        onKeyDown={handleKeyDown}
        role="switch"
        aria-checked={checked}
        aria-label={label}
        tabIndex={disabled ? -1 : 0}
      >
        <div className="mac-toggle__track">
          <div className="mac-toggle__thumb">
            <div className="mac-toggle__thumb-inner" />
          </div>
        </div>
      </div>
      {label && (
        <label className="mac-toggle-label" onClick={handleToggle}>
          {label}
        </label>
      )}
    </div>
  );
};

export default MacToggle;
