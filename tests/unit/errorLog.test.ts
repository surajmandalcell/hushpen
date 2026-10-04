import { mkdtempSync, readFileSync, readdirSync, rmSync, statSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";

const require = createRequire(import.meta.url);
const { createLog, DEFAULT_MAX_BYTES, DEFAULT_MAX_FILES } = require("../../electron/core/log.cjs") as {
  createLog: (options: { dir: string; maxBytes?: number; maxFiles?: number; now?: () => Date }) => {
    filePath: string;
    error: (scope: string, code: string, detail?: string) => void;
    warn: (scope: string, code: string, detail?: string) => void;
  };
  DEFAULT_MAX_BYTES: number;
  DEFAULT_MAX_FILES: number;
};

let dir: string;
afterEach(() => rmSync(dir, { recursive: true, force: true }));
const fresh = () => {
  dir = mkdtempSync(join(tmpdir(), "asrpro-log-"));
  return dir;
};

describe("error log", () => {
  it("rotates at about 2 MB and keeps 3 files", () => {
    expect(DEFAULT_MAX_BYTES).toBe(2 * 1024 * 1024);
    expect(DEFAULT_MAX_FILES).toBe(3);

    const log = createLog({ dir: fresh() });
    const detail = "x".repeat(480);
    // About 12 MB of entries, far beyond three files of 2 MB.
    for (let index = 0; index < 24_000; index += 1) log.error("ipc", "INTERNAL", detail);

    const files = readdirSync(dir).sort();
    expect(files).toEqual(["asrpro.log", "asrpro.log.1", "asrpro.log.2"]);
    for (const file of files) {
      expect(statSync(join(dir, file)).size).toBeLessThanOrEqual(DEFAULT_MAX_BYTES);
    }
  });

  it("writes one line per entry with a timestamp, level, scope, and code", () => {
    const log = createLog({ dir: fresh(), now: () => new Date("2026-01-02T03:04:05.000Z") });
    log.error("migration", "SETTINGS_INVALID", "first\nsecond");
    log.warn("not a scope!", "OFFLINE");

    expect(readFileSync(log.filePath, "utf8").split("\n").filter(Boolean)).toEqual([
      "2026-01-02T03:04:05.000Z ERROR migration SETTINGS_INVALID first second",
      "2026-01-02T03:04:05.000Z WARN app OFFLINE ",
    ]);
  });

  it("caps the detail so a long message cannot carry a transcript", () => {
    const log = createLog({ dir: fresh() });
    log.error("engine", "ENGINE_CRASHED", "y".repeat(5000));

    expect(readFileSync(log.filePath, "utf8").length).toBeLessThan(700);
  });

  it("never throws when the directory cannot be written", () => {
    const log = createLog({ dir: "/dev/null/asrpro-logs" });
    expect(() => log.error("ipc", "INTERNAL", "ignored")).not.toThrow();
  });
});
