const { app, BrowserWindow, dialog, ipcMain, Menu, shell } = require("electron");
const path = require("node:path");

const DEV_SERVER_URL = process.env.VITE_DEV_SERVER_URL || "http://127.0.0.1:4270";

let mainWindow;

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1180,
    height: 760,
    minWidth: 980,
    minHeight: 680,
    show: false,
    frame: false,
    title: "ASR Pro",
    backgroundColor: "#e8e8ed",
    webPreferences: {
      preload: path.join(__dirname, "preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false,
    },
  });

  mainWindow.once("ready-to-show", () => {
    mainWindow.show();
  });

  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    shell.openExternal(url);
    return { action: "deny" };
  });

  mainWindow.webContents.session.clearCache().catch(() => {});

  if (app.isPackaged) {
    mainWindow.loadFile(path.join(__dirname, "../dist/index.html"));
  } else {
    mainWindow.loadURL(DEV_SERVER_URL);
  }
}

function registerIpc() {
  ipcMain.handle("app:platform", () => ({
    platform: process.platform,
    arch: process.arch,
    versions: {
      electron: process.versions.electron,
      chrome: process.versions.chrome,
      node: process.versions.node,
    },
  }));

  ipcMain.handle("app:info", () => ({
    name: app.getName(),
    version: app.getVersion(),
  }));

  ipcMain.handle("dialog:select-audio", async () => {
    const result = await dialog.showOpenDialog(mainWindow, {
      title: "Choose audio or video to transcribe",
      properties: ["openFile", "multiSelections"],
      filters: [
        {
          name: "Audio and Video",
          extensions: ["mp3", "wav", "m4a", "flac", "aac", "ogg", "mp4", "mov", "mkv", "webm"],
        },
        { name: "All Files", extensions: ["*"] },
      ],
    });

    if (result.canceled) {
      return [];
    }

    return result.filePaths.map((filePath) => ({
      fileName: path.basename(filePath),
      path: filePath,
    }));
  });

  ipcMain.handle("window:control", (event, action) => {
    const senderWindow = BrowserWindow.fromWebContents(event.sender);
    if (!senderWindow) return;

    if (action === "minimize") senderWindow.minimize();
    if (action === "maximize") {
      if (senderWindow.isMaximized()) {
        senderWindow.unmaximize();
      } else {
        senderWindow.maximize();
      }
    }
    if (action === "close") senderWindow.close();
  });
}

function createMenu() {
  return Menu.buildFromTemplate([
    {
      label: "ASR Pro",
      submenu: [
        { role: "about" },
        { type: "separator" },
        { role: "hide" },
        { role: "hideOthers" },
        { role: "unhide" },
        { type: "separator" },
        { role: "quit" },
      ],
    },
    {
      label: "File",
      submenu: [{ label: "Add Files", accelerator: "CmdOrCtrl+O", click: () => mainWindow?.webContents.send("menu:add-files") }],
    },
    {
      label: "Edit",
      submenu: [{ role: "undo" }, { role: "redo" }, { type: "separator" }, { role: "cut" }, { role: "copy" }, { role: "paste" }],
    },
    {
      label: "View",
      submenu: [{ role: "reload" }, { role: "toggleDevTools" }, { type: "separator" }, { role: "resetZoom" }, { role: "zoomIn" }, { role: "zoomOut" }],
    },
    {
      label: "Window",
      submenu: [{ role: "minimize" }, { role: "zoom" }, { role: "close" }],
    },
  ]);
}

app.setAppUserModelId("com.surajmandal.asrpro");

app.whenReady().then(() => {
  registerIpc();
  Menu.setApplicationMenu(createMenu());
  createWindow();

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
