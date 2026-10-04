import { audioInputDeviceIconByType, getAudioInputDeviceIconType } from "../../lib/audioDevices";
import type { AudioInputDeviceOption } from "../../types/audio";

export function AudioInputDeviceIcon({ device, className }: { device: AudioInputDeviceOption; className: string }) {
  const iconType = getAudioInputDeviceIconType(device);
  const Icon = audioInputDeviceIconByType[iconType];

  return <Icon aria-hidden="true" data-device-icon={iconType} className={className} />;
}
