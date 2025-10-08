import { contextBridge, ipcRenderer } from 'electron';
import { ElectronAPI } from './types';

// Define the API that will be exposed to the renderer process
const electronAPI: ElectronAPI = {
  // Window controls
  minimizeWindow: () => ipcRenderer.invoke('window:minimize'),
  maximizeWindow: () => ipcRenderer.invoke('window:maximize'),
  closeWindow: () => ipcRenderer.invoke('window:close'),
  isWindowMaximized: () => ipcRenderer.invoke('window:isMaximized'),
  
  // App controls
  quitApp: () => ipcRenderer.invoke('app:quit'),
  
  // System tray
  showTrayNotification: (title: string, body: string) => 
    ipcRenderer.invoke('tray:showNotification', title, body),
  
  // File system operations
  openFile: () => ipcRenderer.invoke('file:open'),
  saveFile: (content: string, filename: string) =>
    ipcRenderer.invoke('file:save', content, filename),
  
  // Audio recording
  startRecording: () => ipcRenderer.invoke('audio:startRecording'),
  stopRecording: () => ipcRenderer.invoke('audio:stopRecording'),
  getRecordingDevices: () => ipcRenderer.invoke('audio:getDevices'),
  
  // Settings
  getSettings: () => ipcRenderer.invoke('settings:get'),
  saveSettings: (settings: any) => ipcRenderer.invoke('settings:save', settings),
  
  // Events from main to renderer
  onWindowStateChanged: (callback) => {
    ipcRenderer.on('window:stateChanged', (_, isMaximized) => callback(isMaximized));
  },
  onRecordingStateChanged: (callback) => {
    ipcRenderer.on('audio:recordingStateChanged', (_, isRecording) => callback(isRecording));
  },
  onNotificationReceived: (callback) => {
    ipcRenderer.on('notification:received', (_, notification) => callback(notification));
  },
  
  // Remove listeners
  removeAllListeners: (channel: string) => {
    ipcRenderer.removeAllListeners(channel);
  }
};

// Expose the API to the renderer process
contextBridge.exposeInMainWorld('electronAPI', electronAPI);