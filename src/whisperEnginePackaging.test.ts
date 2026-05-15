import { existsSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import packageMetadata from "../package.json";

describe("ASR engine development scripts", () => {
  it("uses the native Whisper check as the active engine path", () => {
    expect(packageMetadata.scripts["engine:check"]).toBe("node scripts/check-whisper-engine.cjs");
    expect(packageMetadata.dependencies["@kutalia/whisper-node-addon"]).toBeTruthy();
  });

  it("packages the native Whisper addon", () => {
    const packageSource = readFileSync("package.json", "utf8");

    expect(packageSource).toContain("node_modules/@kutalia/whisper-node-addon/**/*");
    expect(packageSource).toContain("asarUnpack");
    expect(packageSource).toContain("**/*.node");
    expect(packageSource).toContain('"afterPack": "scripts/after-pack-cleanup.cjs"');
  });

  it("launches the Electron shell without waiting for the lazy engine", () => {
    const electronDevScript = packageMetadata.scripts["electron:dev"];

    expect(electronDevScript).toContain("wait-on http://127.0.0.1:4270");
  });

  it("checks the native Whisper addon dependency", () => {
    const scriptPath = "scripts/check-whisper-engine.cjs";

    expect(existsSync(scriptPath)).toBe(true);

    const scriptSource = readFileSync(scriptPath, "utf8");

    expect(scriptSource).toContain("@kutalia/whisper-node-addon");
  });

  it("removes unused native addon platforms after packaging", () => {
    const scriptPath = "scripts/after-pack-cleanup.cjs";

    expect(existsSync(scriptPath)).toBe(true);

    const scriptSource = readFileSync(scriptPath, "utf8");

    expect(scriptSource).toContain("@kutalia");
    expect(scriptSource).toContain("whisper-node-addon");
    expect(scriptSource).toContain("app.asar.unpacked");
  });
});
