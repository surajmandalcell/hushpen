import type { ButtonHTMLAttributes, ReactNode } from "react";
import { dropdownOptionButtonClass, dropdownSurfaceClass } from "./classes";

interface DropdownSurfaceProps {
  id: string;
  ariaLabel: string;
  alignClassName: string;
  children: ReactNode;
}

export function DropdownSurface({ id, ariaLabel, alignClassName, children }: DropdownSurfaceProps) {
  return (
    <div id={id} role="listbox" aria-label={ariaLabel} className={`${alignClassName} ${dropdownSurfaceClass}`}>
      {children}
    </div>
  );
}

interface DropdownOptionButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  selected: boolean;
}

export function DropdownOptionButton({ selected, className = "", children, ...props }: DropdownOptionButtonProps) {
  return (
    <button
      type="button"
      role="option"
      aria-selected={selected}
      {...props}
      className={`${dropdownOptionButtonClass} ${selected ? "bg-[#5a5a5a] text-white" : "text-[#dddddd] hover:bg-[#454545]"} ${className}`}
    >
      {children}
    </button>
  );
}
