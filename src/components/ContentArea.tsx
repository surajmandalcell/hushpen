import React from "react";

// Pages
import GeneralPage from "../pages/GeneralPage";
import ModelsPage from "../pages/ModelsPage";
import MicrophonePage from "../pages/MicrophonePage";
import KeyboardPage from "../pages/KeyboardPage";
import TranscribePage from "../pages/TranscribePage";
import AboutPage from "../pages/AboutPage";
import AccessibilityPage from "../pages/AccessibilityPage";

// PalantirUI Components
import { PalPanel, PalPanelContent } from "./palantirui";

interface ContentAreaProps {
  activeSection: string;
}

const ContentArea: React.FC<ContentAreaProps> = ({
  activeSection,
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
      default:
        return <GeneralPage />;
    }
  };

  return (
    <PalPanel
      className="flex-1 h-full pal-p-lg"
      variant="default"
      padding="none"
      withGlow={false}
      withCornerMarkers={false}
    >
      <PalPanelContent className="overflow-y-auto pal-container">
        <div className="pal-section animate-fade-in">
          {renderPage()}
        </div>
      </PalPanelContent>
    </PalPanel>
  );
};

export default ContentArea;
