const { app, Menu, nativeImage, nativeTheme } = require("electron");
const { APP_ID, APP_NAME, buildAboutPanelOptions } = require("./identity.cjs");
const { DEFAULT_MODEL, resolveAppIconPath, resolveRuntimeAssetRoot } = require("./runtime.cjs");
const { DEV_SERVER_URL, SCREENSHOT_MODE } = require("./core/constants.cjs");
const { createContext } = require("./core/context.cjs");
const { createRecordingController } = require("./core/recording.cjs");
const { createRouter } = require("./ipc/router.cjs");
const { registerAppIpc } = require("./ipc/app.cjs");
const { createEngineService, registerEngineIpc } = require("./ipc/engine.cjs");
const { registerRecordingIpc } = require("./ipc/recording.cjs");
const { createRuntimeState } = require("./ipc/runtimeState.cjs");
const { registerSettingsIpc } = require("./ipc/settings.cjs");
const { registerTranscriptIpc } = require("./ipc/transcripts.cjs");
const { createMenu } = require("./shell/menu.cjs");
const { registerGlobalShortcut, unregisterGlobalShortcuts } = require("./shell/shortcuts.cjs");
const { createStartup, resolveExecutablePath } = require("./shell/startup.cjs");
const { createTextEditors } = require("./shell/textEditors.cjs");
const { createTrayController } = require("./shell/tray.cjs");
const { createMainWindowController } = require("./windows/mainWindow.cjs");
const { configureMediaPermissions } = require("./windows/mediaPermissions.cjs");
const { createOverlayController } = require("./windows/overlayWindow.cjs");

app.commandLine.appendSwitch("enable-features", "GlobalShortcutsPortal");

const ctx = createContext({
  app,
  executablePath: resolveExecutablePath({ app, platform: process.platform, env: process.env }),
});

app.setPath("userData", ctx.layout.userDataDir);
app.setPath("sessionData", ctx.layout.sessionDir);
app.setPath("logs", ctx.layout.logsDir);
process.env.ASRPRO_DATA_DIR = ctx.dataDir;
process.env.ASRPRO_DEFAULT_MODEL = DEFAULT_MODEL.id;
process.env.XDG_CACHE_HOME = ctx.layout.cacheDir;

const hasSingleInstanceLock = app.requestSingleInstanceLock();

if (!hasSingleInstanceLock) {
  app.quit();
}

const getAssetRoot = () => resolveRuntimeAssetRoot({
  isPackaged: app.isPackaged,
  resourcesPath: process.resourcesPath,
  appPath: app.getAppPath(),
});

const mainWindow = createMainWindowController({ ctx, getAssetRoot });
const overlay = createOverlayController({ ctx });
const recording = createRecordingController({ ctx, overlay });
const textEditors = createTextEditors({ ctx });
const startup = createStartup({ ctx });

function quitApp() {
  ctx.state.isQuitting = true;
  overlay.hide();
  tray.destroy();
  app.quit();
}

const actions = {
  showMainWindow: () => mainWindow.show(),
  toggleRecording: (source) => recording.toggle(source),
  quit: quitApp,
};
const tray = createTrayController({ ctx, actions, getAssetRoot });
ctx.events.on("recording-changed", () => tray.updateMenu());

function registerIpc() {
  const router = createRouter({ ctx });
  const getRuntimeState = createRuntimeState({ ctx, recording, overlay, textEditors, startup });
  const engine = createEngineService({ ctx, getRuntimeState });

  registerAppIpc({ router, ctx, getRuntimeState });
  registerSettingsIpc({ router, ctx, overlay, startup });
  registerRecordingIpc({ router, recording, overlay });
  registerEngineIpc({ router, ctx, engine });
  registerTranscriptIpc({ router, ctx, textEditors });
  return engine;
}

function setMacDockIcon() {
  if (process.platform !== "darwin" || !app.dock) return;
  app.dock.setIcon(nativeImage.createFromPath(resolveAppIconPath(process.platform, getAssetRoot())));
}

app.setName(APP_NAME);
app.setAppUserModelId(APP_ID);
app.setAboutPanelOptions(buildAboutPanelOptions(app.getVersion()));

if (hasSingleInstanceLock) {
  app.on("second-instance", actions.showMainWindow);

  app.whenReady().then(() => {
    const engine = registerIpc();
    configureMediaPermissions({ devServerUrl: DEV_SERVER_URL });
    if (SCREENSHOT_MODE) {
      engine.setState({
        status: "ready",
        mode: "screenshot",
        modelId: DEFAULT_MODEL.id,
        model: DEFAULT_MODEL.displayName,
        progress: null,
        error: null,
      });
      Menu.setApplicationMenu(null);
    } else {
      Menu.setApplicationMenu(createMenu({ ctx, actions }));
    }
    setMacDockIcon();
    mainWindow.create();
    if (!SCREENSHOT_MODE) {
      ctx.state.shortcutRegistered = registerGlobalShortcut({ actions });
      tray.create();
      nativeTheme.on("updated", tray.updateIcon);
    }

    app.on("activate", actions.showMainWindow);
  });
}

app.on("window-all-closed", () => {
  if (ctx.state.isQuitting) app.quit();
});

app.on("before-quit", () => {
  ctx.state.isQuitting = true;
});

app.on("will-quit", () => {
  unregisterGlobalShortcuts();
  overlay.hide();
  tray.destroy();
});
