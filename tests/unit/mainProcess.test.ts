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
