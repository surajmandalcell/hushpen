import { createRequire } from "node:module";
import * as nodePath from "node:path";
import { describe, expect, it } from "vitest";

const require = createRequire(import.meta.url);
const { sanitizeTranscriptFileName, isPathInside, resolveTranscriptDeletePath, getTranscriptTextPath } = require(
  "../../electron/ipc/transcripts.cjs",
) as {
  sanitizeTranscriptFileName: (value?: string) => string;
  isPathInside: (parentDir: string, candidatePath: string) => boolean;
  getTranscriptTextPath: (transcriptDir: string, title: string) => string;
  resolveTranscriptDeletePath: (transcriptDir: string, request?: { title?: string; filePath?: string }) => string;
};
const { isTrustedMediaPermission } = require("../../electron/windows/mediaPermissions.cjs") as {
  isTrustedMediaPermission: (
    webContents: { getURL?: () => string } | undefined,
    permission: string,
    details: { mediaType?: string; mediaTypes?: string[]; requestingUrl?: string; requestingOrigin?: string; securityOrigin?: string },
    devServerUrl: string,
  ) => boolean;
};

const devServerUrl = "http://127.0.0.1:4270";

describe("transcript file name guard", () => {
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

  it("resolves a title to a text file inside the transcript directory", () => {
    expect(resolveTranscriptDeletePath(transcriptDir, {})).toBe(nodePath.join(transcriptDir, "Transcript.txt"));
    expect(resolveTranscriptDeletePath(transcriptDir, { title: "My Notes" })).toBe(nodePath.join(transcriptDir, "My Notes.txt"));
    expect(resolveTranscriptDeletePath(transcriptDir, { filePath: "  " })).toBe(nodePath.join(transcriptDir, "Transcript.txt"));
    expect(getTranscriptTextPath(transcriptDir, "a/b")).toBe(nodePath.join(transcriptDir, "a b.txt"));
  });

  it("accepts a file path inside the transcript directory", () => {
    expect(resolveTranscriptDeletePath(transcriptDir, { filePath: nodePath.join(transcriptDir, "a.txt") }))
      .toBe(nodePath.join(transcriptDir, "a.txt"));
    expect(isPathInside(transcriptDir, nodePath.join(transcriptDir, "nested", "a.txt"))).toBe(true);
  });

  it("rejects paths outside the transcript directory with a coded error", () => {
    const outside = () => resolveTranscriptDeletePath(transcriptDir, { filePath: "/etc/passwd" });
    expect(outside).toThrow(expect.objectContaining({ code: "INVALID_ARGUMENT" }));
    expect(() => resolveTranscriptDeletePath(transcriptDir, { filePath: nodePath.join(transcriptDir, "..", "escape.txt") }))
      .toThrow(expect.objectContaining({ code: "INVALID_ARGUMENT" }));
  });
});

describe("media permission check", () => {
  it("allows audio media for the app and dev origins", () => {
    expect(isTrustedMediaPermission(undefined, "media", { requestingUrl: "file:///app/dist/index.html", mediaType: "audio" }, devServerUrl)).toBe(true);
    expect(isTrustedMediaPermission(undefined, "media", { requestingOrigin: devServerUrl }, devServerUrl)).toBe(true);
    expect(isTrustedMediaPermission({ getURL: () => "file:///app/dist/index.html" }, "media", {}, devServerUrl)).toBe(true);
  });

  it("denies non-audio media types", () => {
    expect(isTrustedMediaPermission(undefined, "media", { requestingUrl: "file:///app/dist/index.html", mediaType: "video" }, devServerUrl)).toBe(false);
    expect(isTrustedMediaPermission(undefined, "media", { requestingUrl: "file:///app/dist/index.html", mediaTypes: ["video", "audio"] }, devServerUrl)).toBe(false);
    expect(isTrustedMediaPermission(undefined, "media", { requestingUrl: "file:///app/dist/index.html", mediaTypes: ["audio"] }, devServerUrl)).toBe(true);
  });

  it("denies other permissions and untrusted origins", () => {
    expect(isTrustedMediaPermission(undefined, "clipboard-read", { requestingUrl: "file:///app/dist/index.html" }, devServerUrl)).toBe(false);
    expect(isTrustedMediaPermission(undefined, "clipboard-sanitized-write", { requestingUrl: "file:///app/dist/index.html" }, devServerUrl)).toBe(false);
    expect(isTrustedMediaPermission(undefined, "media", { requestingUrl: "https://example.com" }, devServerUrl)).toBe(false);
    expect(isTrustedMediaPermission(undefined, "media", {}, devServerUrl)).toBe(false);
  });
});
