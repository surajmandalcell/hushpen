import { useState } from "react";
import { Sidebar } from "./components/layout/Sidebar";
import { Toolbar } from "./components/layout/Toolbar";
import { useScrollbarAutohide } from "./components/layout/useScrollbarAutohide";
import { useAudioInputDevices } from "./features/devices/useAudioInputDevices";
import { localStorageHistoryRepository, type HistoryRepository } from "./features/history/historyRepository";
import { useHistory } from "./features/history/useHistory";
import { useModelLibrary } from "./features/models/useModelLibrary";
import { useRecordingFlow } from "./features/recording/useRecordingFlow";
import { useTranscriber } from "./features/recording/useTranscriber";
import { useRuntimeBridge } from "./features/runtime/useRuntimeBridge";
import { useSettings } from "./features/settings/useSettings";
import { defaultAppInfo } from "./lib/defaults";
import type { AppInfo, ViewId, WindowAction } from "./types/app";
import type { RuntimeInfo } from "./types/runtime";
import type { ViewProps } from "./types/view";
import { findView } from "./views";

interface AppProps {
  historyRepository?: HistoryRepository;
}

function handleWindowAction(action: WindowAction) {
  void window.asrpro?.windowControl(action);
}

function App({ historyRepository = localStorageHistoryRepository }: AppProps) {
  const [activeView, setActiveView] = useState<ViewId>("home");
  const [runtimeInfo, setRuntimeInfo] = useState<RuntimeInfo | null>(null);
  const [appInfo, setAppInfo] = useState<AppInfo>(defaultAppInfo);
  const audio = useAudioInputDevices();
  const models = useModelLibrary({ runtimeInfo, setRuntimeInfo });
  const transcribe = useTranscriber(models.selectedModelId);
  const history = useHistory({ repository: historyRepository, selectedModel: models.selectedModel, transcribe });
  const settings = useSettings({ setRuntimeInfo });
  const recording = useRecordingFlow({
    selectedAudioInputId: audio.selectedDeviceId,
    selectedModel: models.selectedModel,
    autoCopyTranscripts: settings.autoCopyTranscripts,
    transcribe,
    addHistoryRow: history.addRow,
    setRuntimeInfo,
  });
  const { isScrollbarVisible, handleScrollActivity } = useScrollbarAutohide(activeView);
  useRuntimeBridge({
    setRuntimeInfo,
    setAppInfo,
    applyModelState: models.applyRuntimeState,
    applySettingsState: settings.applyRuntimeState,
    restoreRecordingState: recording.restoreFromRuntime,
    applyRecordingState: recording.applyBridgeState,
    updateModelDownloadProgress: models.updateDownloadProgress,
    clearModelDownloadProgress: models.clearDownloadProgress,
  });

  const activeDefinition = findView(activeView);
  const ActiveView = activeDefinition.component;
  const viewProps: ViewProps = {
    appInfo,
    runtimeInfo,
    recording,
    models,
    audio,
    settings,
    history,
    navigate: setActiveView,
  };

  return (
    <div className="app-chrome h-screen w-screen overflow-hidden bg-[#2f2f2f] font-[Inter,-apple-system,BlinkMacSystemFont,'SF_Pro_Text','Segoe_UI',sans-serif] text-[#ededed] antialiased">
      <div className="grid h-full grid-cols-1 grid-rows-[auto_minmax(0,1fr)] sm:grid-cols-[208px_minmax(0,1fr)] sm:grid-rows-1">
        <Sidebar activeView={activeView} onChange={setActiveView} onWindowAction={handleWindowAction} />
        <section className="grid min-h-0 min-w-0 grid-rows-[34px_minmax(0,1fr)] bg-[radial-gradient(circle_at_68%_10%,rgba(57,89,62,0.16),transparent_38%),#333333] sm:border-l sm:border-[#3f3f3f]">
          <Toolbar activeTitle={activeDefinition.label} audio={audio} />
          <main
            tabIndex={-1}
            className={`scrollbar-macos scrollbar-autohide min-h-0 min-w-0 overflow-y-auto px-3 pb-5 pt-3 outline-none focus:outline-none focus-visible:outline-none sm:px-4 ${isScrollbarVisible ? "is-scrollbar-visible" : ""}`}
            onScroll={handleScrollActivity}
            onTouchMove={handleScrollActivity}
            onWheel={handleScrollActivity}
          >
            <ActiveView {...viewProps} />
          </main>
        </section>
      </div>
    </div>
  );
}

export default App;
