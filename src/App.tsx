import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { usePlatform } from "./services/platform";
import "./App.css";
import "./styles/platform.css";

// Components
import Sidebar from "./components/Sidebar";
import ContentArea from "./components/ContentArea";
import SystemTray from "./components/SystemTray";
import ToastContainer from "./components/ToastContainer";
import RecordingOverlay from "./components/RecordingOverlay";
import TrayNotificationList from "./components/TrayNotificationList";

// Hooks
import { useWindowState } from "./hooks/useWindowState";
import { useRecording } from "./services/recordingManager";
import { useTrayNotifications } from "./services/trayNotifications";
import { useWebSocket } from "./hooks/useWebSocket";
import { globalShortcutService } from "./services/globalShortcut";

function App() {
  const [activeSection, setActiveSection] = useState("general");
  const [isRecordingOverlayActive, setIsRecordingOverlayActive] =
    useState(false);
  const platformInfo = usePlatform();
  // Window state management is handled internally
  useWindowState();

  // Recording state management
  const { state: recordingState } = useRecording();

  // Tray notification state
  const { notifications } = useTrayNotifications();

  // WebSocket connection (for future use)
  useWebSocket();

  // Handler to start recording
  const startRecording = () => {
    setIsRecordingOverlayActive(true);
  };

  // Initialize global shortcuts and handle recording overlay activation
  useEffect(() => {
    const initializeShortcuts = async () => {
      try {
        await globalShortcutService.initialize();
        console.log("Global shortcuts initialized");
      } catch (error) {
        console.error("Failed to initialize global shortcuts:", error);
      }
    };

    initializeShortcuts();

    return () => {
      globalShortcutService.cleanup();
    };
  }, []);

  // Listen for global shortcut events to control recording overlay
  useEffect(() => {
    const unlistenStart = listen("recording-start", () => {
      setIsRecordingOverlayActive(true);
    });

    const unlistenStop = listen("recording-stop", () => {
      setIsRecordingOverlayActive(false);
    });

    return () => {
      unlistenStart.then((fn) => fn());
      unlistenStop.then((fn) => fn());
    };
  }, []);

  return (
    <div className={`app-container ${platformInfo.getPlatformCSSClass()}`}>
      <SystemTray />
      <ToastContainer />
      <RecordingOverlay
        isActive={isRecordingOverlayActive}
        isTranscribing={recordingState.isTranscribing}
        statusText={recordingState.statusText}
      />
      {notifications.length > 0 && <TrayNotificationList />}
      <div className="main-window">
        <Sidebar
          activeSection={activeSection}
          onSectionChange={setActiveSection}
        />
        <ContentArea
          activeSection={activeSection}
          onStartRecording={startRecording}
        />
      </div>
    </div>
  );
}

export default App;
