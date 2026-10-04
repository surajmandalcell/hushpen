import type { AudioInputDevices } from "../features/devices/useAudioInputDevices";
import type { HistoryState } from "../features/history/useHistory";
import type { ModelLibrary } from "../features/models/useModelLibrary";
import type { RecordingFlow } from "../features/recording/useRecordingFlow";
import type { Settings } from "../features/settings/useSettings";
import type { AppInfo, ViewId } from "./app";
import type { RuntimeInfo } from "./runtime";

export interface ViewProps {
  appInfo: AppInfo;
  runtimeInfo: RuntimeInfo | null;
  recording: RecordingFlow;
  models: ModelLibrary;
  audio: AudioInputDevices;
  settings: Settings;
  history: HistoryState;
  navigate: (view: ViewId) => void;
}
