import React from "react";
import {
  Settings,
  Brain,
  Mic,
  Keyboard,
  FileText,
  Info,
  Palette,
  Accessibility,
} from "lucide-react";
import MacTrafficLights from "./macos/MacTrafficLights";
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
  { id: "accessibility", label: "Accessibility", icon: Accessibility },
  { id: "demo", label: "macOS Components", icon: Palette },
  { id: "about", label: "About", icon: Info },
];

const Sidebar: React.FC<SidebarProps> = ({
  activeSection,
  onSectionChange,
}) => {
  return (
    <div className="sidebar">
      <div className="sidebar-header">
        <MacTrafficLights />
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
