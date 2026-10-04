import type errorCodes from "../../shared/error-codes.json";
import type ipcChannels from "../../shared/ipc-channels.json";
import type settingsDefaults from "../../shared/settings-defaults.json";

export type ErrorCode = keyof typeof errorCodes.codes;
export type ChannelName = keyof typeof ipcChannels.channels;
export type SettingKey = keyof typeof settingsDefaults;
export type SettingsValues = Record<SettingKey, unknown>;

export interface ErrorShape {
  code: ErrorCode;
  params?: Record<string, unknown>;
  detail?: string;
}

export type Envelope<T> = { ok: true; value: T } | { ok: false; error: ErrorShape };
