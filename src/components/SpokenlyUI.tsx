import React, { useRef } from 'react';
import { HamburgerIcon, ChevronRightIcon } from './icons';

interface ToggleSwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
}

export const ToggleSwitch: React.FC<ToggleSwitchProps> = ({ checked, onChange }) => (
  <div
    onClick={() => onChange(!checked)}
    className={`relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out ${checked ? 'bg-blue-600' : 'bg-gray-600'}`}
  >
    <span className={`inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out ${checked ? 'translate-x-5' : 'translate-x-0'}`} />
  </div>
);

interface SettingsSectionProps {
  title: string;
  children: React.ReactNode;
  description?: string;
}

export const SettingsSection: React.FC<SettingsSectionProps> = ({ title, children, description }) => (
  <div className="py-3">
    <h3 className="text-sm font-semibold text-gray-200">{title}</h3>
    {description && <p className="text-xs text-gray-400 mt-1">{description}</p>}
    <div className="mt-4 space-y-3">{children}</div>
  </div>
);

interface SettingsRowProps {
  label: string;
  children: React.ReactNode;
  description?: string;
}

export const SettingsRow: React.FC<SettingsRowProps> = ({ label, children, description }) => (
  <div className="flex items-center justify-between">
    <div>
      <p className="text-sm text-gray-200">{label}</p>
      {description && <p className="text-xs text-gray-400">{description}</p>}
    </div>
    {children}
  </div>
);

interface ShortcutKeyProps {
  keys: string[];
}

export const ShortcutKey: React.FC<ShortcutKeyProps> = ({ keys }) => (
  <div className="flex items-center space-x-1 bg-[#232425] border border-gray-600 rounded-md px-2 py-1 text-sm">
    {keys.map((key, i) => (
      <span key={i} className="font-sans text-gray-300">{key}</span>
    ))}
  </div>
);

interface DraggableListItemProps {
  id: number;
  text: string;
  index: number;
  moveItem: (dragIndex: number, hoverIndex: number) => void;
  isPrimary?: boolean;
}

export const DraggableListItem: React.FC<DraggableListItemProps> = ({ id, text, index, moveItem, isPrimary }) => {
  const ref = useRef<HTMLDivElement>(null);

  return (
    <div ref={ref} className="flex items-center p-2 bg-[#3a3b3d] rounded-md border border-gray-600">
      <HamburgerIcon />
      <span className="ml-3 text-sm text-gray-200">{`${index + 1}. ${text}`}</span>
      {isPrimary && <span className="ml-2 w-2 h-2 bg-green-500 rounded-full"></span>}
    </div>
  );
};

interface ModalProps {
  isVisible: boolean;
  onClose: () => void;
  children: React.ReactNode;
}

export const Modal: React.FC<ModalProps> = ({ isVisible, onClose, children }) => {
  if (!isVisible) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="bg-[#2e2f31] border border-gray-700 rounded-xl shadow-2xl p-6 text-center text-white w-full max-w-md">
        {children}
      </div>
    </div>
  );
};

interface ContentHeaderProps {
  title: string;
  subtitle?: string;
}

export const ContentHeader: React.FC<ContentHeaderProps> = ({ title, subtitle }) => (
  <div className="mb-5">
    <h2 className="text-lg font-semibold text-white">{title}</h2>
    {subtitle && <p className="text-xs text-gray-400 mt-1">{subtitle}</p>}
  </div>
);