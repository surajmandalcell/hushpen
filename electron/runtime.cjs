const path = require("node:path");

const RECORDING_SHORTCUT = "CommandOrControl+`";
const OVERLAY_WINDOW_SIZE = {
  width: 420,
  height: 84,
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

function createRecordingOverlayHtml({ modelName = DEFAULT_MODEL.displayName, shortcut = RECORDING_SHORTCUT } = {}) {
  const waveformBars = [18, 36, 54, 28, 68, 44, 72, 38, 58, 32, 76, 48, 64, 26, 52, 34, 70, 42, 60, 30, 50, 24];
  const barsHtml = waveformBars.map((height, index) => (
    `<span style="--bar-height:${height}px;--bar-delay:${index * 42}ms"></span>`
  )).join("");
  const shortcutLabel = formatShortcutForOverlay(shortcut);

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
        display: grid;
        grid-template-columns: auto 1fr auto;
        align-items: center;
        gap: 8px;
        width: 398px;
        height: 62px;
        box-sizing: border-box;
        padding: 10px 12px;
        border-radius: 18px;
        color: #f8fafc;
        background:
          linear-gradient(135deg, rgba(15, 23, 42, 0.94), rgba(24, 31, 48, 0.9)),
          rgba(15, 23, 42, 0.88);
        border: 1px solid rgba(255, 255, 255, 0.18);
        box-shadow:
          0 20px 46px rgba(15, 23, 42, 0.34),
          0 8px 18px rgba(15, 23, 42, 0.24),
          inset 0 1px 0 rgba(255, 255, 255, 0.2);
        backdrop-filter: blur(22px) saturate(1.35);
        -webkit-backdrop-filter: blur(22px) saturate(1.35);
        -webkit-app-region: drag;
      }
      .live {
        display: grid;
        place-items: center;
        width: 38px;
        height: 38px;
        border-radius: 999px;
        background:
          radial-gradient(circle at 50% 50%, rgba(255, 99, 87, 0.95), rgba(255, 69, 58, 0.7) 42%, rgba(255, 69, 58, 0.12) 43%),
          rgba(255, 69, 58, 0.12);
        box-shadow: 0 0 0 1px rgba(255, 134, 125, 0.2), 0 0 28px rgba(255, 69, 58, 0.4);
      }
      .live::before {
        content: "";
        width: 10px;
        height: 10px;
        border-radius: 999px;
        background: #fff7f6;
        animation: pulse 1.45s ease-in-out infinite;
      }
      .copy {
        min-width: 0;
        display: grid;
        gap: 4px;
      }
      .main {
        width: max-content;
        max-width: 214px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        font-size: 14px;
        line-height: 1;
        font-weight: 750;
        letter-spacing: 0;
        color: transparent;
        background: linear-gradient(100deg, #dbeafe 0%, #ffffff 28%, #93c5fd 46%, #ffffff 64%, #dbeafe 100%);
        background-size: 220% 100%;
        -webkit-background-clip: text;
        background-clip: text;
        animation: shimmer 2.15s linear infinite;
      }
      .meta {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        font-size: 11px;
        line-height: 1;
        color: rgba(226, 232, 240, 0.72);
        font-weight: 600;
      }
      .waveform {
        position: relative;
        display: flex;
        align-items: center;
        justify-content: flex-end;
        gap: 3px;
        width: 116px;
        height: 42px;
        overflow: hidden;
        mask-image: linear-gradient(90deg, transparent, #000 16%, #000 84%, transparent);
        -webkit-mask-image: linear-gradient(90deg, transparent, #000 16%, #000 84%, transparent);
      }
      .waveform span {
        width: 3px;
        height: var(--bar-height);
        border-radius: 999px;
        background: linear-gradient(180deg, #bfdbfe 0%, #60a5fa 46%, #2563eb 100%);
        box-shadow: 0 0 10px rgba(96, 165, 250, 0.4);
        transform-origin: center;
        animation: wave 760ms ease-in-out infinite alternate;
        animation-delay: var(--bar-delay);
      }
      @keyframes shimmer {
        from { background-position: 200% 50%; }
        to { background-position: -20% 50%; }
      }
      @keyframes wave {
        from { transform: scaleY(0.42); opacity: 0.64; }
        to { transform: scaleY(1); opacity: 1; }
      }
      @keyframes pulse {
        0%, 100% { transform: scale(0.78); opacity: 0.82; }
        50% { transform: scale(1); opacity: 1; }
      }
      @media (prefers-reduced-motion: reduce) {
        .main,
        .waveform span,
        .live::before {
          animation: none;
        }
      }
    </style>
  </head>
  <body>
    <div class="surface" role="status" aria-label="ASR Pro recording overlay">
      <span class="live"></span>
      <span class="copy">
        <span class="main">Recording now</span>
        <span class="meta">${escapeHtml(modelName)} | ${escapeHtml(shortcutLabel)}</span>
      </span>
      <span class="waveform" aria-hidden="true">${barsHtml}</span>
    </div>
  </body>
</html>`;
}

function formatShortcutForOverlay(value) {
  return String(value).replace("CommandOrControl+", "Cmd/Ctrl ");
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
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
