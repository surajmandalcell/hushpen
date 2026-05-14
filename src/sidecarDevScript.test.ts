import { existsSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import packageMetadata from "../package.json";

describe("ASR engine development scripts", () => {
  it("prepares the Python engine environment before launching dev", () => {
    const sidecarDevScript = packageMetadata.scripts["sidecar:dev"];

    expect(sidecarDevScript).toBe("node scripts/ensure-sidecar-env.cjs --run");
    expect(sidecarDevScript).not.toContain("else python3 sidecar/main.py");
  });

  it("keeps the engine dependency preflight tied to requirements.txt", () => {
    const scriptPath = "scripts/ensure-sidecar-env.cjs";

    expect(existsSync(scriptPath)).toBe(true);

    const scriptSource = readFileSync(scriptPath, "utf8");

    expect(scriptSource).toContain("sidecar");
    expect(scriptSource).toContain("requirements.txt");
    expect(scriptSource).toContain(".asrpro-requirements.sha256");
    expect(scriptSource).toContain("fastapi");
    expect(scriptSource).toContain("nemo.collections.asr");
    expect(scriptSource).toContain("torch");
    expect(scriptSource).toContain("main.py");
  });
});
