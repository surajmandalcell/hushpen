// Settings service for ASR Pro
export interface AppSettings {
  // General settings
  launchAtLogin: boolean;
  startMinimized: boolean;
  autoUnloadModel: boolean;
  autoUnloadTimeout: number; // in minutes
  
  // Model settings
  currentModel: string;
  language: string;
  processingDevice: string;
  realTimeProcessing: boolean;
  modelStoragePath: string;
  
  // Microphone settings
  audioInputDevice: string;
  sampleRate: number;
  channels: number;
  audioQuality: string;
  
  // Keyboard settings
  globalHotkey: string;
  enableOverlay: boolean;
  autoPaste: boolean;
  
  // UI settings
  theme: 'dark'; // Only dark theme supported
  windowOpacity: number;
  alwaysOnTop: boolean;
}

const DEFAULT_SETTINGS: AppSettings = {
  // General settings
  launchAtLogin: false,
  startMinimized: false,
  autoUnloadModel: true,
  autoUnloadTimeout: 30,
  
  // Model settings
  currentModel: '',
  language: 'auto',
  processingDevice: 'cpu',
  realTimeProcessing: false,
  modelStoragePath: '',
  
  // Microphone settings
  audioInputDevice: '',
  sampleRate: 44100,
  channels: 1,
  audioQuality: 'high',
  
  // Keyboard settings
  globalHotkey: 'CmdOrCtrl+Shift+Space',
  enableOverlay: true,
  autoPaste: true,
  
  // UI settings
  theme: 'dark',
  windowOpacity: 0.95,
  alwaysOnTop: false,
};

const STORAGE_KEY = 'asrpro-settings';

class SettingsService {
  private settings: AppSettings = { ...DEFAULT_SETTINGS };

  constructor() {
    this.loadSettings();
  }

  /**
   * Load settings from localStorage
   */
  private loadSettings(): void {
    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (stored) {
        const parsedSettings = JSON.parse(stored);
        this.settings = { ...DEFAULT_SETTINGS, ...parsedSettings };
      }
    } catch (error) {
      console.error('Failed to load settings:', error);
      this.settings = { ...DEFAULT_SETTINGS };
    }
  }

  /**
   * Save settings to localStorage
   */
  private saveSettings(): void {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(this.settings));
    } catch (error) {
      console.error('Failed to save settings:', error);
    }
  }

  /**
   * Get all settings
   */
  getSettings(): AppSettings {
    return { ...this.settings };
  }

  /**
   * Get a specific setting
   */
  getSetting<K extends keyof AppSettings>(key: K): AppSettings[K] {
    return this.settings[key];
  }

  /**
   * Update a specific setting
   */
  updateSetting<K extends keyof AppSettings>(key: K, value: AppSettings[K]): void {
    this.settings[key] = value;
    this.saveSettings();
  }

  /**
   * Update multiple settings at once
   */
  updateSettings(updates: Partial<AppSettings>): void {
    this.settings = { ...this.settings, ...updates };
    this.saveSettings();
  }

  /**
   * Reset settings to defaults
   */
  resetSettings(): void {
    this.settings = { ...DEFAULT_SETTINGS };
    this.saveSettings();
  }

  /**
   * Export settings as JSON
   */
  exportSettings(): string {
    return JSON.stringify(this.settings, null, 2);
  }

  /**
   * Import settings from JSON
   */
  importSettings(jsonString: string): boolean {
    try {
      const imported = JSON.parse(jsonString);
      this.settings = { ...DEFAULT_SETTINGS, ...imported };
      this.saveSettings();
      return true;
    } catch (error) {
      console.error('Failed to import settings:', error);
      return false;
    }
  }
}

// Export singleton instance
export const settingsService = new SettingsService();

// Export hook for React components
export const useSettings = () => {
  const [settings, setSettings] = React.useState<AppSettings>(settingsService.getSettings());

  const updateSetting = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
    settingsService.updateSetting(key, value);
    setSettings(settingsService.getSettings());
  };

  const updateSettings = (updates: Partial<AppSettings>) => {
    settingsService.updateSettings(updates);
    setSettings(settingsService.getSettings());
  };

  const resetSettings = () => {
    settingsService.resetSettings();
    setSettings(settingsService.getSettings());
  };

  return {
    settings,
    updateSetting,
    updateSettings,
    resetSettings,
  };
};

// Import React for the hook
import React from 'react';
