import React from "react";


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
    "pal-toggle",
    `pal-toggle--${size}`,
    checked ? "pal-toggle--checked" : "pal-toggle--unchecked",
    disabled ? "pal-toggle--disabled" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className="pal-toggle-container">
      <div
        className={toggleClasses}
        onClick={handleToggle}
        onKeyDown={handleKeyDown}
        role="switch"
        aria-checked={checked}
        aria-label={label}
        tabIndex={disabled ? -1 : 0}
      >
        <div className="pal-toggle__track">
          <div className="pal-toggle__thumb">
            <div className="pal-toggle__thumb-inner" />
          </div>
        </div>
      </div>
      {label && (
        <button
          type="button"
          className="pal-toggle-label"
          onClick={handleToggle}
          onKeyDown={handleKeyDown}
          disabled={disabled}
        >
          {label}
        </button>
      )}
    </div>
  );
};

export default MacToggle;
