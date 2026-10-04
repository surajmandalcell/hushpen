import { MicrophoneSelector } from "../../features/devices/MicrophoneSelector";
import type { AudioInputDevices } from "../../features/devices/useAudioInputDevices";

interface ToolbarProps {
  activeTitle: string;
  audio: AudioInputDevices;
}

export function Toolbar({ activeTitle, audio }: ToolbarProps) {
  return (
    <header className="flex min-w-0 items-center justify-between border-b border-[#3c3c3c]/70 bg-transparent px-4 [-webkit-app-region:drag]">
      <div className="flex min-w-0 items-center">
        <span className="truncate text-[12px] font-semibold text-[#bdbdbd]">{activeTitle}</span>
      </div>
      <MicrophoneSelector
        ariaLabel="Toolbar microphone selector"
        devices={audio.devices}
        disabled={audio.loading}
        selectedDeviceId={audio.selectedDeviceId}
        selectedLabel={audio.selectedLabel}
        variant="toolbar"
        onSelect={audio.select}
      />
    </header>
  );
}
