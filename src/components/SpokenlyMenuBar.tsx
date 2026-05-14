import React, { useState, useEffect, useRef } from 'react';
import { AppleIcon, SpokenlyMenuBarIcon, ControlCenterIcon, WifiIcon, BatteryIcon, CheckIcon } from './icons';

interface MenuDropdownProps {
  items: Array<{label?: string; shortcut?: string; type?: string}>;
  closeMenu: () => void;
}

const MenuDropdown: React.FC<MenuDropdownProps> = ({ items, closeMenu }) => {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        closeMenu();
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [closeMenu]);

  return (
    <div ref={menuRef} className="absolute top-full left-0 mt-1 w-56 bg-black/50 backdrop-blur-2xl rounded-lg p-1.5 shadow-2xl border border-white/10 text-white text-sm z-50">
      {items.map((item, index) => {
        if (item.type === 'divider') {
          return <div key={index} className="h-[1px] bg-white/10 my-1 mx-1.5"></div>;
        }
        return (
          <div key={index} className="flex items-center justify-between px-2.5 py-1 rounded hover:bg-blue-600 cursor-default">
            <span>{item.label}</span>
            {item.shortcut && <span className="text-gray-400">{item.shortcut}</span>}
          </div>
        );
      })}
    </div>
  );
};

interface IconContextMenuProps {
  x: number;
  y: number;
  isVisible: boolean;
  closeMenu: () => void;
}

export const IconContextMenu: React.FC<IconContextMenuProps> = ({ x, y, isVisible, closeMenu }) => {
  if (!isVisible) return null;
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        closeMenu();
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [closeMenu]);

  const menuItems = [
    { label: 'Whisper Tiny (English)', checked: true, shortcut: '⌘1' },
    { label: 'Whisper Large V3 Turbo (Quantized)', shortcut: '⌘2' },
    { label: 'NVIDIA Parakeet TDT 0.6B V3', shortcut: '⌘3' },
    { label: 'Distil-Whisper Small (English Only)', shortcut: '⌘4' },
    { label: 'Distil-Whisper Medium (English Only)', shortcut: '⌘5' },
    { label: 'Soniox Realtime', shortcut: '⌘6' },
    { type: 'divider' },
    { label: 'Show all models', shortcut: '⌘O' },
  ];

  return (
    <div
      ref={menuRef}
      style={{ top: y, left: x }}
      className="absolute z-50 w-72 bg-black/50 backdrop-blur-2xl rounded-lg p-1.5 shadow-2xl border border-white/10 text-white text-sm transform -translate-x-full"
    >
      {menuItems.map((item, index) => {
        if (item.type === 'divider') {
          return <div key={index} className="h-[1px] bg-white/10 my-1 mx-1.5"></div>;
        }
        return (
          <div key={index} className="flex items-center justify-between px-2.5 py-1 rounded hover:bg-blue-600 cursor-default">
            <div className="flex items-center gap-2">
              {item.checked && <CheckIcon />}
              <span className={!item.checked ? 'ml-6' : ''}>{item.label}</span>
            </div>
            {item.shortcut && <span className="text-gray-400">{item.shortcut}</span>}
          </div>
        );
      })}
    </div>
  );
};

interface MenuBarProps {
  onSpokenlyIconClick: (event: React.MouseEvent) => void;
}

export const SpokenlyMenuBar: React.FC<MenuBarProps> = ({ onSpokenlyIconClick }) => {
  const [currentTime, setCurrentTime] = useState('');
  const [openMenu, setOpenMenu] = useState<string | null>(null);

  useEffect(() => {
    const timer = setInterval(() => {
      setCurrentTime(new Date().toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' }));
    }, 1000);
    return () => clearInterval(timer);
  }, []);

  const menus = {
    spokenly: [
      { label: 'About Spokenly' },
      { type: 'divider' },
      { label: 'Preferences...', shortcut: '⌘,' },
      { type: 'divider' },
      { label: 'Quit Spokenly', shortcut: '⌘Q' }
    ],
    file: [
      { label: 'New Transcription', shortcut: '⌘N' },
      { label: 'Open File...', shortcut: '⌘O' }
    ],
    edit: [
      { label: 'Undo', shortcut: '⌘Z' },
      { label: 'Redo', shortcut: '⇧⌘Z' },
      { type: 'divider' },
      { label: 'Cut', shortcut: '⌘X' },
      { label: 'Copy', shortcut: '⌘C' },
      { label: 'Paste', shortcut: '⌘V' }
    ],
  };

  const toggleMenu = (menu: string) => {
    setOpenMenu(openMenu === menu ? null : menu);
  };

  const closeMenu = () => {
    setOpenMenu(null);
  };

  return (
    <div className="absolute top-0 left-0 right-0 h-7 bg-black/20 backdrop-blur-xl text-white text-sm flex items-center justify-between px-3 z-40">
      <div className="flex items-center h-full">
        <div className="mr-4"><AppleIcon /></div>
        <div className="relative h-full flex items-center">
          <button onClick={() => toggleMenu('spokenly')} className={`font-semibold px-2 h-full rounded-md ${openMenu === 'spokenly' ? 'bg-white/20' : ''}`}>
            Spokenly
          </button>
          {openMenu === 'spokenly' && <MenuDropdown items={menus.spokenly} closeMenu={closeMenu} />}
        </div>
        <div className="relative h-full flex items-center">
          <button onClick={() => toggleMenu('file')} className={`px-2 h-full rounded-md ${openMenu === 'file' ? 'bg-white/20' : ''}`}>
            File
          </button>
          {openMenu === 'file' && <MenuDropdown items={menus.file} closeMenu={closeMenu} />}
        </div>
        <div className="relative h-full flex items-center">
          <button onClick={() => toggleMenu('edit')} className={`px-2 h-full rounded-md ${openMenu === 'edit' ? 'bg-white/20' : ''}`}>
            Edit
          </button>
          {openMenu === 'edit' && <MenuDropdown items={menus.edit} closeMenu={closeMenu} />}
        </div>
      </div>
      <div className="flex items-center gap-4">
        <button onClick={onSpokenlyIconClick} className="w-6 h-6 flex items-center justify-center rounded hover:bg-white/20">
          <SpokenlyMenuBarIcon />
        </button>
        <ControlCenterIcon />
        <WifiIcon />
        <BatteryIcon />
        <span>{currentTime}</span>
      </div>
    </div>
  );
};