export interface ElectronAPI {
  // Window controls
  minimizeWindow: () => Promise<void>;
  maximizeWindow: () => Promise<void>;
  closeWindow: () => Promise<void>;
  isWindowMaximized: () => Promise<boolean>;
  
  // App controls
  quitApp: () => Promise<void>;
  
  // System tray
  showTrayNotification: (title: string, body: string) => Promise<void>;
  
  // File system operations
  openFile: () => Promise<string | null>;
  saveFile: (content: string, filename: string) => Promise<boolean>;
  
  // Audio recording
  startRecording: () => Promise<void>;
  stopRecording: () => Promise<string | null>; // Returns the path to the recorded file
  getRecordingDevices: () => Promise<MediaDeviceInfo[]>;
  
  // Settings
  getSettings: () => Promise<any>;
  saveSettings: (settings: any) => Promise<void>;
  
  // Events from main to renderer
  onWindowStateChanged: (callback: (isMaximized: boolean) => void) => void;
  onRecordingStateChanged: (callback: (isRecording: boolean) => void) => void;
  onNotificationReceived: (callback: (notification: any) => void) => void;
  
  // Remove listeners
  removeAllListeners: (channel: string) => void;
}

declare global {
  interface Window {
    electronAPI: ElectronAPI;
  }
}