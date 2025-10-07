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
import { PalSidebar, PalSidebarItem, PalSidebarSection, PalHeader, PalHeaderTitle } from "./palantirui";

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
  { id: "palantirui", label: "PalantirUI Demo", icon: Palette },
  { id: "about", label: "About", icon: Info },
];

const Sidebar: React.FC<SidebarProps> = ({
  activeSection,
  onSectionChange,
}) => {
  return (
    <PalSidebar
      width="md"
      withGlow={true}
      withCornerMarkers={true}
      className="border-r border-palantir-zinc-200 dark:border-palantir-zinc-700"
    >
      <PalHeader
        height="md"
        withWindowControls={true}
        className="px-4 py-3 border-b border-palantir-zinc-200 dark:border-palantir-zinc-700"
      >
        <PalHeaderTitle title="ASR Pro" />
      </PalHeader>
      
      <PalSidebarSection>
        {sidebarSections.map((section) => {
          const IconComponent = section.icon;
          return (
            <PalSidebarItem
              key={section.id}
              active={activeSection === section.id}
              icon={<IconComponent size={18} />}
              label={section.label}
              onClick={() => {
                console.log(`Sidebar navigation: clicked on ${section.id}`);
                onSectionChange(section.id);
              }}
              className="mx-2"
            />
          );
        })}
      </PalSidebarSection>
    </PalSidebar>
  );
};

export default Sidebar;
