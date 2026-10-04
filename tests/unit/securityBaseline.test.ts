import { readFileSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { describe, expect, it } from "vitest";

const require = createRequire(import.meta.url);
const { secureWebPreferences } = require("../../electron/windows/webPreferences.cjs") as {
  secureWebPreferences: (preload: string) => Record<string, unknown>;
};

function listSources(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const full = path.join(directory, entry.name);
    if (entry.isDirectory()) return listSources(full);
    return entry.name.endsWith(".cjs") ? [full] : [];
  });
}

describe("window web preferences", () => {
  it("sandbox both windows with isolated contexts and no Node access", () => {
    for (const preload of ["preload.cjs", "overlay-preload.cjs"]) {
      expect(secureWebPreferences(preload)).toMatchObject({
        contextIsolation: true,
        nodeIntegration: false,
        nodeIntegrationInSubFrames: false,
        sandbox: true,
        webSecurity: true,
        webviewTag: false,
        allowRunningInsecureContent: false,
      });
    }
  });

  it("points at the preload file next to the electron entry", () => {
    expect(secureWebPreferences("preload.cjs").preload).toBe(path.resolve("electron/preload.cjs"));
  });

  it("is the only source of web preferences for the main window and the overlay", () => {
    for (const file of ["electron/windows/mainWindow.cjs", "electron/windows/overlayWindow.cjs"]) {
      const source = readFileSync(file, "utf8");
      expect(source).toContain("secureWebPreferences(");
      expect(source).not.toMatch(/sandbox:\s*false/);
    }
  });
});

describe("IPC entry points", () => {
  it("are registered only through the router, which checks every sender", () => {
    const offenders = listSources("electron")
      .filter((file) => !file.endsWith(path.join("ipc", "router.cjs")))
      .filter((file) => /\bipcMain\b/.test(readFileSync(file, "utf8")));

    expect(offenders).toEqual([]);
  });
});

describe("sandboxed preloads", () => {
  it.each(["electron/preload.cjs", "electron/overlay-preload.cjs"])("%s requires nothing but electron", (file) => {
    const required = Array.from(readFileSync(file, "utf8").matchAll(/require\(["']([^"']+)["']\)/g), (match) => match[1]);

    expect(new Set(required)).toEqual(new Set(["electron"]));
  });
});

describe("external navigation", () => {
  it("never hands a page URL to the system browser outside the allow-listed open targets", () => {
    const offenders = listSources("electron")
      .filter((file) => !file.endsWith(path.join("shell", "openTargets.cjs")))
      .filter((file) => /openExternal\(/.test(readFileSync(file, "utf8")));

    expect(offenders).toEqual([]);
  });
});
