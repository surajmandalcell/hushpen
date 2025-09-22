import React from "react";
import {
  Settings,
  Brain,
  Mic,
  Keyboard,
  FileText,
  Info,
  X,
  Minus,
} from "lucide-react";
import "./Sidebar.css";

interface SidebarProps {
  activeSection: string;
  onSectionChange: (section: string) => void;
}

const sidebarSections = [
  { id: "general", label: "General", icon: Settings },
  { id: "models", label: "Models", icon: Brain },
  { id: "microphone", label: "Microphone", icon: Mic },
  { id: "keyboard", label: "Keyboard", icon: Keyboard },
  { id: "transcribe", label: "Transcribe Files", icon: FileText },
  { id: "about", label: "About", icon: Info },
];

const Sidebar: React.FC<SidebarProps> = ({
  activeSection,
  onSectionChange,
}) => {
  const handleClose = async () => {
    // Use Tauri API to close the window
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().close();
    } catch (error) {
      console.error("Failed to close window:", error);
    }
  };

  const handleMinimize = async () => {
    // Use Tauri API to minimize the window
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().minimize();
    } catch (error) {
      console.error("Failed to minimize window:", error);
    }
  };

  return (
    <div className="sidebar">
      <div className="sidebar-header">
        <div className="window-controls">
          <button className="window-control minimize" onClick={handleMinimize}>
            <Minus size={14} />
          </button>
          <button className="window-control close" onClick={handleClose}>
            <X size={14} />
          </button>
        </div>
        <h2 className="app-title">ASR Pro</h2>
      </div>

      <nav className="sidebar-nav">
        {sidebarSections.map((section) => {
          const IconComponent = section.icon;
          return (
            <button
              key={section.id}
              className={`sidebar-item ${
                activeSection === section.id ? "active" : ""
              }`}
              onClick={() => onSectionChange(section.id)}
            >
              <IconComponent size={18} className="sidebar-icon" />
              <span className="sidebar-label">{section.label}</span>
            </button>
          );
        })}
      </nav>
    </div>
  );
};

export default Sidebar;
