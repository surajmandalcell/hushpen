const { app, BrowserWindow, Tray, globalShortcut, ipcMain, Menu, nativeImage, nativeTheme, shell, screen, session } = require("electron");
const { spawn } = require("node:child_process");
const fs = require("node:fs");
const http = require("node:http");
const path = require("node:path");
const {
  APP_ID,
  APP_NAME,
  buildAboutPanelOptions,
} = require("./identity.cjs");
const {
  DEFAULT_MODEL,
  DEFAULT_OVERLAY_SETTINGS,
  OVERLAY_WINDOW_SIZE,
  RECORDING_SHORTCUT,
  SIDECAR_HEALTH_URL,
  buildSidecarLaunchConfig,
  buildModelPaths,
  createRecordingOverlayHtml,
  normalizeOverlaySettings,
  resolveContainedDataDir,
  resolveAppIconPath,
  resolveOverlayBounds,
  resolveRuntimeAssetRoot,
  resolveTrayIconPath,
  shouldShowRecordingOverlay,
} = require("./runtime.cjs");

const DEV_SERVER_URL = process.env.VITE_DEV_SERVER_URL || "http://127.0.0.1:4270";
const MAIN_WINDOW_SIZE = { width: 780, height: 520 };
const SCREENSHOT_MODE = process.env.ASRPRO_SCREENSHOT_MODE === "1";

let mainWindow;
let overlayWindow;
let tray;
let containedDataDir;
let sidecarProcess;
let sidecarStartPromise;
let isQuitting = false;
let isRecording = false;
let shortcutRegistered = false;
let overlaySettings = DEFAULT_OVERLAY_SETTINGS;
let positioningOverlay = false;
let lastWaveformFrame = [];
let sidecarState = {
  status: "stopped",
  mode: "unknown",
  healthUrl: SIDECAR_HEALTH_URL,
};

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
    userDataPath: app.getPath("userData"),
    dataDirOverride: process.env.ASRPRO_DATA_DIR,
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

  overlaySettings = loadOverlaySettings();
}

