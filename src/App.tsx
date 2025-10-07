import { useState, useEffect, useId } from "react";
import { listen } from "@tauri-apps/api/event";
import { usePlatform } from "./services/platform";
import { useAccessibility } from "./services/accessibility";
// Removed CSS imports - now using Tailwind CSS

// Components
import Sidebar from "./components/Sidebar";
import ContentArea from "./components/ContentArea";
import SystemTray from "./components/SystemTray";
import ToastContainer from "./components/ToastContainer";
import RecordingOverlay from "./components/RecordingOverlay";
import TrayNotificationList from "./components/TrayNotificationList";

// PalantirUI Components
import { PalPanel, PalHeader, PalWindowControls } from "./components/palantirui";

// Hooks
import { useWindowState } from "./hooks/useWindowState";
import { useRecording } from "./services/recordingManager";
import { useTrayNotifications } from "./services/trayNotifications";
import { useWebSocket } from "./hooks/useWebSocket";
import { globalShortcutService } from "./services/globalShortcut";

function App() {
  const mainContentId = useId();
  const [activeSection, setActiveSection] = useState("general");
  const [isRecordingOverlayActive, setIsRecordingOverlayActive] =
    useState(false);
  const platformInfo = usePlatform();
  useAccessibility(); // Initialize accessibility features
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
    console.log("App: startRecording called, setting overlay active");
    setIsRecordingOverlayActive(true);
  };

  // Handler for section changes with logging
  const handleSectionChange = (section: string) => {
    console.log(`App: Section changing from ${activeSection} to ${section}`);
    setActiveSection(section);
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
    <div className={`h-screen w-screen overflow-hidden bg-palantir-zinc-50 dark:bg-palantir-zinc-900 transition-colors duration-200 ${platformInfo.getPlatformCSSClass()}`}>
      {/* Skip to content link for screen readers */}
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:absolute focus:top-2 focus:left-2 focus:z-50 px-4 py-2 bg-palantir-accent-blue text-white rounded-pal"
      >
        Skip to main content
      </a>

      <SystemTray />
      <ToastContainer />
      <RecordingOverlay
        isActive={isRecordingOverlayActive}
        isTranscribing={recordingState.isTranscribing}
        statusText={recordingState.statusText}
      />
      {notifications.length > 0 && <TrayNotificationList />}
      
      <div className="flex h-full" id={mainContentId}>
        <Sidebar
          activeSection={activeSection}
          onSectionChange={handleSectionChange}
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
