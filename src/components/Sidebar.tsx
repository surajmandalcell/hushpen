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
    <div className="sidebar-macos w-64 h-full flex flex-col">
      <div className="flex flex-col items-center py-4 border-b border-macos-border dark:border-macos-border-dark">
        <MacTrafficLights />
        <h2 className="text-lg font-semibold text-macos-text dark:text-macos-text-dark mt-2">
          ASR Pro
        </h2>
      </div>

      <nav className="flex-1 px-3 py-4 space-y-1 scrollbar-macos overflow-y-auto">
        {sidebarSections.map((section) => {
          const IconComponent = section.icon;
          return (
            <button
              key={section.id}
              className={`sidebar-item w-full ${
                activeSection === section.id ? "active" : ""
              }`}
              onClick={() => onSectionChange(section.id)}
            >
              <IconComponent size={18} className="mr-3 flex-shrink-0" />
              <span className="truncate">{section.label}</span>
            </button>
          );
        })}
      </nav>
    </div>
  );
};

export default Sidebar;
