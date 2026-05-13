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
});
