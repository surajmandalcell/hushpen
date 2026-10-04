import type { ReactNode } from "react";
import { iconTileClass, panelDividerClass } from "./classes";

interface PanelRowProps {
  icon?: ReactNode;
  title: string;
  detail?: string;
  trailing?: ReactNode;
  extra?: ReactNode;
}

export function PanelRow({ icon, title, detail, trailing, extra }: PanelRowProps) {
  return (
    <div className={`border-t ${panelDividerClass} p-4 first:border-t-0`}>
      <div className="flex min-w-0 items-center gap-3">
        {icon ? <div className={iconTileClass}>{icon}</div> : null}
        <div className="min-w-0 flex-1">
          <p className="truncate text-[13px] font-semibold text-[#eeeeee]">{title}</p>
          {detail ? <p className="selectable-text mt-0.5 truncate text-[12px] font-medium text-[#aaa]">{detail}</p> : null}
        </div>
        {trailing ? <div className="shrink-0">{trailing}</div> : null}
      </div>
      {extra ? <div className={icon ? "mt-3 pl-11" : "mt-3"}>{extra}</div> : null}
    </div>
  );
}
