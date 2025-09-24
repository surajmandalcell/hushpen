import React, { useState } from "react";


export interface MacSegmentedOption {
  value: string;
  label: string;
  disabled?: boolean;
}

export interface MacSegmentedProps {
  options: MacSegmentedOption[];
  value?: string;
  onChange: (value: string) => void;
  disabled?: boolean;
  size?: "small" | "medium" | "large";
  className?: string;
  fullWidth?: boolean;
}

const MacSegmented: React.FC<MacSegmentedProps> = ({
  options,
  value,
  onChange,
  disabled = false,
  size = "medium",
  className = "",
  fullWidth = false,
}) => {
  const [hoveredIndex, setHoveredIndex] = useState<number | null>(null);
  const selectedIndex = options.findIndex(option => option.value === value);

  const handleOptionClick = (option: MacSegmentedOption) => {
    if (!option.disabled && !disabled) {
      onChange(option.value);
    }
  };

  const handleKeyDown = (event: React.KeyboardEvent, option: MacSegmentedOption, index: number) => {
    if (disabled || option.disabled) return;

    switch (event.key) {
      case "Enter":
      case " ":
        event.preventDefault();
        onChange(option.value);
        break;
      case "ArrowLeft":
        event.preventDefault();
        const prevIndex = index > 0 ? index - 1 : options.length - 1;
        const prevOption = options[prevIndex];
        if (!prevOption.disabled) {
          onChange(prevOption.value);
        }
        break;
      case "ArrowRight":
        event.preventDefault();
        const nextIndex = index < options.length - 1 ? index + 1 : 0;
        const nextOption = options[nextIndex];
        if (!nextOption.disabled) {
          onChange(nextOption.value);
        }
        break;
    }
  };

  const containerClasses = [
    "mac-segmented",
    `mac-segmented--${size}`,
    disabled ? "mac-segmented--disabled" : "",
    fullWidth ? "mac-segmented--full-width" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div
      className={containerClasses}
      role="tablist"
      aria-label="Segmented control"
    >
      {options.map((option, index) => {
        const isSelected = selectedIndex === index;
        const isHovered = hoveredIndex === index;
        const isFirst = index === 0;
        const isLast = index === options.length - 1;

        const optionClasses = [
          "mac-segmented__option",
          isSelected ? "mac-segmented__option--selected" : "",
          option.disabled ? "mac-segmented__option--disabled" : "",
          isHovered && !isSelected ? "mac-segmented__option--hovered" : "",
          isFirst ? "mac-segmented__option--first" : "",
          isLast ? "mac-segmented__option--last" : "",
        ]
          .filter(Boolean)
          .join(" ");

        return (
          <button
            key={option.value}
            className={optionClasses}
            onClick={() => handleOptionClick(option)}
            onKeyDown={(e) => handleKeyDown(e, option, index)}
            onMouseEnter={() => setHoveredIndex(index)}
            onMouseLeave={() => setHoveredIndex(null)}
            disabled={disabled || option.disabled}
            role="tab"
            aria-selected={isSelected}
            aria-disabled={disabled || option.disabled}
            tabIndex={isSelected ? 0 : -1}
          >
            <span className="mac-segmented__option-label">{option.label}</span>
          </button>
        );
      })}
    </div>
  );
};

export default MacSegmented;
