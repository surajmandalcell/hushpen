import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";

const require = createRequire(import.meta.url);
const { readJsonFile, writeFileAtomic } = require("../../electron/core/atomicFile.cjs") as {
  readJsonFile: (filePath: string, options?: { isValid?: (value: unknown) => boolean; now?: () => number }) => {
    status: string;
    value?: unknown;
    backupPath?: string;
  };
  writeFileAtomic: (filePath: string, contents: string) => void;
};

let dir: string;
afterEach(() => rmSync(dir, { recursive: true, force: true }));
const fresh = () => {
  dir = mkdtempSync(join(tmpdir(), "asrpro-atomic-"));
  return dir;
};

describe("writeFileAtomic", () => {
  it("replaces the file in one step and leaves no temporary file", () => {
    const target = join(fresh(), "nested", "settings.json");
    writeFileAtomic(target, "one");
    writeFileAtomic(target, "two");

    expect(readFileSync(target, "utf8")).toBe("two");
    expect(readdirSync(join(dir, "nested"))).toEqual(["settings.json"]);
  });

  it("keeps the previous contents and removes the temporary file when the write fails", () => {
    const target = join(fresh(), "settings.json");
    writeFileAtomic(target, "kept");

    expect(() => writeFileAtomic(target, 42 as unknown as string)).toThrow(/argument/i);
    expect(readFileSync(target, "utf8")).toBe("kept");
    expect(readdirSync(dir)).toEqual(["settings.json"]);
  });
});

describe("readJsonFile", () => {
  it("reports a missing file without touching the disk", () => {
    expect(readJsonFile(join(fresh(), "none.json"))).toEqual({ status: "missing" });
    expect(readdirSync(dir)).toEqual([]);
  });

  it("backs up a file that is not valid JSON", () => {
    const target = join(fresh(), "settings.json");
    writeFileSync(target, "not-json{");

    const result = readJsonFile(target, { now: () => 42 });

    expect(result.status).toBe("corrupt");
    expect(result.backupPath).toBe(`${target}.corrupt-42`);
    expect(readFileSync(`${target}.corrupt-42`, "utf8")).toBe("not-json{");
    expect(readdirSync(dir)).toEqual(["settings.json.corrupt-42"]);
  });

  it("backs up valid JSON that fails the shape check", () => {
    const target = join(fresh(), "settings.json");
    writeFileSync(target, "[]");

    expect(readJsonFile(target, { isValid: (value) => !Array.isArray(value), now: () => 7 }).status).toBe("corrupt");
    expect(readdirSync(dir)).toEqual(["settings.json.corrupt-7"]);
  });
});
