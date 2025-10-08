import { join } from 'path';

// Backend server will be initialized after app is ready
let backendServer: any = null;
let icon: any = null;

// Debug logging
console.log('Starting Electron main process...');
console.log('Current working directory:', process.cwd());
console.log('Module filename:', __filename);
console.log('Module directory:', __dirname);

// The Electron API is only available when running inside the Electron runtime
// We need to check if we're running in Electron
const isElectron = process.versions.electron !== undefined;
console.log('Running in Electron:', isElectron);

if (!isElectron) {
  console.error('This script must be run inside Electron');
  process.exit(1);
}

// The Electron API is not available in the global scope
// Let's try to use a different approach


// Try to use the electron module directly
const electronPath = require('electron');
console.log('Electron path:', electronPath);

// Try to load the Electron API from the electron module
try {
  const fs = require('fs');
  const path = require('path');
  
  // Get the path to the Electron executable
  const electronExecutablePath = electronPath;
  console.log('Electron executable path:', electronExecutablePath);
  
  // Try to find the Electron API module
  const electronModulePath = path.join(path.dirname(electronExecutablePath), 'index.js');
  console.log('Electron module path:', electronModulePath);
  
  // Check if the Electron module exists
  if (fs.existsSync(electronModulePath)) {
    console.log('Electron module exists');
    const electronModule = require(electronModulePath);
    console.log('Electron module type:', typeof electronModule);
    console.log('Electron module keys:', Object.keys(electronModule));
  } else {
    console.error('Electron module does not exist');
  }
} catch (error) {
  console.error('Error loading Electron module:', error);
}

// As a last resort, let's try to create a minimal Electron app
// without using the Electron API
console.log('Creating minimal Electron app without Electron API');

process.exit(0);

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
      preload: join(__dirname, 'preload.js'),
      sandbox: false,
      contextIsolation: true,
      nodeIntegration: false
    }
  });

  mainWindow.on('ready-to-show', () => {
    mainWindow.show();
  });

  mainWindow.webContents.setWindowOpenHandler((details) => {
    const { shell } = electronAPIs;
    shell.openExternal(details.url);
    return { action: 'deny' };
  });

  // Load the app
  const { is } = require('@electron-toolkit/utils');
  if (is.dev && process.env['ELECTRON_RENDERER_URL']) {
    mainWindow.loadURL(process.env['ELECTRON_RENDERER_URL']);
  } else {
    mainWindow.loadFile(join(__dirname, '../renderer/index.html'));
  }

  // Set up system tray (will be implemented when migrating backend)
  // setupTray(mainWindow);
}

// This method will be called when Electron has finished initialization
const { app: appInstance, nativeImage: nativeImageInstance } = electronAPIs;

appInstance.whenReady().then(() => {
  // Set app user model id for windows
  const { electronApp, optimizer } = require('@electron-toolkit/utils');
  electronApp.setAppUserModelId('com.asrpro.app');

  // Create icon after app is ready
  icon = nativeImageInstance.createFromPath(join(__dirname, '../../resources/icon.png'));

  // Default open or close DevTools by F12 in development
  appInstance.on('browser-window-created', (_, window) => {
    optimizer.watchWindowShortcuts(window);
  });

  createWindow();

  appInstance.on('activate', function () {
    // On macOS it's common to re-create a window in the app when the
    // dock icon is clicked and there are no other windows open.
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });

  // Set up IPC handlers
  setupIpcHandlers();

  // Initialize backend server after app is ready
  try {
    backendServer = require('./backend/server');
    console.log('Backend server initialized successfully');
  } catch (error) {
    console.error('Failed to initialize backend server:', error);
  }
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
    appInstance.quit();
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