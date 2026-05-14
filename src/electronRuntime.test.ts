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

  it("uses Parakeet-TDT-0.6B-v3 as the default model", () => {
    expect(runtime.DEFAULT_MODEL.id).toBe("parakeet-tdt-0.6b-v3");
    expect(runtime.DEFAULT_MODEL.displayName).toBe("Parakeet-TDT-0.6B-v3");
    expect(runtime.DEFAULT_MODEL.repo).toBe("nvidia/parakeet-tdt-0.6b-v3");
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
    expect(html).not.toContain("Parakeet-TDT");
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

  it("builds a packaged sidecar launch config from bundled executables", () => {
    const windowsExecutable = "C:\\ASR Pro\\resources\\sidecar\\bin\\asrpro-sidecar.exe";
    const linuxExecutable = "/opt/asrpro/resources/sidecar/bin/asrpro-sidecar";
    const existing = new Set([windowsExecutable, linuxExecutable]);
    const existsSync = (candidate: string) => existing.has(candidate);

    expect(runtime.buildSidecarLaunchConfig({
      isPackaged: true,
      platform: "win32",
      resourcesPath: "C:\\ASR Pro\\resources",
      appPath: "C:\\repo",
      existsSync,
    })).toMatchObject({
      mode: "executable",
      command: windowsExecutable,
      args: [],
      healthUrl: "http://127.0.0.1:8000/health",
    });

    expect(runtime.buildSidecarLaunchConfig({
      isPackaged: true,
      platform: "linux",
      resourcesPath: "/opt/asrpro/resources",
      appPath: "/repo",
      existsSync,
    })).toMatchObject({
      mode: "executable",
      command: linuxExecutable,
      args: [],
      healthUrl: "http://127.0.0.1:8000/health",
    });
  });

  it("uses Python source only for development sidecar launches", () => {
    const mainPath = "/repo/sidecar/main.py";
    const existsSync = (candidate: string) => candidate === mainPath;

    expect(runtime.buildSidecarLaunchConfig({
      isPackaged: false,
      platform: "darwin",
      resourcesPath: "/repo/resources",
      appPath: "/repo",
      pythonCommand: "/repo/sidecar/.venv/bin/python",
      existsSync,
    })).toMatchObject({
      mode: "python",
      command: "/repo/sidecar/.venv/bin/python",
      args: [mainPath],
      cwd: "/repo/sidecar",
    });

    expect(runtime.buildSidecarLaunchConfig({
      isPackaged: true,
      platform: "linux",
      resourcesPath: "/opt/asrpro/resources",
      appPath: "/repo",
      existsSync: () => false,
    })).toMatchObject({
      mode: "missing",
      command: null,
      error: "Packaged ASR Pro is missing its bundled Python sidecar executable.",
    });
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
