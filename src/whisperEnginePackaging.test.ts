import { existsSync, readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
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

  it("packages platform-specific app and tray icon assets", () => {
    const buildConfig = packageMetadata.build;
    const assetFilter = buildConfig.extraResources[0].filter;

    expect(buildConfig.mac.icon).toBe("src/assets/asrpro-app-icon.icns");
    expect(buildConfig.win.icon).toBe("src/assets/asrpro-app-icon.ico");
    expect(buildConfig.linux.icon).toBe("src/assets/linux-icons");
    expect(buildConfig.linux.target).toEqual(["AppImage", "deb", "rpm", "tar.gz"]);
    expect(assetFilter).toEqual(expect.arrayContaining([
      "asrpro-app-icon.icns",
      "asrpro-app-icon.ico",
      "asrpro-app-icon.png",
      "asrpro-tray-dark.ico",
      "asrpro-tray-light.ico",
      "asrpro-tray-dark.png",
      "asrpro-tray-light.png",
    ]));
  });

  it("keeps the app icon padded and on the selected Ink Slate background", () => {
    const iconSource = readFileSync("src/assets/asrpro-app-icon.svg", "utf8");

    expect(iconSource).toContain('id="ink-slate"');
    expect(iconSource).toContain('data-icon-artwork="padded"');
    expect(iconSource).toContain('transform="translate(72 72) scale(0.859375)"');
    expect(iconSource).toContain('stop-color="#20272d"');
    expect(iconSource).toContain('stop-color="#10171d"');
    expect(iconSource).toContain('stop-color="#04070a"');
    expect(iconSource).toContain('stroke="#eef4f5"');
  });

  it("launches the Electron shell without waiting for the lazy engine", () => {
    const electronDevScript = packageMetadata.scripts["electron:dev"];

    expect(electronDevScript).toContain("wait-on http://127.0.0.1:4270");
  });

  it("builds Windows release artifacts as x64 from the Make target", () => {
    const dryRun = execFileSync("make", ["-n", "build:win"], { encoding: "utf8" });

    expect(dryRun).toContain("node scripts/build-electron.cjs win --x64");
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
