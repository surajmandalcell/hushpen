import { Activity, HardDrive } from "lucide-react";
import { iconTileClass, panelDividerClass, sharedRadiusClass } from "../../components/ui/classes";
import { GroupedPanel } from "../../components/ui/GroupedPanel";
import { PanelRow } from "../../components/ui/PanelRow";
import { StatusLabel } from "../../components/ui/StatusLabel";
import { formatByteCount, formatHomeRelativePath } from "../../lib/format";
import type { RuntimeStorageStats } from "../../types/runtime";

export function ResourceStatsPanel({ stats }: { stats?: RuntimeStorageStats }) {
  const groups = stats?.groups ?? [];

  return (
    <GroupedPanel title="Storage and memory" allowOverflow>
      {groups.length ? (
        groups.map((group) => (
          <section key={group.id} className={`border-t ${panelDividerClass} p-4 first:border-t-0`}>
            <div className="flex items-center gap-3">
              <div className={iconTileClass}>
                {group.id === "memory" ? <Activity className="size-3" /> : <HardDrive className="size-3" />}
              </div>
              <div className="min-w-0 flex-1">
                <p className="text-[13px] font-semibold text-[#eeeeee]">{group.label}</p>
                {group.detail ? <p className="selectable-text mt-0.5 truncate text-[12px] font-medium text-[#aaa]">{group.detail}</p> : null}
              </div>
              <span className="shrink-0 text-[13px] font-semibold text-[#f0f0f0]">{formatByteCount(group.totalBytes)}</span>
            </div>
            <div className="mt-3 grid grid-cols-1 gap-2 sm:grid-cols-2">
              {group.items.map((item) => (
                <div key={item.id} className={`${sharedRadiusClass} border border-white/[0.07] bg-white/[0.045] px-3 py-2`}>
                  <div className="flex min-w-0 items-center justify-between gap-2">
                    <span className="truncate text-[12px] font-semibold text-[#d8d8d8]">{item.label}</span>
                    <span className="shrink-0 text-[12px] font-semibold text-[#eeeeee]">{formatByteCount(item.bytes)}</span>
                  </div>
                  {item.detail || item.path ? (
                    <p className="selectable-text mt-1 truncate text-[11px] font-medium text-[#8f8f8f]">{item.detail ?? formatHomeRelativePath(item.path)}</p>
                  ) : null}
                </div>
              ))}
            </div>
          </section>
        ))
      ) : (
        <div className="p-4">
          <PanelRow
            icon={<HardDrive className="size-3.5" />}
            title="Runtime stats"
            detail="Waiting for desktop storage and memory details"
            trailing={<StatusLabel>Pending</StatusLabel>}
          />
        </div>
      )}
    </GroupedPanel>
  );
}
