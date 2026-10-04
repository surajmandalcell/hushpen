const { BrowserWindow, screen } = require("electron");
const {
  OVERLAY_WINDOW_SIZE,
  createRecordingOverlayHtml,
  normalizeOverlaySettings,
  resolveOverlayBounds,
  RECORDING_SHORTCUT,
} = require("../runtime.cjs");
const { getModelById } = require("../whisper-engine.cjs");
const { secureWebPreferences } = require("./webPreferences.cjs");

const DRAG_SAVE_DELAY_MS = 250;

function createOverlayController({ ctx }) {
  let positioning = false;
  let dragSaveTimer;

  function getOverlaySettings() {
    return normalizeOverlaySettings({
      placement: ctx.settings.get("overlay.placement"),
      customBounds: ctx.settings.get("overlay.customBounds"),
    });
  }

  function resolveBounds() {
    return resolveOverlayBounds({
      settings: getOverlaySettings(),
      primaryDisplay: screen.getPrimaryDisplay(),
      displays: screen.getAllDisplays(),
      width: OVERLAY_WINDOW_SIZE.width,
      height: OVERLAY_WINDOW_SIZE.height,
    });
  }

  function position() {
    const win = ctx.windows.overlay;
    if (!win || win.isDestroyed()) return;

    clearTimeout(dragSaveTimer);
    dragSaveTimer = undefined;
    positioning = true;
    win.setBounds({
      ...resolveBounds(),
      ...OVERLAY_WINDOW_SIZE,
    });
    setTimeout(() => {
      positioning = false;
    }, 80);
  }

  function savePosition() {
    dragSaveTimer = undefined;
    const win = ctx.windows.overlay;
    if (!win || win.isDestroyed()) return;

    const bounds = win.getBounds();
    const display = screen.getDisplayMatching(bounds);
    try {
      ctx.settings.set("overlay.customBounds", { displayId: display.id, x: bounds.x, y: bounds.y }, { source: "main" });
    } catch (error) {
      ctx.log.error("settings", "INTERNAL", `Overlay position could not be saved: ${error.message}`);
    }
  }

  // A drag fires a move event per pixel; each save is an fsynced write, so coalesce them.
  function persistDraggedPosition() {
    const win = ctx.windows.overlay;
    if (positioning || !win || win.isDestroyed()) return;

    clearTimeout(dragSaveTimer);
    dragSaveTimer = setTimeout(savePosition, DRAG_SAVE_DELAY_MS);
  }

  function updateWaveformFrame(frame) {
    const normalizedFrame = Array.isArray(frame)
      ? frame.slice(0, 80).map((value) => Math.min(Math.max(Number(value) || 0, 0), 1))
      : [];

    ctx.state.lastWaveformFrame = normalizedFrame;

    const win = ctx.windows.overlay;
    if (!win || win.isDestroyed()) return;

    win.webContents.send("overlay:waveform-frame", normalizedFrame);
  }

  function show() {
    const existing = ctx.windows.overlay;
    if (existing && !existing.isDestroyed()) {
      position();
      existing.showInactive();
      return;
    }

    const { width, height } = OVERLAY_WINDOW_SIZE;
    const bounds = resolveBounds();

    const win = new BrowserWindow({
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
      webPreferences: secureWebPreferences("overlay-preload.cjs"),
    });
    ctx.windows.overlay = win;

    win.setAlwaysOnTop(true, "screen-saver");
    win.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true });
    win.setIgnoreMouseEvents(false);
    win.once("ready-to-show", () => {
      win.showInactive();
      updateWaveformFrame(ctx.state.lastWaveformFrame);
    });
    win.on("move", persistDraggedPosition);
    win.on("closed", () => {
      if (ctx.windows.overlay === win) ctx.windows.overlay = undefined;
    });
    win.loadURL(`data:text/html;charset=UTF-8,${encodeURIComponent(createRecordingOverlayHtml({
      modelName: getModelById(ctx.settings.get("transcription.modelId")).displayName,
      shortcut: RECORDING_SHORTCUT,
    }))}`);
  }

  function hide() {
    if (dragSaveTimer) {
      clearTimeout(dragSaveTimer);
      savePosition();
    }
    const win = ctx.windows.overlay;
    if (win && !win.isDestroyed()) {
      win.close();
    }
    ctx.windows.overlay = undefined;
  }

  return { getOverlaySettings, hide, position, show, updateWaveformFrame };
}

module.exports = { createOverlayController };
