const { BrowserWindow } = require("electron");
const { APP_NAME } = require("../identity.cjs");
const { resolveAppIconPath } = require("../runtime.cjs");
const { MAIN_WINDOW_BACKGROUND, MAIN_WINDOW_SIZE } = require("../core/constants.cjs");
const { secureWebPreferences } = require("./webPreferences.cjs");

// The window must stay 780x520 on every platform (D-15). Each flag below is also
// re-applied after creation because some platforms ignore constructor flags.
function lockMainWindowSize(win, platform = process.platform) {
  win.setMinimumSize(MAIN_WINDOW_SIZE.width, MAIN_WINDOW_SIZE.height);
  win.setMaximumSize(MAIN_WINDOW_SIZE.width, MAIN_WINDOW_SIZE.height);
  win.setResizable(false);
  win.setMaximizable(false);
  win.setFullScreenable(false);

  // Electron ignores setMaximizable on Linux and isMaximizable() keeps answering
  // true, although the window cannot be maximized (not resizable, and the
  // "maximize" handler below undoes it). Report what the window really does.
  if (platform === "linux") win.isMaximizable = () => false;

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

function createMainWindowController({ ctx, getAssetRoot, appUrl }) {
  function create() {
    const existing = ctx.windows.main;
    if (existing && !existing.isDestroyed()) {
      return existing;
    }

    const win = new BrowserWindow({
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
      icon: resolveAppIconPath(ctx.platform, getAssetRoot()),
      backgroundColor: MAIN_WINDOW_BACKGROUND,
      webPreferences: secureWebPreferences("preload.cjs"),
    });
    ctx.windows.main = win;
    lockMainWindowSize(win, ctx.platform);

    win.once("ready-to-show", () => {
      win.show();
    });

    win.on("close", (event) => {
      if (!ctx.state.isQuitting) {
        event.preventDefault();
        win.hide();
      }
    });

    win.on("closed", () => {
      ctx.windows.main = undefined;
    });

    win.webContents.session.clearCache().catch(() => {});

    win.loadURL(appUrl);

    return win;
  }

  function show() {
    const win = create();
    if (win.isMinimized()) win.restore();
    win.show();
    win.focus();
  }

  return { create, show };
}

module.exports = { createMainWindowController, lockMainWindowSize };
