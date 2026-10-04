import { BrainCircuit } from "lucide-react";
import {
  iconTileClass,
  modelMetaBadgeClass,
  panelDividerClass,
  sharedRadiusClass,
} from "../../components/ui/classes";
import { GroupedPanel } from "../../components/ui/GroupedPanel";
import { ViewFrame } from "../../components/ui/ViewFrame";
import { formatByteCount } from "../../lib/format";
import { clampNumber } from "../../lib/math";
import type { ViewProps } from "../../types/view";
import { ModelActionButton, ModelSelectButton, ModelStatusLabel } from "./ModelControls";
import { ResourceStatsPanel } from "./ResourceStatsPanel";

export function ModelsView({ models: library, runtimeInfo }: ViewProps) {
  const { selectedModel, models, busyModelIds, progressById, error: actionError } = library;
  const storageStats = runtimeInfo?.storageStats;
  const engine = runtimeInfo?.engine;

  return (
    <ViewFrame title="Models library">
      <GroupedPanel title="Recognition models" allowOverflow>
        {models.map((model) => {
          const selected = selectedModel === model.displayName;
          const busy = busyModelIds.has(model.id);
          const installed = Boolean(model.installed);
          const diskLabel = installed && model.diskBytes ? formatByteCount(model.diskBytes) : model.sizeLabel;
          const trackedProgress = progressById[model.id];
          const fallbackProgress = busy && engine?.modelId === model.id && typeof engine.progress === "number"
            ? clampNumber(engine.progress, 0, 100)
            : null;
          const progress = busy && typeof trackedProgress === "number"
            ? clampNumber(trackedProgress, 0, 100)
            : fallbackProgress;

          return (
            <div
              key={model.id}
              className={`border-t ${panelDividerClass} p-3 first:border-t-0 ${selected ? "bg-white/[0.055]" : ""}`}
            >
              <div className="flex min-w-0 flex-col gap-3 sm:flex-row sm:items-center">
                <div
                  className={`flex min-w-0 flex-1 items-center gap-3 ${sharedRadiusClass} px-2 py-1.5 text-left`}
                >
                  <div className={iconTileClass}>
                    <BrainCircuit className="size-3" />
                  </div>
                  <span className="min-w-0 flex-1">
                    <span className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
                      <span className="text-[13px] font-semibold leading-5 text-[#f2f2f2]">{model.displayName}</span>
                      <span className={modelMetaBadgeClass}>{diskLabel}</span>
                    </span>
                    <span className="selectable-text mt-0.5 block text-[12px] font-medium leading-4 text-[#aaa]">{model.detail}</span>
                  </span>
                </div>
                <div data-model-controls className="grid shrink-0 grid-cols-[32px_32px_32px] items-center gap-2 pl-11 sm:pl-0">
                  <ModelSelectButton
                    modelName={model.displayName}
                    selected={selected}
                    onClick={() => library.selectModel(model.displayName)}
                  />
                  <ModelStatusLabel installed={installed} modelName={model.displayName} />
                  {installed ? (
                    <ModelActionButton
                      ariaLabel={`Delete ${model.displayName}`}
                      busy={busy}
                      kind="delete"
                      onClick={() => library.deleteModel(model.id)}
                    />
                  ) : (
                    <ModelActionButton
                      ariaLabel={`Download ${model.displayName}`}
                      busy={busy}
                      kind="download"
                      onClick={() => library.downloadModel(model.id)}
                    />
                  )}
                </div>
              </div>
              {progress !== null ? (
                <div className="mt-3 pl-11">
                  <div className="flex items-center justify-between text-[11px] font-semibold text-[#9c9c9c]">
                    <span>Setup progress</span>
                    <span>{Math.round(progress)}%</span>
                  </div>
                  <div className="mt-1 h-1.5 overflow-hidden rounded-full bg-white/[0.08]">
                    <div className="h-full rounded-full bg-[#9bcfff]" style={{ width: `${progress}%` }} />
                  </div>
                </div>
              ) : null}
            </div>
          );
        })}
      </GroupedPanel>
      {actionError ? (
        <p role="status" className="selectable-text px-1 text-[12px] font-semibold text-[#ffb3aa]">{actionError}</p>
      ) : null}
      <ResourceStatsPanel stats={storageStats} />
    </ViewFrame>
  );
}
