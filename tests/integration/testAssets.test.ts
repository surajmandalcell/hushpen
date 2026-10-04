import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const assetsDir = process.env.ASRPRO_TEST_ASSETS;

describe.skipIf(!assetsDir)("ASR Pro test assets", () => {
  it("provides a checksum manifest that covers real fixtures", () => {
    const manifestPath = join(assetsDir as string, "MANIFEST.sha256");

    expect(existsSync(manifestPath)).toBe(true);

    const manifest = readFileSync(manifestPath, "utf8");

    expect(manifest).toContain("speech-short.wav");
    expect(manifest).toContain("dictation-45s.wav");
  });
});
