const { app, BrowserWindow, Tray, dialog, globalShortcut, ipcMain, Menu, shell, screen } = require("electron");
const fs = require("node:fs");
const path = require("node:path");
const {
  DEFAULT_MODEL,
  RECORDING_SHORTCUT,
  buildModelPaths,
  createRecordingOverlayHtml,
  resolveContainedDataDir,
  resolveTrayIconPath,
} = require("./runtime.cjs");

const DEV_SERVER_URL = process.env.VITE_DEV_SERVER_URL || "http://127.0.0.1:4270";

let mainWindow;
let overlayWindow;
let tray;
let containedDataDir;
let isQuitting = false;
let isRecording = false;
let shortcutRegistered = false;

app.commandLine.appendSwitch("enable-features", "GlobalShortcutsPortal");

configureContainedData();

const hasSingleInstanceLock = app.requestSingleInstanceLock();

if (!hasSingleInstanceLock) {
  app.quit();
}

function configureContainedData() {
  containedDataDir = resolveContainedDataDir({
    isPackaged: app.isPackaged,
    platform: process.platform,
    resourcesPath: process.resourcesPath,
    exePath: app.getPath("exe"),
    appPath: app.getAppPath(),
  });

  const paths = buildModelPaths(containedDataDir);
  for (const dir of [
    containedDataDir,
    path.join(containedDataDir, "config"),
    path.join(containedDataDir, "session"),
    path.join(containedDataDir, "logs"),
    paths.modelsDir,
    paths.defaultModelDir,
  ]) {
    fs.mkdirSync(dir, { recursive: true });
  }

  app.setPath("userData", path.join(containedDataDir, "user-data"));
  app.setPath("sessionData", path.join(containedDataDir, "session"));
  app.setPath("logs", path.join(containedDataDir, "logs"));

  process.env.ASRPRO_DATA_DIR = containedDataDir;
  process.env.ASRPRO_DEFAULT_MODEL = DEFAULT_MODEL.id;
  process.env.ASRPRO_DEFAULT_MODEL_REPO = DEFAULT_MODEL.repo;
  process.env.HF_HOME = path.join(paths.modelsDir, "huggingface");
  process.env.HUGGINGFACE_HUB_CACHE = path.join(paths.modelsDir, "huggingface", "hub");
  process.env.NEMO_HOME = path.join(paths.modelsDir, "nemo");
  process.env.TORCH_HOME = path.join(paths.modelsDir, "torch");
  process.env.XDG_CACHE_HOME = path.join(containedDataDir, "cache");

  if (!fs.existsSync(paths.defaultModelManifest)) {
    fs.writeFileSync(
      paths.defaultModelManifest,
      JSON.stringify({
        id: DEFAULT_MODEL.id,
        displayName: DEFAULT_MODEL.displayName,
        repo: DEFAULT_MODEL.repo,
        status: "download-on-first-use",
        cacheDir: paths.defaultModelDir,
      }, null, 2)
    );
  }
}

function createWindow() {
  if (mainWindow && !mainWindow.isDestroyed()) {
    return mainWindow;
  }

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

  mainWindow.on("close", (event) => {
    if (!isQuitting) {
      event.preventDefault();
      mainWindow.hide();
    }
  });

  mainWindow.on("closed", () => {
    mainWindow = undefined;
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

  return mainWindow;
}

function showMainWindow() {
  const win = createWindow();
  if (win.isMinimized()) win.restore();
  win.show();
  win.focus();
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

  ipcMain.handle("runtime:state", () => getRuntimeState());

  ipcMain.handle("recording:set", (_event, active) => {
    setRecording(Boolean(active), "renderer");
    return getRecordingState();
  });

  ipcMain.handle("recording:toggle", () => {
    setRecording(!isRecording, "renderer");
    return getRecordingState();
  });

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
        { label: "Quit ASR Pro", accelerator: "CmdOrCtrl+Q", click: quitApp },
      ],
    },
    {
      label: "File",
      submenu: [
        { label: "Start or Stop Recording", accelerator: RECORDING_SHORTCUT, click: () => setRecording(!isRecording, "menu") },
        { label: "Add Files", accelerator: "CmdOrCtrl+O", click: () => showMainWindow().webContents.send("menu:add-files") },
      ],
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
      submenu: [{ label: "Show ASR Pro", click: showMainWindow }, { role: "minimize" }, { role: "zoom" }, { role: "close" }],
    },
  ]);
}

