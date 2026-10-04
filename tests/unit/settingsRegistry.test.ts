import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";

const require = createRequire(import.meta.url);
const { createSettingsRegistry } = require("../../electron/settings/registry.cjs");
const defaults = require("../../shared/settings-defaults.json") as Record<string, unknown>;

let dir: string;
afterEach(() => rmSync(dir, { recursive: true, force: true }));

function makeLog() {
  return { error: vi.fn(), warn: vi.fn() };
}

function open(files: Record<string, string> = {}) {
  dir = dir && existsSync(dir) ? dir : mkdtempSync(join(tmpdir(), "asrpro-settings-"));
  for (const [name, contents] of Object.entries(files)) writeFileSync(join(dir, name), contents);
  const log = makeLog();
  return { registry: createSettingsRegistry({ configDir: dir, log }), log };
}

const settingsFile = () => JSON.parse(readFileSync(join(dir, "settings.json"), "utf8"));

describe("settings registry", () => {
  it("writes schema v2 defaults on first run", () => {
    const { registry } = open();

    expect(registry.getAll()).toEqual(defaults);
    expect(settingsFile().schemaVersion).toBe(2);
    expect(settingsFile().values["output.autoCopy"]).toBe(true);
  });

  it("persists a change across a restart and leaves no temporary file", () => {
    const first = open().registry;
    first.set("output.autoCopy", false);
    first.set("overlay.placement", "bottom");

    const second = open().registry;

    expect(second.get("output.autoCopy")).toBe(false);
    expect(second.get("overlay.placement")).toBe("bottom");
    expect(readdirSync(dir).sort()).toEqual(["settings.json"]);
  });

  it("backs up a corrupt file, regenerates defaults, and logs it", () => {
    const { registry, log } = open({ "settings.json": "{not json" });

    expect(registry.getAll()).toEqual(defaults);
    expect(readdirSync(dir).filter((name) => name.startsWith("settings.json.corrupt-"))).toHaveLength(1);
    expect(settingsFile().schemaVersion).toBe(2);
    expect(log.error).toHaveBeenCalledWith("settings", "SETTINGS_INVALID", expect.stringContaining("corrupt"));
  });

  it("treats a wrong-shaped file as corrupt", () => {
    const { registry } = open({ "settings.json": JSON.stringify({ schemaVersion: "two", values: [] }) });

    expect(registry.getAll()).toEqual(defaults);
    expect(readdirSync(dir).some((name) => name.includes(".corrupt-"))).toBe(true);
  });

  it("keeps unknown keys through a save round-trip", () => {
    open({ "settings.json": JSON.stringify({
      schemaVersion: 2,
      values: { "output.autoCopy": false, "future.feature": { keep: "me" } },
    }) });
    const { registry } = open();

    registry.set("overlay.placement", "bottom");

    expect(settingsFile().values._unknown).toEqual({ "future.feature": { keep: "me" } });
    expect(open().registry.get("output.autoCopy")).toBe(false);
    expect(settingsFile().values._unknown).toEqual({ "future.feature": { keep: "me" } });
  });

  it("resets invalid stored values to their defaults on load", () => {
    const { registry, log } = open({ "settings.json": JSON.stringify({
      schemaVersion: 2,
      values: {
        "output.autoCopy": "yes",
        "editor.defaultTextEditor": "emacs",
        "engine.threads": 99,
        "overlay.placement": "left",
        "transcription.modelId": "whisper-small-en",
      },
    }) });

    expect(registry.get("output.autoCopy")).toBe(true);
    expect(registry.get("editor.defaultTextEditor")).toBe("system");
    expect(registry.get("engine.threads")).toBe("auto");
    expect(registry.get("overlay.placement")).toBe("top");
    expect(registry.get("transcription.modelId")).toBe("whisper-small-en");
    expect(log.warn).toHaveBeenCalledWith("settings", "SETTINGS_INVALID", expect.stringContaining("output.autoCopy"));
    expect(settingsFile().values["output.autoCopy"]).toBe(true);
  });

  it("rejects invalid values and unknown keys from the renderer with SETTINGS_INVALID", () => {
    const { registry } = open();

    expect(() => registry.check("output.autoCopy", "yes", { source: "renderer" })).toThrow(expect.objectContaining({ code: "SETTINGS_INVALID" }));
    expect(() => registry.check("not.a.key", true, { source: "renderer" })).toThrow(expect.objectContaining({ code: "SETTINGS_INVALID" }));
    expect(() => registry.check("migrations.legacyLocalStorage", true, { source: "renderer" })).toThrow(expect.objectContaining({ code: "SETTINGS_INVALID" }));
    expect(registry.get("output.autoCopy")).toBe(true);
  });

  it("keeps the old value when the write fails", () => {
    const { registry } = open();
    rmSync(dir, { recursive: true, force: true });
    writeFileSync(dir, "a file where the directory was");

    expect(() => registry.set("output.autoCopy", false)).toThrow(/ENOTDIR|ENOENT|EEXIST|not a directory/i);
    expect(registry.get("output.autoCopy")).toBe(true);
    rmSync(dir, { force: true });
  });

  it("notifies listeners with only the keys that changed", () => {
    const { registry } = open();
    const listener = vi.fn();
    registry.onChange(listener);

    registry.set("output.autoCopy", true);
    registry.update({ "output.autoCopy": false, "overlay.placement": "top" });

    expect(listener).toHaveBeenCalledTimes(1);
    expect(listener).toHaveBeenCalledWith(expect.objectContaining({ changed: ["output.autoCopy"] }));
  });
});

