const path = require("node:path");

const RECORDING_SHORTCUT = "CommandOrControl+`";
const OVERLAY_WINDOW_SIZE = {
  width: 188,
  height: 40,
};
const OVERLAY_EDGE_MARGIN = 16;
const DEFAULT_OVERLAY_SETTINGS = Object.freeze({
  placement: "top",
  customBounds: null,
});

const DEFAULT_MODEL = {
  id: "parakeet-tdt-0.6b-v3",
  displayName: "Parakeet-TDT-0.6B-v3",
  repo: "nvidia/parakeet-tdt-0.6b-v3",
};

function platformPath(platform) {
  return platform === "win32" ? path.win32 : path.posix;
}

function resolveContainedDataDir({ isPackaged, platform, resourcesPath, exePath, appPath }) {
  const pathModule = platformPath(platform);

  if (!isPackaged) {
    return pathModule.join(appPath, "tmp", "app-data");
  }

  if (platform === "darwin") {
    return pathModule.join(resourcesPath, "data");
  }

  return pathModule.join(pathModule.dirname(exePath), "data");
}

function resolveTrayIconPath(platform, projectRoot) {
  if (platform === "darwin") return path.join(projectRoot, "src-tauri", "icons", "32x32.png");
  if (platform === "win32") return path.join(projectRoot, "src-tauri", "icons", "icon.ico");
  return path.join(projectRoot, "src-tauri", "icons", "32x32.png");
}

function buildModelPaths(dataDir) {
  const modelsDir = path.join(dataDir, "models");
  const defaultModelDir = path.join(modelsDir, DEFAULT_MODEL.id);

  return {
    modelsDir,
    defaultModelDir,
    defaultModelManifest: path.join(defaultModelDir, "model.json"),
  };
}

function normalizeOverlaySettings(value = {}) {
  const placement = value && value.placement === "bottom" ? "bottom" : "top";
  const customBounds = normalizeCustomBounds(value && value.customBounds);

  return {
    placement,
    customBounds,
  };
}

function normalizeCustomBounds(value) {
  if (!value || typeof value !== "object") return null;

  const displayId = Number(value.displayId);
  const x = Number(value.x);
  const y = Number(value.y);

  if (!Number.isFinite(displayId) || !Number.isFinite(x) || !Number.isFinite(y)) {
    return null;
  }

  return {
    displayId,
    x: Math.round(x),
    y: Math.round(y),
  };
}

function resolveOverlayBounds({
  settings,
  primaryDisplay,
  displays = [],
  width = OVERLAY_WINDOW_SIZE.width,
  height = OVERLAY_WINDOW_SIZE.height,
  margin = OVERLAY_EDGE_MARGIN,
} = {}) {
  const normalized = normalizeOverlaySettings(settings);
  const customBounds = normalized.customBounds;
  const rememberedDisplay = customBounds
    ? displays.find((display) => String(display.id) === String(customBounds.displayId))
    : undefined;

  if (customBounds && rememberedDisplay) {
    const area = getDisplayWorkArea(rememberedDisplay);
    return {
      x: clamp(customBounds.x, area.x + margin / 2, area.x + area.width - width - margin / 2),
      y: clamp(customBounds.y, area.y + margin / 2, area.y + area.height - height - margin / 2),
    };
  }

  const display = primaryDisplay || displays[0] || {
    workArea: { x: 0, y: 0, width: 800, height: 600 },
  };
  const area = getDisplayWorkArea(display);

  return {
    x: Math.round(area.x + (area.width - width) / 2),
    y: normalized.placement === "bottom"
      ? Math.round(area.y + area.height - height - margin)
      : Math.round(area.y + margin),
  };
}

function getDisplayWorkArea(display) {
  const area = display && (display.workArea || display.bounds);

  return {
    x: Number(area && area.x) || 0,
    y: Number(area && area.y) || 0,
    width: Number(area && area.width) || 800,
    height: Number(area && area.height) || 600,
  };
}

function clamp(value, min, max) {
  if (max < min) return Math.round(min);
  return Math.round(Math.min(Math.max(value, min), max));
}

function createRecordingOverlayHtml() {
  const waveformPattern = [8, 12, 16, 10, 18, 14, 17, 11, 15, 9, 13, 16, 12, 18, 14, 10, 15, 8, 12, 16, 11, 17];
  const waveformBars = Array.from({ length: 55 }, (_, index) => waveformPattern[index % waveformPattern.length]);
  const barsHtml = waveformBars.map((height, index) => (
    `<span style="--bar-height:${height}px;--bar-opacity:${index < 4 || index > waveformBars.length - 5 ? 0.36 : 0.78}"></span>`
  )).join("");

  return `<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <style>
      html, body {
        margin: 0;
        background: transparent;
        overflow: hidden;
        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
        user-select: none;
        cursor: move;
      }
      body {
        width: ${OVERLAY_WINDOW_SIZE.width}px;
        height: ${OVERLAY_WINDOW_SIZE.height}px;
        display: inline-flex;
        align-items: center;
        justify-content: center;
      }
      .surface {
        position: relative;
        display: flex;
        align-items: center;
        justify-content: center;
        width: 180px;
        height: 32px;
        box-sizing: border-box;
        padding: 6px 8px;
        border-radius: 999px;
        color: #1d1d1f;
        background:
          linear-gradient(180deg, rgba(255, 255, 255, 0.68), rgba(245, 245, 247, 0.5)),
          rgba(245, 245, 247, 0.72);
        border: 1px solid rgba(0, 0, 0, 0.08);
        box-shadow:
          inset 0 1px 0 rgba(255, 255, 255, 0.72),
          inset 0 -1px 0 rgba(0, 0, 0, 0.04);
        backdrop-filter: saturate(180%) blur(20px);
        -webkit-backdrop-filter: saturate(180%) blur(20px);
        -webkit-app-region: drag;
      }
      .waveform {
        position: relative;
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 1px;
        width: 164px;
        height: 20px;
        overflow: hidden;
        mask-image: linear-gradient(90deg, transparent, #000 6%, #000 94%, transparent);
        -webkit-mask-image: linear-gradient(90deg, transparent, #000 6%, #000 94%, transparent);
      }
      .waveform span {
        width: 2px;
        height: var(--bar-height);
        border-radius: 999px;
        background: rgba(80, 80, 86, var(--bar-opacity));
        transform-origin: center;
      }
    </style>
  </head>
  <body>
    <div class="surface" role="status" aria-label="ASR Pro recording overlay">
      <span class="waveform" aria-hidden="true">${barsHtml}</span>
    </div>
  </body>
</html>`;
}

module.exports = {
  DEFAULT_MODEL,
  DEFAULT_OVERLAY_SETTINGS,
  OVERLAY_EDGE_MARGIN,
  OVERLAY_WINDOW_SIZE,
  RECORDING_SHORTCUT,
  buildModelPaths,
  createRecordingOverlayHtml,
  normalizeOverlaySettings,
  resolveContainedDataDir,
  resolveOverlayBounds,
  resolveTrayIconPath,
};
