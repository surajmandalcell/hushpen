import { app, BrowserWindow, Menu, Tray, nativeImage, ipcMain } from 'electron';
import { join } from 'path';
import { electronApp, optimizer, is } from '@electron-toolkit/utils';

// Create icon from file system
const icon = nativeImage.createFromPath(join(__dirname, '../../resources/icon.png'));

// Import backend server - this will start the server automatically
import './backend/server';

function createWindow(): void {
  // Create the browser window
  const mainWindow = new BrowserWindow({
    width: 800,
    height: 600,
    minWidth: 600,
    minHeight: 400,
    show: false,
    autoHideMenuBar: true,
    frame: false,
    transparent: true,
    titleBarStyle: 'hiddenInset',
    ...(process.platform === 'linux' ? { icon } : {}),
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      sandbox: false,
      contextIsolation: true,
      nodeIntegration: false
    }
  });

  mainWindow.on('ready-to-show', () => {
    mainWindow.show();
  });

  mainWindow.webContents.setWindowOpenHandler((details) => {
    const { shell } = require('electron');
    shell.openExternal(details.url);
    return { action: 'deny' };
  });

  // Load the app
  if (is.dev && process.env['ELECTRON_RENDERER_URL']) {
    mainWindow.loadURL(process.env['ELECTRON_RENDERER_URL']);
  } else {
    mainWindow.loadFile(join(__dirname, '../renderer/index.html'));
  }

  // Set up system tray (will be implemented when migrating backend)
  // setupTray(mainWindow);
}

// This method will be called when Electron has finished initialization
app.whenReady().then(() => {
  // Set app user model id for windows
  electronApp.setAppUserModelId('com.asrpro.app');

  // Default open or close DevTools by F12 in development
  app.on('browser-window-created', (_, window) => {
    optimizer.watchWindowShortcuts(window);
  });

  createWindow();

  app.on('activate', function () {
    // On macOS it's common to re-create a window in the app when the
    // dock icon is clicked and there are no other windows open.
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
  
  // Set up IPC handlers
  setupIpcHandlers();
});

// Quit when all windows are closed
app.on('window-all-closed', () => {
  // On macOS it is common for applications and their menu bar
  // to stay active until the user quits explicitly with Cmd + Q
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

// Set up IPC handlers for communication with renderer process
function setupIpcHandlers() {
  // Window controls
  ipcMain.handle('window:minimize', () => {
    const mainWindow = BrowserWindow.getFocusedWindow();
    if (mainWindow) mainWindow.minimize();
  });
  
  ipcMain.handle('window:maximize', () => {
    const mainWindow = BrowserWindow.getFocusedWindow();
    if (mainWindow) {
      if (mainWindow.isMaximized()) {
        mainWindow.unmaximize();
      } else {
        mainWindow.maximize();
      }
    }
  });
  
  ipcMain.handle('window:close', () => {
    const mainWindow = BrowserWindow.getFocusedWindow();
    if (mainWindow) mainWindow.close();
  });
  
  ipcMain.handle('window:isMaximized', () => {
    const mainWindow = BrowserWindow.getFocusedWindow();
    return mainWindow ? mainWindow.isMaximized() : false;
  });
  
  // App controls
  ipcMain.handle('app:quit', () => {
    app.quit();
  });
  
  // File system operations
  ipcMain.handle('file:open', async () => {
    const { dialog } = require('electron');
    const result = await dialog.showOpenDialog({
      properties: ['openFile'],
      filters: [
        { name: 'Audio Files', extensions: ['mp3', 'wav', 'm4a', 'flac', 'ogg'] },
        { name: 'All Files', extensions: ['*'] }
      ]
    });
    
    return result.canceled ? null : result.filePaths[0];
  });
  
  ipcMain.handle('file:save', async (event, content: string, filename: string) => {
    const { dialog } = require('electron');
    const { writeFile } = require('fs').promises;
    const { join } = require('path');
    
    const result = await dialog.showSaveDialog({
      defaultPath: filename,
      filters: [
        { name: 'Text Files', extensions: ['txt'] },
        { name: 'All Files', extensions: ['*'] }
      ]
    });
    
    if (!result.canceled && result.filePath) {
      await writeFile(result.filePath, content, 'utf8');
      return true;
    }
    return false;
  });
  
  // Audio recording (simplified implementation)
  ipcMain.handle('audio:startRecording', async () => {
    // This would be implemented with a proper audio recording library
    console.log('Start recording requested');
  });
  
  ipcMain.handle('audio:stopRecording', async () => {
    // This would be implemented with a proper audio recording library
    console.log('Stop recording requested');
    return null; // Return path to recorded file
  });
  
  ipcMain.handle('audio:getDevices', async () => {
    // This would enumerate audio devices
    return []; // Return empty array for now
  });
  
  // Settings
  ipcMain.handle('settings:get', async () => {
    // This would load settings from a config file
    return {};
  });
  
  ipcMain.handle('settings:save', async (event, settings: any) => {
    // This would save settings to a config file
    console.log('Save settings:', settings);
  });
  
  // System tray notifications
  ipcMain.handle('tray:showNotification', async (event, title: string, body: string) => {
    const { Notification } = require('electron');
    if (Notification.isSupported()) {
      const notification = new Notification({
        title,
        body,
        icon: join(__dirname, '../../resources/icon.png')
      });
      notification.show();
    }
  });
}

// In this file you can include the rest of your app's main process code