describe("legacy settings files", () => {
  it("migrates app-settings.json and overlay-settings.json once and keeps the originals as .migrated", () => {
    const { registry } = open({
      "app-settings.json": JSON.stringify({
        defaultTextEditor: "vscode",
        autoCopyTranscripts: false,
        launchAtStartup: true,
        startupExecutablePath: "/Applications/ASR Pro.app/Contents/MacOS/ASR Pro",
      }),
      "overlay-settings.json": JSON.stringify({ placement: "bottom", customBounds: { displayId: 1, x: 10, y: 20 } }),
    });

    expect(registry.get("editor.defaultTextEditor")).toBe("vscode");
    expect(registry.get("output.autoCopy")).toBe(false);
    expect(registry.get("startup.launchAtLogin")).toBe(true);
    expect(registry.get("startup.executablePath")).toBe("/Applications/ASR Pro.app/Contents/MacOS/ASR Pro");
    expect(registry.get("overlay.placement")).toBe("bottom");
    expect(registry.get("overlay.customBounds")).toEqual({ displayId: 1, x: 10, y: 20 });
    expect(readdirSync(dir).sort()).toEqual(["app-settings.json.migrated", "overlay-settings.json.migrated", "settings.json"]);

    // A second start reads settings.json and never imports the legacy files again.
    writeFileSync(join(dir, "app-settings.json"), JSON.stringify({ autoCopyTranscripts: true }));
    expect(open().registry.get("output.autoCopy")).toBe(false);
  });

  it("never deletes a legacy file it cannot understand", () => {
    const { registry, log } = open({ "app-settings.json": "not-json{" });

    expect(registry.getAll()).toEqual(defaults);
    expect(existsSync(join(dir, "app-settings.json"))).toBe(true);
    expect(log.error).toHaveBeenCalledWith("migration", "SETTINGS_INVALID", expect.stringContaining("app-settings.json"));
  });

  it("sanitizes legacy values with the same rules as stored values", () => {
    const { registry } = open({ "app-settings.json": JSON.stringify({ defaultTextEditor: "emacs", autoCopyTranscripts: "no" }) });

    expect(registry.get("editor.defaultTextEditor")).toBe("system");
    expect(registry.get("output.autoCopy")).toBe(true);
  });
});