function createWindow() {
  if (mainWindow && !mainWindow.isDestroyed()) {
    return mainWindow;
  }

  mainWindow = new BrowserWindow({
    width: MAIN_WINDOW_SIZE.width,
    height: MAIN_WINDOW_SIZE.height,
    minWidth: MAIN_WINDOW_SIZE.width,
    minHeight: MAIN_WINDOW_SIZE.height,
    maxWidth: MAIN_WINDOW_SIZE.width,
    maxHeight: MAIN_WINDOW_SIZE.height,
    show: false,
    frame: false,
    resizable: false,
    maximizable: false,
    fullscreenable: false,
    title: APP_NAME,
    icon: resolveAppIconPath(process.platform, getRuntimeAssetRoot()),
    backgroundColor: "#2f2f2f",
    webPreferences: {
      preload: path.join(__dirname, "preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false,
      backgroundThrottling: false,
    },
  });
  lockMainWindowSize(mainWindow);

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

function lockMainWindowSize(win) {
  win.setMinimumSize(MAIN_WINDOW_SIZE.width, MAIN_WINDOW_SIZE.height);
  win.setMaximumSize(MAIN_WINDOW_SIZE.width, MAIN_WINDOW_SIZE.height);
  win.setResizable(false);
  win.setMaximizable(false);
  win.setFullScreenable(false);

  win.on("will-resize", (event) => {
    event.preventDefault();
    win.setSize(MAIN_WINDOW_SIZE.width, MAIN_WINDOW_SIZE.height, false);
  });

  win.on("resize", () => {
    const [width, height] = win.getSize();
    if (width !== MAIN_WINDOW_SIZE.width || height !== MAIN_WINDOW_SIZE.height) {
      win.setSize(MAIN_WINDOW_SIZE.width, MAIN_WINDOW_SIZE.height, false);
    }
  });

  win.on("maximize", () => {
    win.unmaximize();
    win.setSize(MAIN_WINDOW_SIZE.width, MAIN_WINDOW_SIZE.height, false);
  });

  win.on("enter-full-screen", () => {
    win.setFullScreen(false);
    win.setSize(MAIN_WINDOW_SIZE.width, MAIN_WINDOW_SIZE.height, false);
  });
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

  ipcMain.handle("sidecar:state", () => sidecarState);

  ipcMain.handle("overlay-settings:get", () => overlaySettings);

  ipcMain.handle("overlay-settings:update", (_event, settings) => updateOverlaySettings(settings));

  ipcMain.handle("recording:set", (_event, active) => {
    setRecording(Boolean(active), "renderer");
    return getRecordingState();
  });

  ipcMain.handle("recording:toggle", () => {
    setRecording(!isRecording, "renderer");
    return getRecordingState();
  });

  ipcMain.on("recording:waveform-frame", (_event, frame) => {
    updateOverlayWaveformFrame(frame);
  });

  ipcMain.handle("window:control", (event, action) => {
    const senderWindow = BrowserWindow.fromWebContents(event.sender);
    if (!senderWindow) return;

    if (action === "minimize") senderWindow.minimize();
    if (action === "close") senderWindow.close();
  });
}

function configureMediaPermissions() {
  session.defaultSession.setPermissionRequestHandler((webContents, permission, callback, details = {}) => {
    callback(isTrustedMediaPermission(webContents, permission, details));
  });

  session.defaultSession.setPermissionCheckHandler((webContents, permission, requestingOrigin, details = {}) => (
    isTrustedMediaPermission(webContents, permission, {
      ...details,
      requestingOrigin,
    })
  ));
}

function isTrustedMediaPermission(webContents, permission, details = {}) {
  if (permission !== "media") return false;

  const mediaType = details.mediaType || (Array.isArray(details.mediaTypes) ? details.mediaTypes[0] : undefined);
  if (mediaType && mediaType !== "audio" && mediaType !== "unknown") {
    return false;
  }

  return [
    details.requestingUrl,
    details.requestingOrigin,
    details.securityOrigin,
    webContents?.getURL?.(),
  ].some(isTrustedAppUrl);
}

function isTrustedAppUrl(value = "") {
  if (!value) return false;

  try {
    const url = new URL(value);
    const devUrl = new URL(DEV_SERVER_URL);
    return url.protocol === "file:" || url.origin === devUrl.origin;
  } catch {
    return value.startsWith("file://") || value.startsWith(DEV_SERVER_URL);
  }
}

function createMenu() {
  return Menu.buildFromTemplate([
    {
      label: APP_NAME,
      submenu: [
        { label: `About ${APP_NAME}`, click: () => app.showAboutPanel() },
        { type: "separator" },
        { role: "hide" },
        { role: "hideOthers" },
        { role: "unhide" },
        { type: "separator" },
        { label: `Quit ${APP_NAME}`, accelerator: "CmdOrCtrl+Q", click: quitApp },
      ],
    },
    {
      label: "File",
      submenu: [
        { label: "Start or Stop Recording", accelerator: RECORDING_SHORTCUT, click: () => setRecording(!isRecording, "menu") },
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
      submenu: [{ label: `Show ${APP_NAME}`, click: showMainWindow }, { role: "minimize" }, { role: "close" }],
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

  tray = new Tray(createTrayIcon());
  tray.setToolTip(APP_NAME);
  tray.on("click", showMainWindow);
  updateTrayMenu();
}

function createTrayIcon() {
  const iconPath = resolveTrayIconPath(process.platform, getRuntimeAssetRoot(), nativeTheme.shouldUseDarkColors);
  const icon = nativeImage.createFromPath(iconPath);
  if (process.platform === "darwin") {
    icon.setTemplateImage(true);
  }
  return icon;
}

function getRuntimeAssetRoot() {
  return resolveRuntimeAssetRoot({
    isPackaged: app.isPackaged,
    resourcesPath: process.resourcesPath,
    appPath: app.getAppPath(),
  });
}

function updateTrayIcon() {
  if (!tray) return;
  tray.setImage(createTrayIcon());
}

function updateTrayMenu() {
  if (!tray) return;

  tray.setContextMenu(Menu.buildFromTemplate([
    { label: `Show ${APP_NAME}`, click: showMainWindow },
    { type: "separator" },
    {
      label: isRecording ? "Stop Recording" : "Start Recording",
      accelerator: RECORDING_SHORTCUT,
      click: () => setRecording(!isRecording, "tray"),
    },
    { type: "separator" },
    { label: `Quit ${APP_NAME}`, click: quitApp },
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
    overlaySettings,
    sidecar: sidecarState,
    shortcut: RECORDING_SHORTCUT,
    shortcutRegistered,
  };
}

function getDevelopmentPythonCommand() {
  const localPython = process.platform === "win32"
    ? path.join(app.getAppPath(), "sidecar", ".venv", "Scripts", "python.exe")
    : path.join(app.getAppPath(), "sidecar", ".venv", "bin", "python");

  if (fs.existsSync(localPython)) return localPython;
  return process.platform === "win32" ? "python" : "python3";
}

function getSidecarLaunchConfig() {
  return buildSidecarLaunchConfig({
    isPackaged: app.isPackaged,
    platform: process.platform,
    resourcesPath: process.resourcesPath,
    appPath: app.getAppPath(),
    pythonCommand: getDevelopmentPythonCommand(),
  });
}

function setSidecarState(nextState) {
  sidecarState = {
    ...sidecarState,
    ...nextState,
    healthUrl: SIDECAR_HEALTH_URL,
    updatedAt: new Date().toISOString(),
  };
  emitSidecarState();
  return sidecarState;
}

function emitSidecarState() {
  if (mainWindow && !mainWindow.isDestroyed()) {
    mainWindow.webContents.send("sidecar:state", sidecarState);
  }
}

function checkSidecarHealth(timeoutMs = 900) {
  return new Promise((resolve) => {
    const healthUrl = new URL(SIDECAR_HEALTH_URL);
    const request = http.get({
      hostname: healthUrl.hostname,
      port: healthUrl.port,
      path: healthUrl.pathname,
      timeout: timeoutMs,
    }, (response) => {
      response.resume();
      resolve(response.statusCode >= 200 && response.statusCode < 300);
    });

    request.on("timeout", () => {
      request.destroy();
      resolve(false);
    });
    request.on("error", () => resolve(false));
  });
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitForSidecarHealth(timeoutMs = 15000, shouldAbort = () => false) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (shouldAbort()) return false;
    if (await checkSidecarHealth(800)) return true;
    await delay(250);
  }
  return false;
}

async function startSidecar() {
  if (sidecarStartPromise) return sidecarStartPromise;

  sidecarStartPromise = (async () => {
    if (await checkSidecarHealth(500)) {
      return setSidecarState({
        status: "ready",
        mode: "external",
        pid: null,
        error: null,
      });
    }

    const launchConfig = getSidecarLaunchConfig();
    if (launchConfig.mode === "missing") {
      return setSidecarState({
        status: "failed",
        mode: "missing",
        command: null,
        args: [],
        pid: null,
        error: launchConfig.error,
      });
    }

    setSidecarState({
      status: "starting",
      mode: launchConfig.mode,
      command: launchConfig.command,
      args: launchConfig.args,
      pid: null,
      error: null,
    });

    let spawnError = null;
    sidecarProcess = spawn(launchConfig.command, launchConfig.args, {
      cwd: launchConfig.cwd,
      env: {
        ...process.env,
        ASRPRO_DATA_DIR: containedDataDir,
        ASRPRO_EAGER_LOAD_MODEL: "0",
      },
      stdio: "ignore",
      windowsHide: true,
    });

    sidecarProcess.once("error", (error) => {
      spawnError = error;
      setSidecarState({
        status: "failed",
        mode: launchConfig.mode,
        command: launchConfig.command,
        args: launchConfig.args,
        pid: null,
        error: error.message,
      });
    });

    sidecarProcess.once("exit", (code, signal) => {
      const wasManagedProcess = sidecarProcess;
      sidecarProcess = undefined;
      if (!isQuitting && wasManagedProcess) {
        setSidecarState({
          status: "failed",
          mode: launchConfig.mode,
          pid: null,
          error: `ASR engine exited with code ${code ?? "null"} and signal ${signal ?? "null"}.`,
        });
      }
    });

    const ready = await waitForSidecarHealth(15000, () => Boolean(spawnError));
    if (ready) {
      return setSidecarState({
        status: "ready",
        mode: launchConfig.mode,
        command: launchConfig.command,
        args: launchConfig.args,
        pid: sidecarProcess?.pid ?? null,
        error: null,
      });
    }

    if (sidecarProcess && !sidecarProcess.killed) {
      sidecarProcess.kill();
    }

    return setSidecarState({
      status: "failed",
      mode: launchConfig.mode,
      command: launchConfig.command,
      args: launchConfig.args,
      pid: null,
      error: spawnError?.message || "ASR engine did not become healthy before the startup timeout.",
    });
  })();

  return sidecarStartPromise;
}

function stopSidecar() {
  if (sidecarProcess && !sidecarProcess.killed) {
    sidecarProcess.kill();
  }
  sidecarProcess = undefined;
}

function setRecording(active, source = "app") {
  if (isRecording === active) {
    if (active && shouldShowRecordingOverlay(source)) {
      showRecordingOverlay();
    }
    if (active && !shouldShowRecordingOverlay(source)) {
      hideRecordingOverlay();
    }
    emitRecordingState(source);
    return;
  }

  isRecording = active;
  if (isRecording) {
    if (shouldShowRecordingOverlay(source)) {
      showRecordingOverlay();
    } else {
      hideRecordingOverlay();
    }
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
    positionRecordingOverlay();
    overlayWindow.showInactive();
    return;
  }

  const width = OVERLAY_WINDOW_SIZE.width;
  const height = OVERLAY_WINDOW_SIZE.height;
  const bounds = resolveOverlayBounds({
    settings: overlaySettings,
    primaryDisplay: screen.getPrimaryDisplay(),
    displays: screen.getAllDisplays(),
    width,
    height,
  });

  overlayWindow = new BrowserWindow({
    width,
    height,
    x: bounds.x,
    y: bounds.y,
    frame: false,
    transparent: true,
    resizable: false,
    movable: true,
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
      preload: path.join(__dirname, "overlay-preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      backgroundThrottling: false,
    },
  });

  overlayWindow.setAlwaysOnTop(true, "screen-saver");
  overlayWindow.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true });
  overlayWindow.setIgnoreMouseEvents(false);
  overlayWindow.once("ready-to-show", () => {
    overlayWindow?.showInactive();
    updateOverlayWaveformFrame(lastWaveformFrame);
  });
  overlayWindow.on("move", persistDraggedOverlayPosition);
  overlayWindow.on("closed", () => {
    overlayWindow = undefined;
  });
  overlayWindow.loadURL(`data:text/html;charset=UTF-8,${encodeURIComponent(createRecordingOverlayHtml({
    modelName: DEFAULT_MODEL.displayName,
    shortcut: RECORDING_SHORTCUT,
  }))}`);
}

function hideRecordingOverlay() {
  if (overlayWindow && !overlayWindow.isDestroyed()) {
    overlayWindow.close();
  }
  overlayWindow = undefined;
}

function updateOverlayWaveformFrame(frame) {
  const normalizedFrame = Array.isArray(frame)
    ? frame.slice(0, 80).map((value) => Math.min(Math.max(Number(value) || 0, 0), 1))
    : [];

  lastWaveformFrame = normalizedFrame;

  if (!overlayWindow || overlayWindow.isDestroyed()) return;

  overlayWindow.webContents.send("overlay:waveform-frame", normalizedFrame);
}

function getOverlaySettingsPath() {
  return path.join(containedDataDir, "config", "overlay-settings.json");
}

function loadOverlaySettings() {
  try {
    const settingsPath = getOverlaySettingsPath();
    if (!fs.existsSync(settingsPath)) {
      return DEFAULT_OVERLAY_SETTINGS;
    }

    return normalizeOverlaySettings(JSON.parse(fs.readFileSync(settingsPath, "utf8")));
  } catch {
    return DEFAULT_OVERLAY_SETTINGS;
  }
}

function saveOverlaySettings() {
  fs.writeFileSync(getOverlaySettingsPath(), JSON.stringify(overlaySettings, null, 2));
}

function updateOverlaySettings(settings) {
  const nextSettings = {
    ...overlaySettings,
    ...(settings || {}),
  };

  if (settings && Object.prototype.hasOwnProperty.call(settings, "placement")) {
    nextSettings.customBounds = null;
  }

  overlaySettings = normalizeOverlaySettings(nextSettings);
  saveOverlaySettings();
  positionRecordingOverlay();
  return overlaySettings;
}

function positionRecordingOverlay() {
  if (!overlayWindow || overlayWindow.isDestroyed()) return;

  const bounds = resolveOverlayBounds({
    settings: overlaySettings,
    primaryDisplay: screen.getPrimaryDisplay(),
    displays: screen.getAllDisplays(),
    width: OVERLAY_WINDOW_SIZE.width,
    height: OVERLAY_WINDOW_SIZE.height,
  });

  positioningOverlay = true;
  overlayWindow.setBounds({
    ...bounds,
    ...OVERLAY_WINDOW_SIZE,
  });
  setTimeout(() => {
    positioningOverlay = false;
  }, 80);
}

function persistDraggedOverlayPosition() {
  if (positioningOverlay || !overlayWindow || overlayWindow.isDestroyed()) return;

  const bounds = overlayWindow.getBounds();
  const display = screen.getDisplayMatching(bounds);
  overlaySettings = normalizeOverlaySettings({
    ...overlaySettings,
    customBounds: {
      displayId: display.id,
      x: bounds.x,
      y: bounds.y,
    },
  });
  saveOverlaySettings();
}

function quitApp() {
  isQuitting = true;
  hideRecordingOverlay();
  stopSidecar();
  if (tray) {
    tray.destroy();
    tray = undefined;
  }
  app.quit();
}

app.setName(APP_NAME);
app.setAppUserModelId(APP_ID);
app.setAboutPanelOptions(buildAboutPanelOptions(app.getVersion()));

if (hasSingleInstanceLock) {
  app.on("second-instance", showMainWindow);

  app.whenReady().then(async () => {
    registerIpc();
    configureMediaPermissions();
    if (SCREENSHOT_MODE) {
      setSidecarState({
        status: "ready",
        mode: "screenshot",
        command: null,
        args: [],
        pid: null,
        error: null,
      });
      Menu.setApplicationMenu(null);
    } else {
      await startSidecar();
      Menu.setApplicationMenu(createMenu());
    }
    createWindow();
    if (!SCREENSHOT_MODE) {
      registerGlobalShortcut();
      createTray();
      nativeTheme.on("updated", updateTrayIcon);
    }

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
  stopSidecar();
  if (tray) {
    tray.destroy();
    tray = undefined;
  }
});
