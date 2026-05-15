const { app, BrowserWindow, Tray, globalShortcut, ipcMain, Menu, nativeImage, nativeTheme, shell, screen, session } = require("electron");
const { execFile } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const { promisify } = require("node:util");
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
  collectRuntimeStorageStats,
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
  deleteModelFile,
  downloadModelFile,
  listModels,
  transcribeAudioFile,
} = require("./whisper-engine.cjs");

const DEV_SERVER_URL = process.env.VITE_DEV_SERVER_URL || "http://127.0.0.1:4270";
const MAIN_WINDOW_SIZE = { width: 780, height: 520 };
const MAIN_WINDOW_BACKGROUND = "#2f2f2f";
const SCREENSHOT_MODE = process.env.ASRPRO_SCREENSHOT_MODE === "1";
const execFileAsync = promisify(execFile);
const TEXT_EDITOR_OPTIONS = Object.freeze([
  {
    id: "system",
    label: "System default",
    detail: "Use the operating system default editor",
  },
  {
    id: "textedit",
    label: "TextEdit",
    detail: "Open transcript text in Apple TextEdit",
    macApp: "TextEdit",
    macBundleNames: ["TextEdit.app"],
  },
  {
    id: "vscode",
    label: "Visual Studio Code",
    detail: "Open transcript text in VS Code",
    macApp: "Visual Studio Code",
    macBundleNames: ["Visual Studio Code.app"],
  },
  {
    id: "cursor",
    label: "Cursor",
    detail: "Open transcript text in Cursor",
    macApp: "Cursor",
    macBundleNames: ["Cursor.app"],
  },
]);
const DEFAULT_APP_SETTINGS = Object.freeze({
  defaultTextEditor: "system",
  autoCopyTranscripts: true,
});
const textEditorIconDataUrlCache = new Map();

let mainWindow;
let overlayWindow;
let tray;
let containedDataDir;
let isQuitting = false;
let isRecording = false;
let shortcutRegistered = false;
let overlaySettings = DEFAULT_OVERLAY_SETTINGS;
let appSettings = DEFAULT_APP_SETTINGS;
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
    path.join(containedDataDir, "transcripts"),
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
  appSettings = loadAppSettings();
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

  ipcMain.handle("engine:model-download", (_event, request) => downloadModel(request));

  ipcMain.handle("engine:model-delete", (_event, request) => deleteModel(request));

  ipcMain.handle("engine:transcribe-audio", (_event, request) => transcribeAudio(request));

  ipcMain.handle("transcript:open-text", (_event, request) => openTranscriptText(request));

  ipcMain.handle("transcript:delete-text", (_event, request) => deleteTranscriptText(request));

  ipcMain.handle("overlay-settings:get", () => overlaySettings);

  ipcMain.handle("overlay-settings:update", (_event, settings) => updateOverlaySettings(settings));

  ipcMain.handle("settings:text-editor", (_event, editorId) => setDefaultTextEditor(editorId));

  ipcMain.handle("settings:auto-copy-transcripts", (_event, enabled) => setAutoCopyTranscripts(enabled));

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

function createAppIcon() {
  return nativeImage.createFromPath(resolveAppIconPath(process.platform, getRuntimeAssetRoot()));
}

