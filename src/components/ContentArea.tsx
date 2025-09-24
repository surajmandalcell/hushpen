import React from "react";

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
    <div className="flex-1 bg-macos-bg dark:bg-macos-bg-dark overflow-hidden">
      <div className="h-full p-6 scrollbar-macos overflow-y-auto">
        {onStartRecording && (
          <div className="p-5 border-b border-macos-border dark:border-macos-border-dark mb-5">
            <button
              onClick={onStartRecording}
              className="btn-macos flex items-center space-x-2"
            >
              <span>🎤</span>
              <span>Test Recording (Space to start/stop, Esc to cancel)</span>
            </button>
          </div>
        )}
        <div className="animate-fade-in">
          {renderPage()}
        </div>
      </div>
    </div>
  );
};

export default ContentArea;
