import { useEffect, useRef, useState } from "react";
import { Check } from "lucide-react";
import { AudioInputDeviceIcon } from "../../components/ui/AudioInputDeviceIcon";
import { focusRingClass, sharedRadiusClass } from "../../components/ui/classes";
import { DropdownOptionButton, DropdownSurface } from "../../components/ui/Dropdown";
import { PanelControlButton } from "../../components/ui/PanelControlButton";
import type { AudioInputDeviceOption } from "../../types/audio";

interface MicrophoneSelectorProps {
  ariaLabel: string;
  devices: AudioInputDeviceOption[];
  disabled?: boolean;
  selectedDeviceId: string;
  selectedLabel: string;
  variant: "toolbar" | "panel";
  onSelect: (deviceId: string) => void;
}

export function MicrophoneSelector({
  ariaLabel,
  devices,
  disabled = false,
  selectedDeviceId,
  selectedLabel,
  variant,
  onSelect,
}: MicrophoneSelectorProps) {
  const [isOpen, setIsOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const listboxId = useRef(`mic-options-${Math.random().toString(36).slice(2)}`);
  const isToolbar = variant === "toolbar";
  const selectedDevice = devices.find((device) => device.id === selectedDeviceId) ?? {
    id: selectedDeviceId,
    label: selectedLabel,
  };
  const triggerContent = (
    <>
      <AudioInputDeviceIcon device={selectedDevice} className={isToolbar ? "size-3 shrink-0 text-current" : "size-3 shrink-0 text-[#bdbdbd]"} />
      <span className={isToolbar ? "hidden min-w-0 truncate sm:inline" : "min-w-0 flex-1 whitespace-normal break-words text-left leading-4"}>
        {selectedLabel}
      </span>
    </>
  );

  useEffect(() => {
    if (!isOpen) return undefined;

    const handlePointerDown = (event: PointerEvent) => {
      if (rootRef.current?.contains(event.target as Node)) return;
      setIsOpen(false);
    };

    const handleKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        setIsOpen(false);
      }
    };

    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [isOpen]);

  const handleSelect = (deviceId: string) => {
    onSelect(deviceId);
    setIsOpen(false);
  };

  return (
    <div ref={rootRef} className={`relative min-w-0 ${isToolbar ? "[-webkit-app-region:no-drag]" : "w-full"}`}>
      {isToolbar ? (
        <button
          type="button"
          aria-controls={isOpen ? listboxId.current : undefined}
          aria-expanded={isOpen}
          aria-haspopup="listbox"
          aria-label={ariaLabel}
          disabled={disabled}
          className={`toolbar-mic-trigger inline-flex h-7 max-w-[260px] min-w-0 items-center gap-1.5 ${sharedRadiusClass} px-1.5 text-[12px] font-medium text-[#bdbdbd] disabled:cursor-not-allowed disabled:text-[#7d7d7d] ${focusRingClass}`}
          onClick={() => setIsOpen((current) => !current)}
        >
          {triggerContent}
        </button>
      ) : (
        <PanelControlButton
          type="button"
          aria-controls={isOpen ? listboxId.current : undefined}
          aria-expanded={isOpen}
          aria-haspopup="listbox"
          aria-label={ariaLabel}
          disabled={disabled}
          className="w-full min-w-0 justify-start px-2 py-1.5 text-left"
          onClick={() => setIsOpen((current) => !current)}
        >
          {triggerContent}
        </PanelControlButton>
      )}

      {isOpen ? (
        <DropdownSurface
          id={listboxId.current}
          ariaLabel="Microphone options"
          alignClassName={isToolbar ? "right-0 top-full mt-1 w-[320px] max-w-[calc(100vw-1rem)]" : "left-0 top-full mt-1 w-full min-w-[260px]"}
        >
          {devices.map((device) => {
            const selected = device.id === selectedDeviceId;

            return (
              <DropdownOptionButton
                key={device.id}
                selected={selected}
                onClick={() => handleSelect(device.id)}
              >
                <AudioInputDeviceIcon device={device} className="mt-0.5 size-3 shrink-0 text-[#bdbdbd]" />
                <span className="min-w-0 flex-1 whitespace-normal break-words">{device.label}</span>
                {selected ? <Check className="mt-0.5 size-3 shrink-0 text-[#9bcfff]" /> : null}
              </DropdownOptionButton>
            );
          })}
        </DropdownSurface>
      ) : null}
    </div>
  );
}
