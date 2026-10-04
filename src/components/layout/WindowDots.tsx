import { Minus, X } from "lucide-react";
import type { WindowAction } from "../../types/app";

interface WindowDotsProps {
  onWindowAction: (action: WindowAction) => void;
}

export function WindowDots({ onWindowAction }: WindowDotsProps) {
  const dotButtonClass =
    "grid size-[13px] place-items-center rounded-full border-0 p-0 shadow-none outline-none transition-transform duration-150 [appearance:none] hover:scale-105 focus:outline-none focus-visible:outline-none focus-visible:ring-0 active:outline-none";
  const dotIconClass =
    "size-[9px] opacity-0 transition-opacity duration-100 group-hover/window-dots:opacity-75";

  return (
    <div className="group/window-dots flex shrink-0 items-center gap-[7px] [-webkit-app-region:no-drag]">
      <button
        aria-label="Close window"
        className={`${dotButtonClass} bg-[#ff5f57]`}
        type="button"
        onClick={() => onWindowAction("close")}
      >
        <X
          aria-hidden="true"
          data-window-dot-icon="close"
          strokeWidth={2.6}
          className={`${dotIconClass} text-[#6e140f]`}
        />
      </button>
      <button
        aria-label="Minimize window"
        className={`${dotButtonClass} bg-[#febc2e]`}
        type="button"
        onClick={() => onWindowAction("minimize")}
      >
        <Minus
          aria-hidden="true"
          data-window-dot-icon="minimize"
          strokeWidth={3}
          className={`${dotIconClass} text-[#8f5b00]`}
        />
      </button>
    </div>
  );
}
