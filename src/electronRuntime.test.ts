import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

const require = createRequire(import.meta.url);
const runtime = require("../electron/runtime.cjs");

describe("Electron runtime helpers", () => {
  it("uses CommandOrControl+` as the global recording shortcut", () => {
    expect(runtime.RECORDING_SHORTCUT).toBe("CommandOrControl+`");
  });

  it("keeps the main window fixed-size and removes maximize entry points", () => {
    const mainSource = readFileSync("electron/main.cjs", "utf8");
    const preloadSource = readFileSync("electron/preload.cjs", "utf8");

    expect(mainSource).toContain("const MAIN_WINDOW_SIZE = { width: 780, height: 520 };");
    expect(mainSource).toContain('const SCREENSHOT_MODE = process.env.ASRPRO_SCREENSHOT_MODE === "1";');
    expect(mainSource).toMatch(
      /mainWindow = new BrowserWindow\(\{[\s\S]*?width: MAIN_WINDOW_SIZE\.width,[\s\S]*?height: MAIN_WINDOW_SIZE\.height,[\s\S]*?minWidth: MAIN_WINDOW_SIZE\.width,[\s\S]*?minHeight: MAIN_WINDOW_SIZE\.height,[\s\S]*?maxWidth: MAIN_WINDOW_SIZE\.width,[\s\S]*?maxHeight: MAIN_WINDOW_SIZE\.height,[\s\S]*?resizable: false,[\s\S]*?maximizable: false,[\s\S]*?fullscreenable: false,/
    );
    expect(mainSource).not.toContain('role: "zoom"');
    expect(mainSource).not.toContain('action === "maximize"');
    expect(mainSource).not.toContain("senderWindow.maximize()");
    expect(mainSource).toContain("lockMainWindowSize(mainWindow)");
    expect(mainSource).toContain("setMinimumSize(MAIN_WINDOW_SIZE.width, MAIN_WINDOW_SIZE.height)");
    expect(mainSource).toContain("setMaximumSize(MAIN_WINDOW_SIZE.width, MAIN_WINDOW_SIZE.height)");
    expect(mainSource).toContain('win.on("will-resize"');
    expect(mainSource).toContain("event.preventDefault()");
    expect(mainSource).toContain('win.on("maximize"');
    expect(mainSource).toContain("win.unmaximize()");
    expect(mainSource).toContain('win.on("enter-full-screen"');
    expect(mainSource).toContain("win.setFullScreen(false)");
    expect(mainSource).toContain('mode: "screenshot"');
    expect(mainSource).toContain("if (!SCREENSHOT_MODE)");
    expect(preloadSource).toContain('new Set(["minimize", "close"])');
    expect(preloadSource).not.toContain('"maximize"');
  });

  it("keeps the startup paint dark with a temporary loader until React mounts", () => {
    const indexSource = readFileSync("index.html", "utf8");
    const rendererEntrySource = readFileSync("src/main.tsx", "utf8");
    const mainSource = readFileSync("electron/main.cjs", "utf8");

    expect(indexSource).toContain('<html lang="en" class="dark">');
    expect(indexSource).toContain('id="app-loading-state"');
    expect(indexSource).toContain("Loading ASR Pro");
    expect(indexSource).toMatch(/html,\s*body,\s*#root[\s\S]*background:\s*#2f2f2f/);
    expect(rendererEntrySource).toContain('document.getElementById("app-loading-state")');
    expect(rendererEntrySource).toContain("requestAnimationFrame");
    expect(mainSource).toContain('const MAIN_WINDOW_BACKGROUND = "#2f2f2f";');
    expect(mainSource).toContain("backgroundColor: MAIN_WINDOW_BACKGROUND");
  });

  it("uses Whisper Base English as the default native model", () => {
    expect(runtime.DEFAULT_MODEL.id).toBe("whisper-base-en");
    expect(runtime.DEFAULT_MODEL.displayName).toBe("Whisper Base English");
    expect(runtime.DEFAULT_MODEL.fileName).toBe("ggml-base.en.bin");
    expect(runtime.AVAILABLE_MODELS.map((model: { id: string }) => model.id)).toEqual([
      "whisper-tiny-en",
      "whisper-base-en",
      "whisper-small-en",
      "whisper-base",
    ]);
  });

  it("uses Electron IPC for native Node Whisper transcription", () => {
    const mainSource = readFileSync("electron/main.cjs", "utf8");
    const preloadSource = readFileSync("electron/preload.cjs", "utf8");

    expect(mainSource).toContain('ipcMain.handle("engine:transcribe-audio"');
    expect(preloadSource).toContain("transcribeAudio");
    expect(mainSource.indexOf("createWindow();")).toBeLessThan(mainSource.indexOf("registerGlobalShortcut();"));
  });

  it("renders the recording overlay as a text-free waveform pill", () => {
    const html = runtime.createRecordingOverlayHtml();

    expect(runtime.OVERLAY_WINDOW_SIZE).toEqual({ width: 156, height: 40 });
    expect(html).toContain('class="surface"');
    expect(html).toContain('class="waveform"');
    expect(html).toContain("width: 148px");
    expect(html).toContain("width: 132px");
    expect(html.match(/data-base=/g)?.length).toBe(44);
    expect(html).toContain("now - lastInputAt > 260");
    expect(html).toContain("desired > current[index] ? 0.34 : 0.2");
    expect(html).toContain("Math.sin(now * 0.018");
    expect(html).toContain("rgba(18, 18, 20, 0.94)");
    expect(html).toContain("rgba(230, 230, 234, var(--bar-opacity))");
    expect(html).toContain("asrproSetWaveformFrame");
    expect(html).toContain("asrproOverlay?.onWaveformFrame");
    expect(html).toContain("requestAnimationFrame(renderWaveform)");
    expect(html).not.toContain("rgba(245, 245, 247");
    expect(html).not.toContain("rgba(255, 255, 255");
    expect(html).not.toContain("executeJavaScript");
    expect(html).not.toContain("transition: height");
    expect(html).not.toContain("animation:");
    expect(html).not.toContain("Recording now");
    expect(html).not.toContain("CommandOrControl");
  });

  it("shows the global recording overlay for every recording start source", () => {
    expect(runtime.shouldShowRecordingOverlay("renderer")).toBe(true);
    expect(runtime.shouldShowRecordingOverlay("shortcut")).toBe(true);
    expect(runtime.shouldShowRecordingOverlay("tray")).toBe(true);
    expect(runtime.shouldShowRecordingOverlay("menu")).toBe(true);
  });

  it("resolves packaged data under writable app data when available", () => {
    const dataDir = runtime.resolveContainedDataDir({
      isPackaged: true,
      platform: "darwin",
      resourcesPath: "/Applications/ASR Pro.app/Contents/Resources",
      exePath: "/Applications/ASR Pro.app/Contents/MacOS/ASR Pro",
      appPath: "/project",
      userDataPath: "/Users/suraj/Library/Application Support/ASR Pro",
    });

    expect(dataDir).toBe("/Users/suraj/Library/Application Support/ASR Pro/data");
  });

  it("uses an explicit data directory override for isolated automation runs", () => {
    const dataDir = runtime.resolveContainedDataDir({
      isPackaged: false,
      platform: "darwin",
      resourcesPath: "/repo/resources",
      exePath: "/repo/node_modules/electron/dist/Electron.app/Contents/MacOS/Electron",
      appPath: "/repo",
      userDataPath: "/Users/suraj/Library/Application Support/ASR Pro",
      dataDirOverride: "/private/tmp/asrpro-readme-capture-1234",
    });

    expect(dataDir).toBe("/private/tmp/asrpro-readme-capture-1234");
  });

  it("resolves packaged Windows and Linux data under user-writable app data", () => {
    expect(runtime.resolveContainedDataDir({
      isPackaged: true,
      platform: "win32",
      resourcesPath: "C:\\ASR Pro\\resources",
      exePath: "C:\\ASR Pro\\ASR Pro.exe",
      appPath: "C:\\repo",
      userDataPath: "C:\\Users\\suraj\\AppData\\Roaming\\ASR Pro",
    })).toBe("C:\\Users\\suraj\\AppData\\Roaming\\ASR Pro\\data");

    expect(runtime.resolveContainedDataDir({
      isPackaged: true,
      platform: "linux",
      resourcesPath: "/opt/asrpro/resources",
      exePath: "/opt/asrpro/asrpro",
      appPath: "/repo",
      userDataPath: "/home/suraj/.config/ASR Pro",
    })).toBe("/home/suraj/.config/ASR Pro/data");
  });

  it("resolves Whisper model cache paths under app data", () => {
    const paths = runtime.buildModelPaths("/Users/suraj/Library/Application Support/ASR Pro/data");

    expect(paths.modelsDir).toBe("/Users/suraj/Library/Application Support/ASR Pro/data/models");
    expect(paths.whisperModelsDir).toBe("/Users/suraj/Library/Application Support/ASR Pro/data/models/whisper");
    expect(paths.defaultModelPath).toBe("/Users/suraj/Library/Application Support/ASR Pro/data/models/whisper/ggml-base.en.bin");
  });

  it("resolves platform app and tray icon assets", () => {
    const assetRoot = "/repo/src/assets";

    expect(runtime.resolveTrayIconPath("darwin", assetRoot)).toBe(path.join(assetRoot, "asrpro-tray-dark.png"));
    expect(runtime.resolveTrayIconPath("win32", assetRoot, false)).toBe(path.join(assetRoot, "asrpro-tray-dark.png"));
    expect(runtime.resolveTrayIconPath("win32", assetRoot, true)).toBe(path.join(assetRoot, "asrpro-tray-light.png"));
    expect(runtime.resolveTrayIconPath("linux", assetRoot, false)).toBe(path.join(assetRoot, "asrpro-tray-dark.png"));
    expect(runtime.resolveTrayIconPath("linux", assetRoot, true)).toBe(path.join(assetRoot, "asrpro-tray-light.png"));
    expect(runtime.resolveAppIconPath("darwin", assetRoot)).toBe(path.join(assetRoot, "asrpro-app-icon.png"));
    expect(runtime.resolveAppIconPath("win32", assetRoot)).toBe(path.join(assetRoot, "asrpro-app-icon.ico"));
    expect(runtime.resolveAppIconPath("linux", assetRoot)).toBe(path.join(assetRoot, "asrpro-app-icon.png"));
  });

  it("resolves runtime asset roots for development and packaged apps", () => {
    expect(runtime.resolveRuntimeAssetRoot({
      isPackaged: false,
      resourcesPath: "/repo/resources",
      appPath: "/repo",
    })).toBe(path.join("/repo", "src", "assets"));

    expect(runtime.resolveRuntimeAssetRoot({
      isPackaged: true,
      resourcesPath: "/Applications/ASR Pro.app/Contents/Resources",
      appPath: "/repo",
    })).toBe(path.join("/Applications/ASR Pro.app/Contents/Resources", "assets"));
  });

  it("places the recording overlay at the bottom of the selected work area", () => {
    const bounds = runtime.resolveOverlayBounds({
      settings: { placement: "bottom" },
      primaryDisplay: {
        id: 1,
        workArea: { x: 10, y: 30, width: 1400, height: 820 },
      },
      displays: [],
      width: 420,
      height: 84,
    });

    expect(bounds).toEqual({
      x: 500,
      y: 750,
    });
  });

  it("remembers a dragged overlay position on the matching monitor", () => {
    const bounds = runtime.resolveOverlayBounds({
      settings: {
        placement: "top",
        customBounds: {
          displayId: 7,
          x: 2030,
          y: 620,
        },
      },
      primaryDisplay: {
        id: 1,
        workArea: { x: 0, y: 25, width: 1440, height: 850 },
      },
      displays: [
        { id: 1, workArea: { x: 0, y: 25, width: 1440, height: 850 } },
        { id: 7, workArea: { x: 1440, y: 0, width: 1728, height: 1117 } },
      ],
      width: 420,
      height: 84,
    });

    expect(bounds).toEqual({
      x: 2030,
      y: 620,
    });
  });
});
