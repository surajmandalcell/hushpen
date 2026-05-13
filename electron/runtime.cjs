const path = require("node:path");

const RECORDING_SHORTCUT = "CommandOrControl+`";

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

function createRecordingOverlayHtml({ modelName = DEFAULT_MODEL.displayName, shortcut = RECORDING_SHORTCUT } = {}) {
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
      }
      .pill {
        display: inline-flex;
        align-items: center;
        gap: 10px;
        height: 38px;
        padding: 0 14px;
        border-radius: 999px;
        color: #1d1d1f;
        background: rgba(255, 255, 255, 0.88);
        border: 1px solid rgba(0, 0, 0, 0.12);
        box-shadow: 0 14px 34px rgba(0, 0, 0, 0.18), inset 0 1px 0 rgba(255, 255, 255, 0.72);
        backdrop-filter: blur(18px) saturate(1.25);
        -webkit-backdrop-filter: blur(18px) saturate(1.25);
      }
      .dot {
        width: 9px;
        height: 9px;
        border-radius: 999px;
        background: #ff3b30;
        box-shadow: 0 0 0 4px rgba(255, 59, 48, 0.14);
      }
      .main {
        font-size: 13px;
        font-weight: 700;
        letter-spacing: 0;
      }
      .meta {
        font-size: 11px;
        color: #6e6e73;
        font-weight: 600;
      }
    </style>
  </head>
  <body>
    <div class="pill" role="status" aria-label="ASR Pro recording overlay">
      <span class="dot"></span>
      <span class="main">Recording</span>
      <span class="meta">${escapeHtml(modelName)} - ${escapeHtml(shortcut)}</span>
    </div>
  </body>
</html>`;
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
  RECORDING_SHORTCUT,
  buildModelPaths,
  createRecordingOverlayHtml,
  resolveContainedDataDir,
  resolveTrayIconPath,
};
