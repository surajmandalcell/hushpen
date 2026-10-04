import type { ReactNode } from "react";

interface HoverPopoverProps {
  content: string;
  children: ReactNode;
}

export function HoverPopover({ content, children }: HoverPopoverProps) {
  return (
    <span className="group relative inline-flex">
      {children}
      <span className="pointer-events-none absolute right-0 top-full z-30 mt-1 whitespace-nowrap rounded-[8px] border border-white/[0.1] bg-[#1f1f1f] px-2 py-1 text-[11px] font-semibold text-[#eeeeee] opacity-0 shadow-xl shadow-black/35 transition group-hover:opacity-100 group-focus-within:opacity-100">
        {content}
      </span>
    </span>
  );
}
