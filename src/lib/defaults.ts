import packageMetadata from "../../package.json";
import type { AppInfo } from "../types/app";
import type { AudioInputDeviceOption } from "../types/audio";
import type { EngineModelInfo } from "../types/engine";
import type { TextEditorOption } from "../types/settings";

export const defaultModelName = "Whisper Base English";
export const defaultAudioInputId = "default";
export const defaultAudioInputLabel = "System default";
export const defaultAudioInputOptions: AudioInputDeviceOption[] = [{ id: defaultAudioInputId, label: defaultAudioInputLabel }];
export const defaultTextEditorId = "system";
export const defaultAutoCopyTranscripts = true;
export const defaultTextEditorOptions: TextEditorOption[] = [
  { id: "system", label: "System default", detail: "Use the operating system default editor" },
  { id: "textedit", label: "TextEdit", detail: "Open transcript text in Apple TextEdit" },
  { id: "vscode", label: "Visual Studio Code", detail: "Open transcript text in VS Code" },
  { id: "cursor", label: "Cursor", detail: "Open transcript text in Cursor" },
];
export const modelIdsByName: Record<string, string> = {
  "Whisper Tiny English": "whisper-tiny-en",
  "Whisper Base English": "whisper-base-en",
  "Whisper Small English": "whisper-small-en",
  "Whisper Base Multilingual": "whisper-base",
  "Whisper Large v3 Turbo": "whisper-large-v3-turbo",
};

export const fallbackModelCards: EngineModelInfo[] = [
  {
    id: "whisper-tiny-en",
    displayName: "Whisper Tiny English",
    detail: "Fastest local model, lowest memory use",
    sizeLabel: "75 MB",
  },
  {
    id: "whisper-base-en",
    displayName: "Whisper Base English",
    detail: "Default local model for English dictation",
    sizeLabel: "142 MB",
  },
  {
    id: "whisper-base",
    displayName: "Whisper Base Multilingual",
    detail: "Small multilingual model with language detection",
    sizeLabel: "142 MB",
  },
  {
    id: "whisper-small-en",
    displayName: "Whisper Small English",
    detail: "Higher accuracy with a larger local model",
    sizeLabel: "466 MB",
  },
  {
    id: "whisper-large-v3-turbo",
    displayName: "Whisper Large v3 Turbo",
    detail: "High accuracy multilingual model with faster large-model decoding",
    sizeLabel: "1.5 GiB",
  },
];

const appBuildVersion = typeof packageMetadata.version === "string" ? packageMetadata.version : "1.0.0";

export const defaultAppInfo: AppInfo = {
  name: "ASR Pro",
  version: appBuildVersion,
};