function setMacDockIcon() {
  if (process.platform !== "darwin" || !app.dock) return;
  app.dock.setIcon(createAppIcon());
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

async function getRuntimeState() {
  return {
    ...getRecordingState(),
    dataDir: containedDataDir,
    defaultModel: DEFAULT_MODEL.displayName,
    defaultModelId: DEFAULT_MODEL.id,
    models: listModels(containedDataDir),
    defaultTextEditor: appSettings.defaultTextEditor,
    autoCopyTranscripts: appSettings.autoCopyTranscripts,
    textEditors: await getTextEditorOptions(),
    overlaySettings,
    engine: engineState,
    storageStats: collectRuntimeStorageStats(containedDataDir, process.memoryUsage(), engineState.modelId),
    shortcut: RECORDING_SHORTCUT,
    shortcutRegistered,
    capabilities: {
      nativeWhisper: true,
    },
  };
}

async function getTextEditorOptions() {
  return Promise.all(TEXT_EDITOR_OPTIONS.map(async (editor) => {
    const { macApp, macBundleNames, ...publicEditor } = editor;
    return {
      ...publicEditor,
      iconDataUrl: await getTextEditorIconDataUrl(editor),
    };
  }));
}

async function getTextEditorIconDataUrl(editor) {
  if (textEditorIconDataUrlCache.has(editor.id)) {
    return textEditorIconDataUrlCache.get(editor.id);
  }

  let iconDataUrl = "";
  try {
    const iconTarget = getTextEditorIconTarget(editor);
    if (iconTarget) {
      const icon = await app.getFileIcon(iconTarget, { size: "normal" });
      if (!icon.isEmpty()) {
        iconDataUrl = icon.resize({ width: 32, height: 32 }).toDataURL();
      }
    }
  } catch {
    iconDataUrl = "";
  }

  textEditorIconDataUrlCache.set(editor.id, iconDataUrl);
  return iconDataUrl;
}

function getTextEditorIconTarget(editor) {
  if (editor.id === DEFAULT_APP_SETTINGS.defaultTextEditor) {
    return ensureSystemTextIconProbe();
  }

  if (process.platform === "darwin" && Array.isArray(editor.macBundleNames)) {
    const appDirectories = [
      "/Applications",
      "/System/Applications",
      path.join(app.getPath("home"), "Applications"),
    ];

    for (const appDirectory of appDirectories) {
      for (const bundleName of editor.macBundleNames) {
        const bundlePath = path.join(appDirectory, bundleName);
        if (fs.existsSync(bundlePath)) return bundlePath;
      }
    }
  }

  return "";
}

function ensureSystemTextIconProbe() {
  const probePath = path.join(containedDataDir, "config", "text-editor-icon-probe.txt");
  if (!fs.existsSync(probePath)) {
    fs.writeFileSync(probePath, "", "utf8");
  }
  return probePath;
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

async function downloadModel(request = {}) {
  const model = AVAILABLE_MODELS.find((candidate) => candidate.id === request.modelId);
  if (!model) {
    throw new Error(`Unsupported recognition model: ${request.modelId}`);
  }

  try {
    await downloadModelFile({
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
  } catch (error) {
    setEngineState({
      status: "failed",
      mode: "native-node",
      modelId: model.id,
      model: model.displayName,
      progress: null,
      error: error instanceof Error ? error.message : "Whisper model setup failed.",
    });
    throw error;
  }

  return getRuntimeState();
}

async function deleteModel(request = {}) {
  const model = AVAILABLE_MODELS.find((candidate) => candidate.id === request.modelId);
  if (!model) {
    throw new Error(`Unsupported recognition model: ${request.modelId}`);
  }

  if ((engineState.status === "downloading" || engineState.status === "transcribing") && engineState.modelId === model.id) {
    throw new Error(`${model.displayName} is currently in use.`);
  }

  deleteModelFile({
    modelId: model.id,
    dataDir: containedDataDir,
  });

  return getRuntimeState();
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

function sanitizeTranscriptFileName(value = "transcript") {
  const normalized = String(value)
    .replace(/[<>:"/\\|?*\u0000-\u001f]/g, " ")
    .replace(/\s+/g, " ")
    .trim();

  return (normalized || "transcript").slice(0, 80);
}

async function openTranscriptText(request = {}) {
  const title = typeof request.title === "string" && request.title.trim() ? request.title : "Transcript";
  const text = typeof request.text === "string" ? request.text.trim() : "";
  const transcriptDir = getTranscriptDir();
  const filePath = getTranscriptTextPath(title);

  fs.mkdirSync(transcriptDir, { recursive: true });
  fs.writeFileSync(filePath, `${text || "No transcript text available."}\n`, "utf8");

  await openTranscriptFile(filePath, appSettings.defaultTextEditor);

  return { filePath };
}

function getTranscriptDir() {
  return path.join(containedDataDir, "transcripts");
}

function getTranscriptTextPath(title) {
  return path.join(getTranscriptDir(), `${sanitizeTranscriptFileName(title)}.txt`);
}

async function deleteTranscriptText(request = {}) {
  const filePath = resolveTranscriptDeletePath(request);
  const existed = fs.existsSync(filePath);
  fs.rmSync(filePath, { force: true });

  return { deleted: existed, filePath };
}

function resolveTranscriptDeletePath(request = {}) {
  const transcriptDir = getTranscriptDir();
  const requestedFilePath = typeof request.filePath === "string" ? request.filePath.trim() : "";
  const filePath = requestedFilePath
    ? path.resolve(requestedFilePath)
    : getTranscriptTextPath(typeof request.title === "string" && request.title.trim() ? request.title : "Transcript");

  if (!isPathInside(transcriptDir, filePath)) {
    throw new Error("Transcript file path is outside ASR Pro data.");
  }

  return filePath;
}

function isPathInside(parentDir, candidatePath) {
  const relativePath = path.relative(path.resolve(parentDir), path.resolve(candidatePath));
  return relativePath === "" || (relativePath && !relativePath.startsWith("..") && !path.isAbsolute(relativePath));
}

async function openTranscriptFile(filePath, editorId = DEFAULT_APP_SETTINGS.defaultTextEditor) {
  const editor = getTextEditorOption(editorId);

  if (process.platform === "darwin" && editor.macApp) {
    try {
      await execFileAsync("open", ["-a", editor.macApp, filePath]);
      return;
    } catch {
      // Fall back to the system handler when a configured app is not available.
    }
  }

  const openError = await shell.openPath(filePath);
  if (openError) {
    throw new Error(openError);
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

function getAppSettingsPath() {
  return path.join(containedDataDir, "config", "app-settings.json");
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

function normalizeAppSettings(settings = {}) {
  return {
    defaultTextEditor: normalizeTextEditorId(settings.defaultTextEditor),
    autoCopyTranscripts: normalizeBooleanSetting(settings.autoCopyTranscripts, DEFAULT_APP_SETTINGS.autoCopyTranscripts),
  };
}

function normalizeBooleanSetting(value, fallback) {
  return typeof value === "boolean" ? value : fallback;
}

function normalizeTextEditorId(editorId) {
  const normalized = typeof editorId === "string" ? editorId : DEFAULT_APP_SETTINGS.defaultTextEditor;
  return TEXT_EDITOR_OPTIONS.some((editor) => editor.id === normalized) ? normalized : DEFAULT_APP_SETTINGS.defaultTextEditor;
}

function getTextEditorOption(editorId) {
  const normalized = normalizeTextEditorId(editorId);
  return TEXT_EDITOR_OPTIONS.find((editor) => editor.id === normalized) || TEXT_EDITOR_OPTIONS[0];
}

function loadAppSettings() {
  try {
    const settingsPath = getAppSettingsPath();
    if (!fs.existsSync(settingsPath)) {
      return DEFAULT_APP_SETTINGS;
    }

    return normalizeAppSettings(JSON.parse(fs.readFileSync(settingsPath, "utf8")));
  } catch {
    return DEFAULT_APP_SETTINGS;
  }
}

function saveAppSettings() {
  fs.writeFileSync(getAppSettingsPath(), JSON.stringify(appSettings, null, 2));
}

function setDefaultTextEditor(editorId) {
  appSettings = normalizeAppSettings({
    ...appSettings,
    defaultTextEditor: editorId,
  });
  saveAppSettings();
  return appSettings;
}

function setAutoCopyTranscripts(enabled) {
  appSettings = normalizeAppSettings({
    ...appSettings,
    autoCopyTranscripts: Boolean(enabled),
  });
  saveAppSettings();
  return appSettings;
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
    setMacDockIcon();
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
