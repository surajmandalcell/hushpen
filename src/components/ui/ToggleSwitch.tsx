import { focusRingClass } from "./classes";

interface ToggleSwitchProps {
  label: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
}

export function ToggleSwitch({ label, checked, disabled = false, onChange }: ToggleSwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      className={`relative h-6 w-11 shrink-0 rounded-full border border-white/[0.1] transition ${focusRingClass} ${disabled ? "cursor-not-allowed opacity-50" : ""} ${checked ? "bg-[#5f9fc6]/70" : "bg-[#2b2b2b]"}`}
      onClick={() => onChange(!checked)}
    >
      <span
        aria-hidden="true"
        className={`absolute left-[3px] top-[3px] size-[18px] rounded-full bg-[#f1f1f1] shadow-[0_1px_4px_rgba(0,0,0,0.35)] transition-transform ${checked ? "translate-x-5" : "translate-x-0"}`}
      />
    </button>
  );
}
