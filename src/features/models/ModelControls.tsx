import { Check, CheckCircle2, Download, RefreshCw, Trash2 } from "lucide-react";
import { focusRingClass } from "../../components/ui/classes";
import { HoverPopover } from "../../components/ui/HoverPopover";

interface ModelSelectButtonProps {
  modelName: string;
  selected: boolean;
  onClick: () => void;
}

export function ModelSelectButton({ modelName, selected, onClick }: ModelSelectButtonProps) {
  return (
    <HoverPopover content={selected ? "Current model" : "Select model"}>
      <button
        type="button"
        aria-label={`Select ${modelName}`}
        aria-pressed={selected}
        className={`grid size-8 shrink-0 place-items-center rounded-full border-0 bg-transparent p-0 transition active:scale-[0.96] ${focusRingClass} ${selected ? "text-[#9bcfff] hover:bg-[#263b4d]" : "text-[#cfcfcf] hover:bg-white/[0.08] hover:text-[#eeeeee]"}`}
        onClick={onClick}
      >
        <Check className="size-3.5" />
      </button>
    </HoverPopover>
  );
}

export function ModelStatusLabel({ installed, modelName }: { installed: boolean; modelName: string }) {
  if (installed) {
    return (
      <HoverPopover content="Downloaded model">
        <span
          role="img"
          aria-label={`${modelName} downloaded`}
          className="grid size-8 shrink-0 place-items-center rounded-full text-[#a9d9b8]"
        >
          <CheckCircle2 className="size-3.5" />
        </span>
      </HoverPopover>
    );
  }

  return (
    <HoverPopover content="Not downloaded">
      <span
        role="img"
        aria-label={`${modelName} not downloaded`}
        className="grid size-8 shrink-0 place-items-center rounded-full text-[#cfcfcf] opacity-75"
      >
        <CheckCircle2 className="size-3.5" />
      </span>
    </HoverPopover>
  );
}

interface ModelActionButtonProps {
  ariaLabel: string;
  busy: boolean;
  kind: "download" | "delete";
  onClick: () => void;
}

export function ModelActionButton({ ariaLabel, busy, kind, onClick }: ModelActionButtonProps) {
  const Icon = busy ? RefreshCw : kind === "delete" ? Trash2 : Download;
  const toneClass = kind === "delete"
    ? "text-[#cfcfcf] hover:bg-[#4a3333] hover:text-[#ffb3aa]"
    : "text-[#cfcfcf] hover:bg-[#344235] hover:text-[#bce7c9]";

  return (
    <HoverPopover content={kind === "delete" ? "Delete model" : "Download model"}>
      <button
        type="button"
        aria-label={ariaLabel}
        className={`grid size-8 shrink-0 place-items-center rounded-full border-0 bg-transparent p-0 transition active:scale-[0.96] disabled:cursor-wait disabled:opacity-55 ${toneClass} ${focusRingClass}`}
        disabled={busy}
        onClick={onClick}
      >
        <Icon className={`size-3.5 ${busy ? "animate-spin" : ""}`} />
      </button>
    </HoverPopover>
  );
}
