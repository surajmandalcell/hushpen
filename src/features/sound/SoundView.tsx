import { BrainCircuit, Mic2, RefreshCw } from "lucide-react";
import { GroupedPanel } from "../../components/ui/GroupedPanel";
import { NavigateButton } from "../../components/ui/NavigateButton";
import { PanelControlButton } from "../../components/ui/PanelControlButton";
import { PanelRow } from "../../components/ui/PanelRow";
import { StatusLabel } from "../../components/ui/StatusLabel";
import { ViewFrame } from "../../components/ui/ViewFrame";
import { defaultAudioInputId } from "../../lib/defaults";
import type { ViewProps } from "../../types/view";
import { MicrophoneSelector } from "../devices/MicrophoneSelector";

export function SoundView({ models, recording, audio, navigate }: ViewProps) {
  const { isRecording } = recording;
  const { selectedLabel, selectedDeviceId, loading, error } = audio;

  return (
    <ViewFrame title="Sound">
      <GroupedPanel title="Input" allowOverflow>
        <PanelRow
          icon={<Mic2 className="size-3.5" />}
          title="Microphone"
          detail={isRecording ? `Recording with ${selectedLabel}` : selectedLabel}
          trailing={<StatusLabel>{isRecording ? "Live" : selectedDeviceId === defaultAudioInputId ? "Default" : "Ready"}</StatusLabel>}
          extra={(
            <div className="space-y-2">
              <div className="flex min-w-0 flex-col gap-2 sm:flex-row sm:items-center">
                <MicrophoneSelector
                  ariaLabel="Microphone selector"
                  devices={audio.devices}
                  disabled={isRecording || loading}
                  selectedDeviceId={selectedDeviceId}
                  selectedLabel={selectedLabel}
                  variant="panel"
                  onSelect={audio.select}
                />
                <PanelControlButton
                  type="button"
                  aria-label="Refresh microphones"
                  className="h-8 px-2.5 hover:bg-[#4a4a4a]"
                  disabled={loading}
                  onClick={audio.refresh}
                >
                  <RefreshCw className={`size-3 ${loading ? "animate-spin" : ""}`} />
                  <span>Refresh</span>
                </PanelControlButton>
              </div>
              {error ? (
                <p role="status" className="selectable-text text-[12px] font-medium text-[#ffb3aa]">
                  {error}
                </p>
              ) : null}
            </div>
          )}
        />
        <PanelRow
          icon={<BrainCircuit className="size-3.5" />}
          title="Recognition model"
          detail={models.selectedModel}
          trailing={<NavigateButton label="Change" onClick={() => navigate("models")} />}
        />
      </GroupedPanel>
    </ViewFrame>
  );
}
