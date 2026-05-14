import React from 'react';
import {
  SpokenlyIcon,
  GeneralSettingsIcon,
  DictationModelsIcon,
  TranscribeFileIcon,
  HistoryIcon,
  KeyboardControlsIcon,
  AiPromptsIcon,
  AboutIcon
} from './icons';

interface WindowProps {
  children: React.ReactNode;
  activeView: string;
  setActiveView: (view: string) => void;
}

export const SpokenlyWindow: React.FC<WindowProps> = ({ children, activeView, setActiveView }) => {
  const sidebarItems = [
    { id: 'general', label: 'General Settings', icon: <GeneralSettingsIcon /> },
    { id: 'dictation', label: 'Dictation Models', icon: <DictationModelsIcon /> },
    { id: 'transcribe', label: 'Transcribe File', icon: <TranscribeFileIcon /> },
    { id: 'history', label: 'History', icon: <HistoryIcon /> },
    { id: 'keyboard', label: 'Keyboard Controls', icon: <KeyboardControlsIcon /> },
    { id: 'ai', label: 'AI Prompts', icon: <AiPromptsIcon /> },
    { id: 'about', label: 'About', icon: <AboutIcon /> }
  ];

  return (
    <div className="w-[960px] h-[640px] bg-[#292a2d] rounded-xl shadow-2xl shadow-black/50 overflow-hidden flex font-sans relative">
      {/* macOS Traffic Lights */}
      <div className="absolute top-3.5 left-4 flex space-x-2 z-10">
        <div className="group w-3 h-3 rounded-full bg-[#ff5f57] flex items-center justify-center">
          <div className="w-1.5 h-1.5 rounded-full bg-black/50 opacity-0 group-hover:opacity-100 transition-opacity"></div>
        </div>
        <div className="group w-3 h-3 rounded-full bg-[#febb2e] flex items-center justify-center">
          <div className="w-1.5 h-1.5 rounded-full bg-black/50 opacity-0 group-hover:opacity-100 transition-opacity"></div>
        </div>
        <div className="group w-3 h-3 rounded-full bg-[#28c840] flex items-center justify-center">
          <div className="w-1.5 h-1.5 rounded-full bg-black/50 opacity-0 group-hover:opacity-100 transition-opacity"></div>
        </div>
      </div>

      {/* Sidebar */}
      <div className="w-56 flex-shrink-0 p-3 pt-12 flex flex-col justify-between">
        <div>
          {/* Spokenly Logo */}
          <div className="flex items-center gap-2 px-0 pb-4">
            <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-blue-500 to-purple-600 flex items-center justify-center text-white">
              <SpokenlyIcon />
            </div>
            <span className="font-semibold text-white">Spokenly</span>
          </div>

          {/* Navigation Items */}
          <div className="space-y-1">
            {sidebarItems.map(item => (
              <button
                key={item.id}
                onClick={() => setActiveView(item.id)}
                className={`w-full flex items-center gap-2.5 px-2 py-1.5 text-sm rounded-md transition-colors ${
                  activeView === item.id
                    ? 'bg-blue-600 text-white'
                    : 'text-gray-300 hover:bg-gray-700/50'
                }`}
              >
                <span className="w-4 h-4">{item.icon}</span>
                <span>{item.label}</span>
              </button>
            ))}
          </div>
        </div>

        {/* Usage Bar at Bottom */}
        <div className="p-2 border-t border-gray-700/50">
          <p className="text-xs text-gray-400">Free usage left: 100%</p>
          <div className="h-1.5 w-full bg-gray-700 rounded-full mt-1.5">
            <div className="h-1.5 bg-blue-500 rounded-full" style={{width: '100%'}}></div>
          </div>
          <button className="text-xs text-blue-400 hover:underline mt-2">Upgrade to Pro</button>
          <p className="text-xs text-gray-500 mt-2">v2.13.3 (259)</p>
        </div>
      </div>

      {/* Main Content Area */}
      <div className="flex-grow bg-[#313235] overflow-y-auto px-6 pt-12 pb-6">
        <div>
          {children}
        </div>
      </div>
    </div>
  );
};