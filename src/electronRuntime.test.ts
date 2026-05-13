import { createRequire } from "node:module";
import { describe, expect, it } from "vitest";

const require = createRequire(import.meta.url);
const runtime = require("../electron/runtime.cjs");

describe("Electron runtime helpers", () => {
  it("uses CommandOrControl+` as the global recording shortcut", () => {
    expect(runtime.RECORDING_SHORTCUT).toBe("CommandOrControl+`");
  });

  it("uses Parakeet-TDT-0.6B-v3 as the default model", () => {
    expect(runtime.DEFAULT_MODEL.id).toBe("parakeet-tdt-0.6b-v3");
    expect(runtime.DEFAULT_MODEL.displayName).toBe("Parakeet-TDT-0.6B-v3");
    expect(runtime.DEFAULT_MODEL.repo).toBe("nvidia/parakeet-tdt-0.6b-v3");
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
