import React from "react";
import {
  Settings,
  Brain,
  Mic,
  Keyboard,
  FileText,
  Info,
  Accessibility,
} from "lucide-react";
import { PalSidebar, PalSidebarItem, PalSidebarSection, PalHeader, PalHeaderTitle } from "./palantirui";
import { TacticalWindowControls } from "./TacticalWindowControls";

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
        className="px-4 py-3 border-b border-palantir-zinc-200 dark:border-palantir-zinc-700"
      >
        <div className="flex items-center justify-between w-full">
          <TacticalWindowControls />
          <PalHeaderTitle title="ASR Pro" />
        </div>
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
