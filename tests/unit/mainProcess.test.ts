import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import * as nodeFs from "node:fs";
import * as nodePath from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { evaluateDeclarations, extractDeclaration, readElectronSources } from "../helpers/electronSources";

const require = createRequire(import.meta.url);
const runtime = require("../../electron/runtime.cjs") as {
  DEFAULT_OVERLAY_SETTINGS: { placement: string; customBounds: unknown };
  normalizeOverlaySettings: (value?: unknown) => { placement: string; customBounds: unknown };
};

const { main } = readElectronSources();

const extract = (name: string) => extractDeclaration(main, name);

type SanitizeTranscriptFileName = (value?: string) => string;
type IsPathInside = (parentDir: string, candidatePath: string) => boolean;
type ResolveTranscriptDeletePath = (request?: { title?: string; filePath?: string }) => string;
type IsTrustedMediaPermission = (
  webContents: { getURL?: () => string } | undefined,
  permission: string,
  details?: { mediaType?: string; mediaTypes?: string[]; requestingUrl?: string; requestingOrigin?: string; securityOrigin?: string },
) => boolean;
interface AppSettings {
  defaultTextEditor: string;
  autoCopyTranscripts: boolean;
  launchAtStartup: boolean;
  startupExecutablePath: string;
}

describe("transcript file name guard", () => {
  const { sanitizeTranscriptFileName } = evaluateDeclarations<{ sanitizeTranscriptFileName: SanitizeTranscriptFileName }>(
    [extract("RESERVED_TRANSCRIPT_FILE_NAME_CHARACTERS"), extract("sanitizeTranscriptFileName")],
    {},
    ["sanitizeTranscriptFileName"],
  );

  it("replaces reserved and control characters with spaces", () => {
    expect(sanitizeTranscriptFileName("My \"best\" <talk>: part 1/2\\3|4?5*6")).toBe("My best talk part 1 2 3 4 5 6");
    expect(sanitizeTranscriptFileName("a\nb\tc")).toBe("a b c");
  });

  it("falls back to a default name for empty input", () => {
    expect(sanitizeTranscriptFileName("")).toBe("transcript");
    expect(sanitizeTranscriptFileName("   ")).toBe("transcript");
    expect(sanitizeTranscriptFileName()).toBe("transcript");
  });

  it("caps the file name at 80 characters and keeps unicode", () => {
    expect(sanitizeTranscriptFileName("a".repeat(100))).toHaveLength(80);
    expect(sanitizeTranscriptFileName("café 東京")).toBe("café 東京");
  });
});

describe("transcript file path guard", () => {
  const transcriptDir = "/data/transcripts";
  const fns = evaluateDeclarations<{
    isPathInside: IsPathInside;
    resolveTranscriptDeletePath: ResolveTranscriptDeletePath;
  }>(
    [
      extract("RESERVED_TRANSCRIPT_FILE_NAME_CHARACTERS"),
      extract("sanitizeTranscriptFileName"),
      extract("getTranscriptTextPath"),
      extract("isPathInside"),
      extract("resolveTranscriptDeletePath"),
    ],
    { path: nodePath, getTranscriptDir: () => transcriptDir },
    ["isPathInside", "resolveTranscriptDeletePath"],
  );

  it("resolves a title to a text file inside the transcript directory", () => {
    expect(fns.resolveTranscriptDeletePath({})).toBe(nodePath.join(transcriptDir, "Transcript.txt"));
    expect(fns.resolveTranscriptDeletePath({ title: "My Notes" })).toBe(nodePath.join(transcriptDir, "My Notes.txt"));
    expect(fns.resolveTranscriptDeletePath({ filePath: "  " })).toBe(nodePath.join(transcriptDir, "Transcript.txt"));
  });

  it("accepts a file path inside the transcript directory", () => {
    expect(fns.resolveTranscriptDeletePath({ filePath: nodePath.join(transcriptDir, "a.txt") }))
      .toBe(nodePath.join(transcriptDir, "a.txt"));
  });

  it("rejects paths outside the transcript directory", () => {
    expect(() => fns.resolveTranscriptDeletePath({ filePath: "/etc/passwd" })).toThrow(/outside ASR Pro data/);
    expect(() => fns.resolveTranscriptDeletePath({ filePath: nodePath.join(transcriptDir, "..", "escape.txt") }))
      .toThrow(/outside ASR Pro data/);
  });
});

describe("media permission check", () => {
  const { isTrustedMediaPermission } = evaluateDeclarations<{ isTrustedMediaPermission: IsTrustedMediaPermission }>(
    [
      extract("DEV_SERVER_URL"),
      extract("isTrustedAppUrl"),
      extract("isTrustedMediaPermission"),
    ],
    {},
    ["isTrustedMediaPermission"],
  );

  it("allows audio media for the app and dev origins", () => {
    expect(isTrustedMediaPermission(undefined, "media", { requestingUrl: "file:///app/dist/index.html", mediaType: "audio" })).toBe(true);
    expect(isTrustedMediaPermission(undefined, "media", { requestingOrigin: "http://127.0.0.1:4270" })).toBe(true);
    expect(isTrustedMediaPermission({ getURL: () => "file:///app/dist/index.html" }, "media", {})).toBe(true);
  });

  it("denies non-audio media types", () => {
    expect(isTrustedMediaPermission(undefined, "media", { requestingUrl: "file:///app/dist/index.html", mediaType: "video" })).toBe(false);
    expect(isTrustedMediaPermission(undefined, "media", { requestingUrl: "file:///app/dist/index.html", mediaTypes: ["video", "audio"] })).toBe(false);
    expect(isTrustedMediaPermission(undefined, "media", { requestingUrl: "file:///app/dist/index.html", mediaTypes: ["audio"] })).toBe(true);
  });

  it("denies other permissions and untrusted origins", () => {
    expect(isTrustedMediaPermission(undefined, "clipboard-read", { requestingUrl: "file:///app/dist/index.html" })).toBe(false);
    expect(isTrustedMediaPermission(undefined, "clipboard-sanitized-write", { requestingUrl: "file:///app/dist/index.html" })).toBe(false);
    expect(isTrustedMediaPermission(undefined, "media", { requestingUrl: "https://example.com" })).toBe(false);
    expect(isTrustedMediaPermission(undefined, "media", {})).toBe(false);
  });
});

