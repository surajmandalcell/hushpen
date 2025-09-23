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

function App() {
  const [activeSection, setActiveSection] = useState("general");
  // Window state management is handled internally
  useWindowState();

  // Recording state management
  const { state: recordingState, cancel, stop } = useRecording();

  // Tray notification state
  const { notifications } = useTrayNotifications();

  return (
    <div className="app-container">
      <SystemTray />
      <ToastContainer />
      <RecordingOverlay
        isActive={recordingState.isActive || recordingState.isTranscribing}
        isTranscribing={recordingState.isTranscribing}
        transcriptionProgress={recordingState.transcriptionProgress}
        onCancel={cancel}
        onStop={stop}
        statusText={recordingState.statusText}
        duration={recordingState.duration}
      />
      {notifications.length > 0 && <TrayNotificationList />}
      <div className="main-window">
        <Sidebar
          activeSection={activeSection}
          onSectionChange={setActiveSection}
        />
        <ContentArea activeSection={activeSection} />
      </div>
    </div>
  );
}

export default App;
