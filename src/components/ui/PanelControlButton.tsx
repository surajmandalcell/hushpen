import type { ButtonHTMLAttributes } from "react";
import { panelControlButtonClass } from "./classes";

export function PanelControlButton({ className = "", children, ...props }: ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button {...props} className={`${panelControlButtonClass} ${className}`}>
      {children}
    </button>
  );
}
