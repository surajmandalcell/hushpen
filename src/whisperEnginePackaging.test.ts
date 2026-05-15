import { existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createRequire } from "node:module";
import { describe, expect, it } from "vitest";
import packageMetadata from "../package.json";

const require = createRequire(import.meta.url);
const afterPackCleanup = require("../scripts/after-pack-cleanup.cjs") as (context: {
  appOutDir: string;
  arch: number | string;
  electronPlatformName: string;
}) => Promise<void>;

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

  it("builds Linux release artifacts as x64 from the Make target", () => {
    const dryRun = execFileSync("make", ["-n", "build:linux"], { encoding: "utf8" });

    expect(dryRun).toContain("node scripts/build-electron.cjs linux --x64");
    expect(dryRun).toContain("/opt/homebrew/opt/binutils/bin");
    expect(dryRun).toContain("/usr/local/opt/binutils/bin");
  });

  it("uses distinct Windows artifact names for installer and portable builds", () => {
    expect(packageMetadata.build.nsis.artifactName).toContain("Setup");
    expect(packageMetadata.build.portable.artifactName).toContain("Portable");
    expect(packageMetadata.build.nsis.artifactName).not.toBe(packageMetadata.build.portable.artifactName);
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

  it("keeps x64 native addon binaries when Electron Builder passes numeric arch values", async () => {
    const appOutDir = join(tmpdir(), `asrpro-after-pack-${Date.now()}`);
    const distDir = join(
      appOutDir,
      "resources",
      "app.asar.unpacked",
      "node_modules",
      "@kutalia",
      "whisper-node-addon",
      "dist",
    );

    try {
      for (const dirname of ["js", "linux-arm64", "linux-x64", "mac-arm64", "win32-x64"]) {
        mkdirSync(join(distDir, dirname), { recursive: true });
      }

      await afterPackCleanup({
        appOutDir,
        arch: 1,
        electronPlatformName: "linux",
      });

      expect(existsSync(join(distDir, "js"))).toBe(true);
      expect(existsSync(join(distDir, "linux-x64"))).toBe(true);
      expect(existsSync(join(distDir, "linux-arm64"))).toBe(false);
      expect(existsSync(join(distDir, "mac-arm64"))).toBe(false);
      expect(existsSync(join(distDir, "win32-x64"))).toBe(false);
    } finally {
      rmSync(appOutDir, { force: true, recursive: true });
    }
  });
});
