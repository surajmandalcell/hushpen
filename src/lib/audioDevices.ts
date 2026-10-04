import { Bluetooth, Camera, Headphones, Laptop, Mic2, Smartphone, Usb, type LucideIcon } from "lucide-react";
import type { AudioInputDeviceIconType, AudioInputDeviceOption } from "../types/audio";
import { defaultAudioInputId, defaultAudioInputOptions } from "./defaults";

export const audioInputDeviceIconByType: Record<AudioInputDeviceIconType, LucideIcon> = {
  mic: Mic2,
  laptop: Laptop,
  phone: Smartphone,
  webcam: Camera,
  headphones: Headphones,
  bluetooth: Bluetooth,
  usb: Usb,
};

export function getAudioInputDeviceIconType(device: AudioInputDeviceOption): AudioInputDeviceIconType {
  const value = `${device.id} ${device.label}`.toLowerCase();

  if (device.id === "default" || value.includes("system default")) return "mic";
  if (/(iphone|ipad|android|mobile|\bphone\b)/.test(value)) return "phone";
  if (/(macbook|built-in|builtin|internal|laptop)/.test(value)) return "laptop";
  if (/(webcam|camera|facetime|logitech|brio|c920)/.test(value)) return "webcam";
  if (/(airpods|headphone|headset|earbud|earphone|buds)/.test(value)) return "headphones";
  if (/(bluetooth|\bbt\b)/.test(value)) return "bluetooth";
  if (/(usb|external|interface|focusrite|scarlett|yeti|rode|shure|elgato|studio)/.test(value)) return "usb";

  return "mic";
}

export function buildAudioInputDeviceOptions(devices: MediaDeviceInfo[]): AudioInputDeviceOption[] {
  const options: AudioInputDeviceOption[] = [...defaultAudioInputOptions];
  const seenDeviceIds = new Set([defaultAudioInputId]);
  let unnamedAudioInputCount = 0;

  for (const device of devices) {
    if (device.kind !== "audioinput" || !device.deviceId || seenDeviceIds.has(device.deviceId)) {
      continue;
    }

    seenDeviceIds.add(device.deviceId);
    unnamedAudioInputCount += 1;
    const label = device.label.trim() || `Microphone ${unnamedAudioInputCount}`;
    options.push({ id: device.deviceId, label });
  }

  return options;
}
