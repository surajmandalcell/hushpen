import React, { useState, useRef, useEffect } from "react";
import { ChevronDown, Check } from "lucide-react";
import "./MacDropdown.css";

export interface MacDropdownOption {
  value: string;
  label: string;
  disabled?: boolean;
}

export interface MacDropdownProps {
  options: MacDropdownOption[];
  value?: string;
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  error?: string;
  label?: string;
  size?: "small" | "medium" | "large";
  className?: string;
  fullWidth?: boolean;
}

const MacDropdown: React.FC<MacDropdownProps> = ({
  options,
  value,
  onChange,
  placeholder = "Select an option",
  disabled = false,
  error,
  label,
  size = "medium",
  className = "",
  fullWidth = false,
}) => {
  const [isOpen, setIsOpen] = useState(false);
  const [focusedIndex, setFocusedIndex] = useState(-1);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLUListElement>(null);

  const selectedOption = options.find((option) => option.value === value);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
        setFocusedIndex(-1);
      }
    };

    if (isOpen) {
      document.addEventListener("mousedown", handleClickOutside);
    }

    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [isOpen]);

  const handleKeyDown = (event: React.KeyboardEvent) => {
    if (disabled) return;

    switch (event.key) {
      case "Enter":
      case " ":
        event.preventDefault();
        if (!isOpen) {
          setIsOpen(true);
          setFocusedIndex(0);
        } else if (focusedIndex >= 0) {
          const option = options[focusedIndex];
          if (!option.disabled) {
            onChange(option.value);
            setIsOpen(false);
            setFocusedIndex(-1);
          }
        }
        break;
      case "ArrowDown":
        event.preventDefault();
        if (!isOpen) {
          setIsOpen(true);
          setFocusedIndex(0);
        } else {
          setFocusedIndex((prev) =>
            prev < options.length - 1 ? prev + 1 : prev
          );
        }
        break;
      case "ArrowUp":
        event.preventDefault();
        if (isOpen) {
          setFocusedIndex((prev) => (prev > 0 ? prev - 1 : prev));
        }
        break;
      case "Escape":
        setIsOpen(false);
        setFocusedIndex(-1);
        break;
    }
  };

  const handleOptionClick = (option: MacDropdownOption) => {
    if (!option.disabled) {
      onChange(option.value);
      setIsOpen(false);
      setFocusedIndex(-1);
    }
  };

  const containerClasses = [
    "mac-dropdown-container",
    fullWidth ? "mac-dropdown-container--full-width" : "",
    disabled ? "mac-dropdown-container--disabled" : "",
  ]
    .filter(Boolean)
    .join(" ");

  const triggerClasses = [
    "mac-dropdown-trigger",
    `mac-dropdown-trigger--${size}`,
    error ? "mac-dropdown-trigger--error" : "",
    isOpen ? "mac-dropdown-trigger--open" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={containerClasses} ref={dropdownRef}>
      {label && <label className="mac-dropdown-label">{label}</label>}
      <div
        className={triggerClasses}
        onClick={() => !disabled && setIsOpen(!isOpen)}
        onKeyDown={handleKeyDown}
        tabIndex={disabled ? -1 : 0}
        role="combobox"
        aria-expanded={isOpen}
        aria-haspopup="listbox"
        aria-label={label}
      >
        <span className="mac-dropdown-value">
          {selectedOption ? selectedOption.label : placeholder}
        </span>
        <ChevronDown
          size={14}
          className={`mac-dropdown-chevron ${
            isOpen ? "mac-dropdown-chevron--open" : ""
          }`}
        />
      </div>
      {isOpen && (
        <ul className="mac-dropdown-menu" ref={listRef} role="listbox">
          {options.map((option, index) => (
            <li
              key={option.value}
              className={`mac-dropdown-option ${
                option.disabled ? "mac-dropdown-option--disabled" : ""
              } ${
                index === focusedIndex ? "mac-dropdown-option--focused" : ""
              } ${
                option.value === value ? "mac-dropdown-option--selected" : ""
              }`}
              onClick={() => handleOptionClick(option)}
              role="option"
              aria-selected={option.value === value}
              aria-disabled={option.disabled}
            >
              <span className="mac-dropdown-option-text">{option.label}</span>
              {option.value === value && (
                <Check size={12} className="mac-dropdown-check" />
              )}
            </li>
          ))}
        </ul>
      )}
      {error && <div className="mac-dropdown-error">{error}</div>}
    </div>
  );
};

export default MacDropdown;
