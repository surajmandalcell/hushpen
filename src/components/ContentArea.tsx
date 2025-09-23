import React from "react";
import "./ContentArea.css";

// Pages
import GeneralPage from "../pages/GeneralPage";
import ModelsPage from "../pages/ModelsPage";
import MicrophonePage from "../pages/MicrophonePage";
import KeyboardPage from "../pages/KeyboardPage";
import TranscribePage from "../pages/TranscribePage";
import AboutPage from "../pages/AboutPage";
import ComponentDemo from "../pages/ComponentDemo";
import AccessibilityPage from "../pages/AccessibilityPage";

interface ContentAreaProps {
  activeSection: string;
  onStartRecording?: () => void;
}

const ContentArea: React.FC<ContentAreaProps> = ({
  activeSection,
  onStartRecording,
}) => {
  const renderPage = () => {
    switch (activeSection) {
      case "general":
        return <GeneralPage />;
      case "models":
        return <ModelsPage />;
      case "microphone":
        return <MicrophonePage />;
      case "keyboard":
        return <KeyboardPage />;
      case "transcribe":
        return <TranscribePage />;
      case "accessibility":
        return <AccessibilityPage />;
      case "about":
        return <AboutPage />;
      case "demo":
        return <ComponentDemo />;
      default:
        return <GeneralPage />;
    }
  };

  return (
    <div className="content-area">
      <div className="content-container">
        {onStartRecording && (
          <div
            style={{
              padding: "20px",
              borderBottom: "1px solid #3e3e42",
              marginBottom: "20px",
            }}
          >
            <button
              onClick={onStartRecording}
              style={{
                padding: "10px 20px",
                background: "#007acc",
                color: "white",
                border: "none",
                borderRadius: "4px",
                cursor: "pointer",
              }}
            >
              🎤 Test Recording (Space to start/stop, Esc to cancel)
            </button>
          </div>
        )}
        {renderPage()}
      </div>
    </div>
  );
};

export default ContentArea;
