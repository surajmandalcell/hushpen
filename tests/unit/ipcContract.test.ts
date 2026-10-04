import { readFileSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { describe, expect, it } from "vitest";

const require = createRequire(import.meta.url);
const channels = JSON.parse(readFileSync("shared/ipc-channels.json", "utf8")).channels as Record<string, { kind: string; roles: string[] }>;
const errorCodes = JSON.parse(readFileSync("shared/error-codes.json", "utf8")).codes as Record<string, string>;

function preloadSet(name: string) {
  const source = readFileSync("electron/preload.cjs", "utf8");
  const match = source.match(new RegExp(`const ${name} = new Set\\(\\[([\\s\\S]*?)\\]\\);`));
  if (!match) throw new Error(`${name} not found in preload`);
  return Array.from(match[1].matchAll(/"([^"]+)"/g), (entry) => entry[1]).sort();
}

const namesOfKind = (kind: string, role = "main-window") => (
  Object.entries(channels).filter(([, entry]) => entry.kind === kind && entry.roles.includes(role)).map(([name]) => name).sort()
);

function listSources(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const full = path.join(directory, entry.name);
    if (entry.isDirectory()) return listSources(full);
    return /\.(cjs|ts|tsx)$/.test(entry.name) && !/\.test\./.test(entry.name) ? [full] : [];
  });
}

describe("shared IPC contract", () => {
  it("exposes exactly the channels listed in shared/ipc-channels.json from the preload", () => {
    expect(preloadSet("INVOKE_CHANNELS")).toEqual(namesOfKind("invoke"));
    expect(preloadSet("SEND_CHANNELS")).toEqual(namesOfKind("send"));
    expect(preloadSet("PUSH_CHANNELS")).toEqual(namesOfKind("push"));
  });

  it("registers a handler for every invoke and send channel, and only for declared ones", () => {
    const registered = new Map<string, string>();
    for (const file of listSources("electron/ipc")) {
      const source = readFileSync(file, "utf8");
      for (const match of source.matchAll(/router\.(handle|on)\("([^"]+)"/g)) {
        registered.set(match[2], match[1] === "handle" ? "invoke" : "send");
      }
    }

    const declared = Object.entries(channels)
      .filter(([, entry]) => entry.kind === "invoke" || entry.kind === "send")
      .map(([name, entry]) => [name, entry.kind]);
    expect([...registered.entries()].sort()).toEqual(declared.sort());
  });

  it("gives every error code a group and keeps INTERNAL and INVALID_ARGUMENT", () => {
    expect(Object.keys(errorCodes)).toEqual(expect.arrayContaining(["INTERNAL", "INVALID_ARGUMENT", "FORBIDDEN_SENDER", "SETTINGS_INVALID"]));
    for (const [code, group] of Object.entries(errorCodes)) {
      expect(code).toMatch(/^[A-Z][A-Z0-9_]+$/);
      expect(typeof group).toBe("string");
    }
  });

  it("rejects error codes that are not in shared/error-codes.json", () => {
    const { AppError } = require("../../electron/core/errors.cjs");
    expect(() => new AppError("NOT_A_REAL_CODE")).toThrow(TypeError);
  });

  it("offers no maximize or fullscreen entry point", () => {
    const preload = readFileSync("electron/preload.cjs", "utf8");
    expect(preload).not.toContain("maximize");
    expect(preload).not.toContain("fullscreen");
  });

  it("matches no English error text in the renderer or the main process", () => {
    const englishPatterns = [
      /\/[^/\n]*(download|checksum|ENOENT|no handler registered|failed to fetch|NetworkError|addon|Cannot find module)[^/\n]*\/[gimsuy]*\.test\(/i,
      /\.(message|detail)\??\.(includes|match|startsWith|test)\(/,
      /Error invoking remote method/,
    ];
    const files = [...listSources("src"), ...listSources("electron")].filter((file) => !file.includes("/test/"));
    const hits = files.filter((file) => {
      const source = readFileSync(file, "utf8");
      return englishPatterns.some((pattern) => pattern.test(source));
    });
    expect(hits).toEqual([]);
  });
});
