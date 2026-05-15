const { app, BrowserWindow, Tray, globalShortcut, ipcMain, Menu, nativeImage, nativeTheme, shell, screen, session } = require("electron");
const fs = require("node:fs");
const path = require("node:path");
const {
  APP_ID,
  APP_NAME,
  buildAboutPanelOptions,
} = require("./identity.cjs");
const {
  AVAILABLE_MODELS,
  DEFAULT_MODEL,
  DEFAULT_OVERLAY_SETTINGS,
  OVERLAY_WINDOW_SIZE,
  RECORDING_SHORTCUT,
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
const {
  listModels,
  transcribeAudioFile,
} = require("./whisper-engine.cjs");

const DEV_SERVER_URL = process.env.VITE_DEV_SERVER_URL || "http://127.0.0.1:4270";
const MAIN_WINDOW_SIZE = { width: 780, height: 520 };
const MAIN_WINDOW_BACKGROUND = "#2f2f2f";
const SCREENSHOT_MODE = process.env.ASRPRO_SCREENSHOT_MODE === "1";

let mainWindow;
let overlayWindow;
let tray;
let containedDataDir;
let isQuitting = false;
let isRecording = false;
let shortcutRegistered = false;
let overlaySettings = DEFAULT_OVERLAY_SETTINGS;
let positioningOverlay = false;
let lastWaveformFrame = [];
let engineState = {
  status: "idle",
  mode: "native-node",
  modelId: DEFAULT_MODEL.id,
  model: DEFAULT_MODEL.displayName,
  progress: null,
  error: null,
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
    paths.whisperModelsDir,
  ]) {
    fs.mkdirSync(dir, { recursive: true });
  }

  app.setPath("userData", path.join(containedDataDir, "user-data"));
  app.setPath("sessionData", path.join(containedDataDir, "session"));
  app.setPath("logs", path.join(containedDataDir, "logs"));

  process.env.ASRPRO_DATA_DIR = containedDataDir;
  process.env.ASRPRO_DEFAULT_MODEL = DEFAULT_MODEL.id;
  process.env.XDG_CACHE_HOME = path.join(containedDataDir, "cache");

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
    backgroundColor: MAIN_WINDOW_BACKGROUND,
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

  ipcMain.handle("engine:state", () => engineState);

  ipcMain.handle("engine:models", () => listModels(containedDataDir));

  ipcMain.handle("engine:transcribe-audio", (_event, request) => transcribeAudio(request));

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
    models: listModels(containedDataDir),
    overlaySettings,
    engine: engineState,
    shortcut: RECORDING_SHORTCUT,
    shortcutRegistered,
    capabilities: {
      nativeWhisper: true,
    },
  };
}

function setEngineState(nextState) {
  engineState = {
    ...engineState,
    ...nextState,
    updatedAt: new Date().toISOString(),
  };
  if (mainWindow && !mainWindow.isDestroyed()) {
    mainWindow.webContents.send("engine:state", engineState);
  }
  return engineState;
}

function createTempAudioPath(mimeType = "audio/wav") {
  const extension = mimeType.includes("wav") ? "wav" : "audio";
  const tempDir = fs.mkdtempSync(path.join(app.getPath("temp"), "asrpro-whisper-"));
  return {
    tempDir,
    filePath: path.join(tempDir, `recording.${extension}`),
  };
}

function toAudioBuffer(audioData) {
  if (Buffer.isBuffer(audioData)) return audioData;
  if (audioData instanceof ArrayBuffer) return Buffer.from(audioData);
  if (ArrayBuffer.isView(audioData)) {
    return Buffer.from(audioData.buffer, audioData.byteOffset, audioData.byteLength);
  }
  if (Array.isArray(audioData)) return Buffer.from(audioData);
  throw new Error("Transcription audio payload is missing.");
}

async function transcribeAudio(request = {}) {
  const modelId = request.modelId || DEFAULT_MODEL.id;
  const model = AVAILABLE_MODELS.find((candidate) => candidate.id === modelId);
  if (!model) {
    throw new Error(`Unsupported recognition model: ${modelId}`);
  }

  const { tempDir, filePath } = createTempAudioPath(request.mimeType);
  try {
    fs.writeFileSync(filePath, toAudioBuffer(request.audioData));
    const result = await transcribeAudioFile({
      filePath,
      modelId: model.id,
      dataDir: containedDataDir,
      onState: setEngineState,
    });
    setEngineState({
      status: "ready",
      mode: "native-node",
      modelId: model.id,
      model: model.displayName,
      progress: null,
      error: null,
    });
    return result;
  } catch (error) {
    setEngineState({
      status: "failed",
      mode: "native-node",
      modelId: model.id,
      model: model.displayName,
      progress: null,
      error: error instanceof Error ? error.message : "Whisper transcription failed.",
    });
    throw error;
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
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

  app.whenReady().then(() => {
    registerIpc();
    configureMediaPermissions();
    if (SCREENSHOT_MODE) {
      setEngineState({
        status: "ready",
        mode: "screenshot",
        modelId: DEFAULT_MODEL.id,
        model: DEFAULT_MODEL.displayName,
        progress: null,
        error: null,
      });
      Menu.setApplicationMenu(null);
    } else {
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
  if (tray) {
    tray.destroy();
    tray = undefined;
  }
});