describe("app settings normalisation", () => {
  let settingsDir: string;
  let settingsPath: string;

  const load = () => {
    const fns = evaluateDeclarations<{
      DEFAULT_APP_SETTINGS: AppSettings;
      loadAppSettings: () => AppSettings;
    }>(
      [
        extract("TEXT_EDITOR_OPTIONS"),
        extract("DEFAULT_APP_SETTINGS"),
        extract("normalizeBooleanSetting"),
        extract("normalizeTextEditorId"),
        extract("normalizeStartupExecutablePath"),
        extract("normalizeAppSettings"),
        extract("loadAppSettings"),
      ],
      { fs: nodeFs, getAppSettingsPath: () => settingsPath },
      ["DEFAULT_APP_SETTINGS", "loadAppSettings"],
    );
    return fns;
  };

  afterEach(() => {
    if (settingsDir) rmSync(settingsDir, { recursive: true, force: true });
  });

  const useSettingsFile = (contents?: string) => {
    settingsDir = mkdtempSync(join(tmpdir(), "asrpro-settings-"));
    settingsPath = join(settingsDir, "app-settings.json");
    if (contents !== undefined) writeFileSync(settingsPath, contents, "utf8");
  };

  it("returns defaults when the settings file is missing", () => {
    useSettingsFile();

    const { DEFAULT_APP_SETTINGS, loadAppSettings } = load();

    expect(loadAppSettings()).toEqual({
      defaultTextEditor: "system",
      autoCopyTranscripts: true,
      launchAtStartup: false,
      startupExecutablePath: "",
    });
    expect(loadAppSettings()).toBe(DEFAULT_APP_SETTINGS);
  });

  it("returns defaults when the settings file is corrupt", () => {
    useSettingsFile("not-json{");

    const { loadAppSettings } = load();

    expect(loadAppSettings()).toEqual({
      defaultTextEditor: "system",
      autoCopyTranscripts: true,
      launchAtStartup: false,
      startupExecutablePath: "",
    });
  });

  it("normalizes a partial settings file against the defaults", () => {
    useSettingsFile(JSON.stringify({
      defaultTextEditor: "vscode",
      autoCopyTranscripts: false,
      launchAtStartup: "yes",
      unknownFutureKey: { keep: "me" },
    }));

    const { loadAppSettings } = load();

    expect(loadAppSettings()).toEqual({
      defaultTextEditor: "vscode",
      autoCopyTranscripts: false,
      launchAtStartup: false,
      startupExecutablePath: "",
    });
  });

  it("rejects an unknown text editor id", () => {
    useSettingsFile(JSON.stringify({ defaultTextEditor: "emacs" }));

    expect(load().loadAppSettings().defaultTextEditor).toBe("system");
  });
});

describe("overlay settings normalisation", () => {
  let settingsDir: string;
  let settingsPath: string;

  const load = () => evaluateDeclarations<{
    loadOverlaySettings: () => { placement: string; customBounds: unknown };
  }>(
    [extract("loadOverlaySettings")],
    {
      fs: nodeFs,
      getOverlaySettingsPath: () => settingsPath,
      DEFAULT_OVERLAY_SETTINGS: runtime.DEFAULT_OVERLAY_SETTINGS,
      normalizeOverlaySettings: runtime.normalizeOverlaySettings,
    },
    ["loadOverlaySettings"],
  );

  afterEach(() => {
    if (settingsDir) rmSync(settingsDir, { recursive: true, force: true });
  });

  it("returns defaults for a corrupt overlay settings file", () => {
    settingsDir = mkdtempSync(join(tmpdir(), "asrpro-overlay-"));
    settingsPath = join(settingsDir, "overlay-settings.json");
    writeFileSync(settingsPath, "{broken", "utf8");

    expect(load().loadOverlaySettings()).toEqual({ placement: "top", customBounds: null });
  });

  it("keeps a valid placement and repairs an invalid one", () => {
    settingsDir = mkdtempSync(join(tmpdir(), "asrpro-overlay-"));
    settingsPath = join(settingsDir, "overlay-settings.json");
    writeFileSync(settingsPath, JSON.stringify({ placement: "bottom", customBounds: { displayId: 1, x: 10.4, y: 20 } }), "utf8");

    expect(load().loadOverlaySettings()).toEqual({ placement: "bottom", customBounds: { displayId: 1, x: 10, y: 20 } });

    writeFileSync(settingsPath, JSON.stringify({ placement: "left" }), "utf8");
    expect(load().loadOverlaySettings()).toEqual({ placement: "top", customBounds: null });
  });
});
