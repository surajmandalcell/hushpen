import React from "react";

// Pages
import GeneralPage from "../pages/GeneralPage";
import ModelsPage from "../pages/ModelsPage";
import MicrophonePage from "../pages/MicrophonePage";
import KeyboardPage from "../pages/KeyboardPage";
import TranscribePage from "../pages/TranscribePage";
import AboutPage from "../pages/AboutPage";
import TacticalDemo from "../pages/TacticalDemo";
import AccessibilityPage from "../pages/AccessibilityPage";
import PalantirUIDemo from "../pages/PalantirUIDemo";

// PalantirUI Components
import { PalPanel, PalPanelHeader, PalPanelContent, PalButton } from "./palantirui";

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
        return <TacticalDemo />;
      case "palantirui":
        return <PalantirUIDemo />;
      default:
        return <GeneralPage />;
    }
  };

  return (
    <PalPanel
      className="flex-1 h-full"
      variant="default"
      padding="md"
      withGlow={false}
      withCornerMarkers={false}
    >
      <PalPanelContent className="overflow-y-auto">
        {onStartRecording && (
          <div className="pb-4 mb-4 border-b border-palantir-zinc-200 dark:border-palantir-zinc-700">
            <PalButton
              variant="primary"
              onClick={() => {
                console.log("Recording button clicked in ContentArea");
                onStartRecording();
              }}
              withGlow={true}
              withCornerMarkers={true}
              className="flex items-center gap-2"
            >
              <span>🎤</span>
              <span>Test Recording (Space to start/stop, Esc to cancel)</span>
            </PalButton>
          </div>
        )}
        <div className="animate-fade-in">
          {renderPage()}
        </div>
      </PalPanelContent>
    </PalPanel>
  );
};

export default ContentArea;
