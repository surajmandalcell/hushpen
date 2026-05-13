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
  const waveformBars = buildOverlayWaveformBars(55);
  const barsHtml = waveformBars.map((height, index) => (
    `<span data-base="${height}" style="--bar-height:${height}px;--bar-opacity:${edgeOpacity(index, waveformBars.length)}"></span>`
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
        will-change: height, background;
      }
    </style>
  </head>
  <body>
    <div class="surface" role="status" aria-label="ASR Pro recording overlay">
      <span class="waveform" aria-hidden="true">${barsHtml}</span>
    </div>
    <script>
      (() => {
        const bars = Array.from(document.querySelectorAll(".waveform span"));
        const bases = bars.map((bar) => Number(bar.dataset.base) || 8);
        const clamp = (value, min, max) => Math.min(Math.max(value, min), max);
        const current = bases.map(() => 0);
        const target = bases.map(() => 0);
        let animationFrame = 0;
        let lastInputAt = 0;

        const renderWaveform = () => {
          const now = performance.now();
          const shouldDecay = lastInputAt === 0 || now - lastInputAt > 180;
          let shouldContinue = false;

          bars.forEach((bar, index) => {
            const desired = shouldDecay ? 0 : target[index];
            const response = desired > current[index] ? 0.48 : 0.3;
            current[index] += (desired - current[index]) * response;

            if (Math.abs(current[index] - desired) < 0.002) {
              current[index] = desired;
            }

            const base = bases[index];
            const energy = clamp(current[index], 0, 1);
            const ripple = energy * 0.045 * Math.sin(now * 0.026 + index * 0.72);
            const level = clamp(energy + ripple, 0, 1);
            const floor = clamp(base * 0.72, 4, 14);
            const height = energy > 0.002 || desired > 0.002 ? clamp(floor + level * (20 - floor), 4, 20) : base;
            const opacity = energy > 0.002 ? clamp(0.42 + level * 0.54, 0.34, 0.96) : Number(bar.dataset.opacity) || 0.72;

            if (energy > 0.002 || desired > 0.002) {
              shouldContinue = true;
            }

            bar.style.setProperty("--bar-height", height.toFixed(2) + "px");
            bar.style.setProperty("--bar-opacity", opacity.toFixed(3));
          });

          if (shouldContinue) {
            animationFrame = requestAnimationFrame(renderWaveform);
          } else {
            animationFrame = 0;
          }
        };

        const startWaveform = () => {
          if (!animationFrame) {
            animationFrame = requestAnimationFrame(renderWaveform);
          }
        };

        const setWaveformFrame = (samples) => {
          const values = Array.isArray(samples) ? samples : [];
          lastInputAt = values.length > 0 ? performance.now() : 0;

          bars.forEach((bar, index) => {
            target[index] = clamp(Number(values[index] || 0), 0, 1);
          });

          startWaveform();
        };

        bars.forEach((bar) => {
          bar.dataset.opacity = bar.style.getPropertyValue("--bar-opacity") || "0.72";
        });

        window.asrproSetWaveformFrame = setWaveformFrame;
        window.asrproOverlay?.onWaveformFrame?.(setWaveformFrame);
      })();
    </script>
  </body>
</html>`;
}

function buildOverlayWaveformBars(count) {
  return Array.from({ length: count }, (_, index) => {
    const position = index / Math.max(1, count - 1);
    const envelope = 0.42 + 0.58 * (1 - Math.abs(position - 0.5) * 1.35);
    const speechShape = 0.36
      + 0.28 * Math.sin(index * 1.73)
      + 0.18 * Math.sin(index * 0.59 + 1.8)
      + 0.11 * Math.sin(index * 2.47 + 0.3);

    return Math.round(clampNumber(6 + 12 * envelope * speechShape, 5, 18));
  });
}

function edgeOpacity(index, count) {
  const distance = Math.min(index, count - 1 - index);
  if (distance < 3) return 0.28 + distance * 0.12;
  return 0.76;
}

function clampNumber(value, min, max) {
  return Math.min(Math.max(value, min), max);
}

function shouldShowRecordingOverlay(source) {
  return source !== "renderer";
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
  shouldShowRecordingOverlay,
};
