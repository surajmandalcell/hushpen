import type { ViewId, WindowAction } from "../../types/app";
import { views } from "../../views";
import { WindowDots } from "./WindowDots";

interface SidebarProps {
  activeView: ViewId;
  onChange: (view: ViewId) => void;
  onWindowAction: (action: WindowAction) => void;
}

export function Sidebar({ activeView, onChange, onWindowAction }: SidebarProps) {
  return (
    <aside className="flex min-h-0 flex-col border-b border-[#545454] bg-[#3c3c3c] text-[#d8d8d8] sm:border-b-0">
      <div className="flex h-12 items-center gap-3 px-4 [-webkit-app-region:drag]">
        <WindowDots onWindowAction={onWindowAction} />
      </div>

      <nav className="scrollbar-macos flex gap-1 overflow-x-auto px-2.5 pb-3 pt-1 sm:block sm:min-h-0 sm:overflow-y-auto" aria-label="Primary">
        {views.map((item) => {
          const Icon = item.icon;
          const isActive = activeView === item.id;

          return (
            <button
              key={item.id}
              type="button"
              aria-label={item.label}
              aria-current={isActive ? "page" : undefined}
              className={`mb-1 flex h-9 shrink-0 items-center gap-2 rounded-[9px] px-2.5 text-left text-[13px] font-semibold transition outline-none focus:outline-none focus-visible:ring-2 focus-visible:ring-[#9bcfff]/70 focus-visible:ring-offset-1 focus-visible:ring-offset-[#3c3c3c] sm:w-full ${
                isActive
                  ? "bg-[#686868] text-white"
                  : "text-[#d0d0d0] hover:bg-[#505050]"
              }`}
              onClick={() => onChange(item.id)}
            >
              <span className={`grid size-5 shrink-0 place-items-center rounded-md ${item.tone}`}>
                <Icon className="size-3.5" />
              </span>
              <span>{item.label}</span>
            </button>
          );
        })}
      </nav>
    </aside>
  );
}
