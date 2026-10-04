import type { ReactNode } from "react";

interface ShortcutClusterProps {
  parts: string[];
  muted?: boolean;
}

export function ShortcutCluster({ parts, muted = false }: ShortcutClusterProps) {
  return (
    <span className="inline-flex shrink-0 items-center gap-1">
      {parts.map((part) => (
        <ShortcutBadge key={part} muted={muted}>{part}</ShortcutBadge>
      ))}
    </span>
  );
}

interface ShortcutBadgeProps {
  children: ReactNode;
  muted?: boolean;
}

function ShortcutBadge({ children, muted = false }: ShortcutBadgeProps) {
  return <span className={`grid min-w-5 place-items-center rounded-[5px] px-1.5 py-1 text-[11px] font-semibold leading-none ${muted ? "bg-[#3a3a3a] text-[#858585]" : "bg-[#646464] text-[#f2f2f2]"}`}>{children}</span>;
}
