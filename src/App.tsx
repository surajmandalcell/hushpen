import { useState } from "react";
import "./App.css";

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

function App() {
  const [activeSection, setActiveSection] = useState("general");
  const [isRecordingOverlayActive, setIsRecordingOverlayActive] =
    useState(false);
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

  return (
    <div className="app-container">
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