function registerGlobalShortcut() {
  shortcutRegistered = globalShortcut.register(RECORDING_SHORTCUT, () => {
    setRecording(!isRecording, "shortcut");
  });
}

function createTray() {
  if (tray) return;

  const iconPath = resolveTrayIconPath(process.platform, path.join(__dirname, ".."));
  tray = new Tray(iconPath);
  tray.setToolTip("ASR Pro");
  tray.on("click", showMainWindow);
  updateTrayMenu();
}

function updateTrayMenu() {
  if (!tray) return;

  tray.setContextMenu(Menu.buildFromTemplate([
    { label: "Show ASR Pro", click: showMainWindow },
    { type: "separator" },
    {
      label: isRecording ? "Stop Recording" : "Start Recording",
      accelerator: RECORDING_SHORTCUT,
      click: () => setRecording(!isRecording, "tray"),
    },
    { label: "Add Files", click: () => showMainWindow().webContents.send("menu:add-files") },
    { type: "separator" },
    { label: "Quit ASR Pro", click: quitApp },
  ]));
}

function getRecordingState(source = "app") {
  return {
    isRecording,
    source,
    model: DEFAULT_MODEL.displayName,
    shortcut: RECORDING_SHORTCUT,
  };
}

function getRuntimeState() {
  return {
    ...getRecordingState(),
    dataDir: containedDataDir,
    defaultModel: DEFAULT_MODEL.displayName,
    defaultModelId: DEFAULT_MODEL.id,
    defaultModelRepo: DEFAULT_MODEL.repo,
    shortcut: RECORDING_SHORTCUT,
    shortcutRegistered,
  };
}

function setRecording(active, source = "app") {
  if (isRecording === active) {
    emitRecordingState(source);
    return;
  }

  isRecording = active;
  if (isRecording) {
    showRecordingOverlay();
  } else {
    hideRecordingOverlay();
  }

  updateTrayMenu();
  emitRecordingState(source);
}

function emitRecordingState(source) {
  if (mainWindow && !mainWindow.isDestroyed()) {
    mainWindow.webContents.send("recording:state", getRecordingState(source));
  }
}

function showRecordingOverlay() {
  if (overlayWindow && !overlayWindow.isDestroyed()) {
    overlayWindow.showInactive();
    return;
  }

  const { workArea } = screen.getPrimaryDisplay();
  const width = 360;
  const height = 52;

  overlayWindow = new BrowserWindow({
    width,
    height,
    x: Math.round(workArea.x + (workArea.width - width) / 2),
    y: Math.round(workArea.y + 14),
    frame: false,
    transparent: true,
    resizable: false,
    movable: false,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: true,
    show: false,
    focusable: false,
    alwaysOnTop: true,
    hasShadow: false,
    backgroundColor: "#00000000",
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });

  overlayWindow.setAlwaysOnTop(true, "screen-saver");
  overlayWindow.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true });
  overlayWindow.setIgnoreMouseEvents(true, { forward: true });
  overlayWindow.once("ready-to-show", () => overlayWindow?.showInactive());
  overlayWindow.on("closed", () => {
    overlayWindow = undefined;
  });
  overlayWindow.loadURL(`data:text/html;charset=UTF-8,${encodeURIComponent(createRecordingOverlayHtml())}`);
}

function hideRecordingOverlay() {
  if (overlayWindow && !overlayWindow.isDestroyed()) {
    overlayWindow.close();
  }
  overlayWindow = undefined;
}

function quitApp() {
  isQuitting = true;
  hideRecordingOverlay();
  if (tray) {
    tray.destroy();
    tray = undefined;
  }
  app.quit();
}

app.setAppUserModelId("com.surajmandal.asrpro");

if (hasSingleInstanceLock) {
  app.on("second-instance", showMainWindow);

  app.whenReady().then(() => {
    registerIpc();
    Menu.setApplicationMenu(createMenu());
    createWindow();
    registerGlobalShortcut();
    createTray();

    app.on("activate", showMainWindow);
  });
}

app.on("window-all-closed", () => {
  if (isQuitting) app.quit();
});

app.on("before-quit", () => {
  isQuitting = true;
});

app.on("will-quit", () => {
  globalShortcut.unregisterAll();
  hideRecordingOverlay();
  if (tray) {
    tray.destroy();
    tray = undefined;
  }
});
