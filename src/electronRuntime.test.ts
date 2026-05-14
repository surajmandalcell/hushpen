import { createRequire } from "node:module";
import path from "node:path";
import { describe, expect, it } from "vitest";

const require = createRequire(import.meta.url);
const runtime = require("../electron/runtime.cjs");

describe("Electron runtime helpers", () => {
  it("uses CommandOrControl+` as the global recording shortcut", () => {
    expect(runtime.RECORDING_SHORTCUT).toBe("CommandOrControl+`");
  });

  it("uses the working local Whisper model as the default model", () => {
    expect(runtime.DEFAULT_MODEL.id).toBe("whisper-base");
    expect(runtime.DEFAULT_MODEL.displayName).toBe("Local Whisper");
    expect(runtime.DEFAULT_MODEL.repo).toBe("onnx-asr/whisper-base");
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

  it("resolves contained data inside the macOS app bundle when packaged", () => {
    const dataDir = runtime.resolveContainedDataDir({
      isPackaged: true,
      platform: "darwin",
      resourcesPath: "/Applications/ASR Pro.app/Contents/Resources",
      exePath: "/Applications/ASR Pro.app/Contents/MacOS/ASR Pro",
      appPath: "/project",
    });

    expect(dataDir).toBe("/Applications/ASR Pro.app/Contents/Resources/data");
  });

  it("resolves contained data beside the executable on Windows and Linux", () => {
    expect(runtime.resolveContainedDataDir({
      isPackaged: true,
      platform: "win32",
      resourcesPath: "C:\\ASR Pro\\resources",
      exePath: "C:\\ASR Pro\\ASR Pro.exe",
      appPath: "C:\\repo",
    })).toBe("C:\\ASR Pro\\data");

    expect(runtime.resolveContainedDataDir({
      isPackaged: true,
      platform: "linux",
      resourcesPath: "/opt/asrpro/resources",
      exePath: "/opt/asrpro/asrpro",
      appPath: "/repo",
    })).toBe("/opt/asrpro/data");
  });

  it("resolves platform app and tray icon assets", () => {
    const projectRoot = "/repo";

    expect(runtime.resolveTrayIconPath("darwin", projectRoot)).toBe(path.join(projectRoot, "src-tauri", "icons", "trayTemplate.png"));
    expect(runtime.resolveTrayIconPath("win32", projectRoot, false)).toBe(path.join(projectRoot, "src-tauri", "icons", "tray-dark.png"));
    expect(runtime.resolveTrayIconPath("win32", projectRoot, true)).toBe(path.join(projectRoot, "src-tauri", "icons", "tray-light.png"));
    expect(runtime.resolveTrayIconPath("linux", projectRoot, false)).toBe(path.join(projectRoot, "src-tauri", "icons", "tray-dark.png"));
    expect(runtime.resolveTrayIconPath("linux", projectRoot, true)).toBe(path.join(projectRoot, "src-tauri", "icons", "tray-light.png"));
    expect(runtime.resolveAppIconPath("darwin", projectRoot)).toBe(path.join(projectRoot, "src-tauri", "icons", "icon.png"));
    expect(runtime.resolveAppIconPath("win32", projectRoot)).toBe(path.join(projectRoot, "src-tauri", "icons", "icon.ico"));
    expect(runtime.resolveAppIconPath("linux", projectRoot)).toBe(path.join(projectRoot, "src-tauri", "icons", "icon.png"));
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
