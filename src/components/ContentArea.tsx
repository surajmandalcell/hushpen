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

interface ContentAreaProps {
  activeSection: string;
}

const ContentArea: React.FC<ContentAreaProps> = ({ activeSection }) => {
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
      <div className="content-container">{renderPage()}</div>
    </div>
  );
};

export default ContentArea;
