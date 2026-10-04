import { ArrowUpRight } from "lucide-react";
import { sharedRadiusClass } from "./classes";

interface NavigateButtonProps {
  label: string;
  onClick: () => void;
}

export function NavigateButton({ label, onClick }: NavigateButtonProps) {
  return (
    <button
      type="button"
      className={`inline-flex h-7 items-center gap-1.5 ${sharedRadiusClass} bg-white/[0.08] px-2.5 text-[12px] font-semibold text-[#eeeeee] transition hover:bg-white/[0.12] active:scale-[0.97]`}
      onClick={onClick}
    >
      <span>{label}</span>
      <ArrowUpRight className="size-3" />
    </button>
  );
}
