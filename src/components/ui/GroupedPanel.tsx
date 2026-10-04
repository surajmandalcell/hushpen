import type { ReactNode } from "react";
import { panelGlassClass, panelSurfaceClass } from "./classes";

interface GroupedPanelProps {
  title?: string;
  allowOverflow?: boolean;
  children: ReactNode;
}

export function GroupedPanel({ title, allowOverflow = false, children }: GroupedPanelProps) {
  return (
    <section className="space-y-2">
      {title ? <h3 className="px-1 text-[13px] font-semibold text-[#a8a8a8]">{title}</h3> : null}
      <div className={allowOverflow ? panelGlassClass : panelSurfaceClass}>{children}</div>
    </section>
  );
}
