import { GroupedPanel } from "../../components/ui/GroupedPanel";
import { NavigateButton } from "../../components/ui/NavigateButton";
import { PanelRow } from "../../components/ui/PanelRow";
import { SegmentedControl, type SegmentedControlOption } from "../../components/ui/SegmentedControl";
import { ShortcutCluster } from "../../components/ui/ShortcutCluster";
import { StatusLabel } from "../../components/ui/StatusLabel";
import { ToggleSwitch } from "../../components/ui/ToggleSwitch";
import { ViewFrame } from "../../components/ui/ViewFrame";
import { formatEngineStatus } from "../../lib/format";
import { formatShortcutParts } from "../../lib/shortcut";
import type { OverlayPlacement } from "../../types/settings";
import type { ViewProps } from "../../types/view";
import { TextEditorSelector } from "./TextEditorSelector";

const overlayPlacementControlOptions: readonly SegmentedControlOption<OverlayPlacement>[] = [
  { value: "top", label: "Top", ariaLabel: "Top overlay position" },
  { value: "bottom", label: "Bottom", ariaLabel: "Bottom overlay position" },
];

export function SettingsView({ runtimeInfo, models, audio, settings, navigate }: ViewProps) {
  const shortcutParts = formatShortcutParts(runtimeInfo?.shortcut);
  const engine = runtimeInfo?.engine;
  const engineStatus = formatEngineStatus(engine?.status);
  const engineDetail = engine?.error || engine?.detail || (engine?.status === "idle" ? "Loads the selected Whisper model when needed" : engine?.model || engine?.mode || "Waiting for desktop runtime");
  const startup = runtimeInfo?.startup;
  const startupSupported = startup?.supported ?? Boolean(window.asrpro?.setStartupLaunch);
  const startupPath = startup?.executablePath || startup?.registeredExecutablePath || "Starts ASR Pro when you sign in";
  const startupDetail = startup?.detail || startupPath;

  return (
    <ViewFrame title="Configuration">
      <GroupedPanel title="Recording overlay">
        <PanelRow
          title="Position"
          detail="Floating waveform location"
          trailing={<OverlayPlacementControl placement={settings.overlayPlacement} onChange={settings.changeOverlayPlacement} />}
        />
      </GroupedPanel>

      <GroupedPanel title="Keyboard shortcuts">
        <PanelRow title="Toggle recording" detail="Registered by the desktop app" trailing={<ShortcutCluster parts={shortcutParts} />} />
      </GroupedPanel>

      <GroupedPanel title="Application" allowOverflow>
        <PanelRow title="Default model" detail={runtimeInfo?.defaultModel ?? models.selectedModel} trailing={<NavigateButton label="Change" onClick={() => navigate("models")} />} />
        <PanelRow title="Microphone input" detail={audio.selectedLabel} trailing={<NavigateButton label="Change" onClick={() => navigate("sound")} />} />
        <PanelRow
          title="Transcript editor"
          detail="Used for history text files"
          trailing={(
            <TextEditorSelector
              options={settings.textEditorOptions}
              selectedEditorId={settings.selectedTextEditorId}
              selectedLabel={settings.selectedTextEditorLabel}
              onSelect={settings.changeTextEditor}
            />
          )}
        />
        <PanelRow
          title="Auto-copy transcripts"
          detail="Copy completed dictation to clipboard"
          trailing={(
            <ToggleSwitch
              label="Auto-copy transcripts"
              checked={settings.autoCopyTranscripts}
              onChange={settings.changeAutoCopyTranscripts}
            />
          )}
        />
        <PanelRow
          title="Launch at startup"
          detail={startupDetail}
          trailing={(
            <ToggleSwitch
              label="Launch at startup"
              checked={settings.launchAtStartup}
              disabled={!startupSupported}
              onChange={settings.changeLaunchAtStartup}
            />
          )}
        />
        <PanelRow title="Engine" detail={engineDetail} trailing={<StatusLabel>{engineStatus}</StatusLabel>} />
        <PanelRow title="Data folder" detail={runtimeInfo?.dataDir ?? "App-contained data directory"} trailing={<StatusLabel>Read only</StatusLabel>} />
      </GroupedPanel>
    </ViewFrame>
  );
}

interface OverlayPlacementControlProps {
  placement: OverlayPlacement;
  onChange: (placement: OverlayPlacement) => void;
}

function OverlayPlacementControl({ placement, onChange }: OverlayPlacementControlProps) {
  return (
    <SegmentedControl value={placement} options={overlayPlacementControlOptions} onChange={onChange} />
  );
}
