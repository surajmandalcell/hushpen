import React, { useState } from 'react';
import { ToggleSwitch, SettingsSection, SettingsRow, DraggableListItem, ContentHeader } from '../components/SpokenlyUI';
import { ChevronRightIcon } from '../components/icons';

export const GeneralPreferences: React.FC = () => {
  const [toggles, setToggles] = useState({
    launchAtLogin: true,
    showInDock: false,
    showInStatusBar: true,
    playSoundEffects: true,
    muteWhileRecording: false,
    enableTrackpadFeedback: true,
    autoCopy: false
  });

  const [microphones, setMicrophones] = useState([
    { id: 1, name: 'Arctis Nova 7' },
    { id: 2, name: 'Macbook Air Microphone' },
    { id: 3, name: "Suraj's AirPods" },
    { id: 4, name: 'Microsoft Teams Audio' },
    { id: 5, name: 'DECO954 Microphone' }
  ]);

  const handleToggle = (key: string) => {
    setToggles(prev => ({ ...prev, [key]: !prev[key as keyof typeof prev] }));
  };

  const moveMicrophone = (dragIndex: number, hoverIndex: number) => {
    // Drag functionality would be implemented here
  };

  return (
    <div>
      <ContentHeader
        title="General Preferences"
      />
      <div className="divide-y divide-gray-700">
        <SettingsSection title="Behavior">
          <SettingsRow label="Launch at login">
            <ToggleSwitch checked={toggles.launchAtLogin} onChange={() => handleToggle('launchAtLogin')} />
          </SettingsRow>
          <SettingsRow label="Show in Dock">
            <ToggleSwitch checked={toggles.showInDock} onChange={() => handleToggle('showInDock')} />
          </SettingsRow>
          <SettingsRow label="Show in Status Bar">
            <ToggleSwitch checked={toggles.showInStatusBar} onChange={() => handleToggle('showInStatusBar')} />
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="App Interface Language">
          <SettingsRow label="English">
            <button className="text-sm text-gray-300 bg-[#3a3b3d] border border-gray-600 rounded-md px-3 py-1 flex items-center">
              English <ChevronRightIcon />
            </button>
          </SettingsRow>
        </SettingsSection>

        <SettingsSection
          title="Microphone Priority Settings"
          description="Microphones are tried in priority order. Drag to reorder."
        >
          <div className="space-y-2">
            {microphones.map((mic, index) => (
              <DraggableListItem
                key={mic.id}
                id={mic.id}
                text={mic.name}
                index={index}
                moveItem={moveMicrophone}
                isPrimary={index === 1}
              />
            ))}
          </div>
        </SettingsSection>

        <SettingsSection title="Audio & Feedback">
          <SettingsRow label="Play sound effects">
            <ToggleSwitch checked={toggles.playSoundEffects} onChange={() => handleToggle('playSoundEffects')} />
          </SettingsRow>
          <SettingsRow label="Mute while recording">
            <ToggleSwitch checked={toggles.muteWhileRecording} onChange={() => handleToggle('muteWhileRecording')} />
          </SettingsRow>
          <SettingsRow label="Enable trackpad feedback">
            <ToggleSwitch checked={toggles.enableTrackpadFeedback} onChange={() => handleToggle('enableTrackpadFeedback')} />
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="Text Handling">
          <SettingsRow label="Automatically copy dictated text to clipboard">
            <ToggleSwitch checked={toggles.autoCopy} onChange={() => handleToggle('autoCopy')} />
          </SettingsRow>
          <SettingsRow label="Text Input Method">
            <button className="text-sm text-gray-300 bg-[#3a3b3d] border border-gray-600 rounded-md px-3 py-1 flex items-center">
              Paste (CMD+V) <ChevronRightIcon />
            </button>
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="Advanced">
          <button className="w-full text-left flex items-center justify-between p-2 hover:bg-gray-700/50 rounded-md">
            <span className="text-sm text-gray-200">Local Whisper Configuration</span>
            <ChevronRightIcon />
          </button>
          <button className="w-full text-left flex items-center justify-between p-2 hover:bg-gray-700/50 rounded-md">
            <span className="text-sm text-gray-200">Quick Commands (legacy - will be removed soon)</span>
            <ChevronRightIcon />
          </button>
        </SettingsSection>
      </div>
    </div>
  );
};
