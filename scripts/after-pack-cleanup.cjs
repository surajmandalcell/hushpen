const fs = require("node:fs");
const path = require("node:path");

function normalizeArch(arch) {
  if (arch === "arm64" || arch === 3) return "arm64";
  if (arch === "x64" || arch === 1) return "x64";
  if (arch === "ia32" || arch === 0) return "ia32";
  return process.arch;
}

function nativeDirFor(platform, arch) {
  if (platform === "darwin") return `mac-${arch}`;
  if (platform === "linux") return `linux-${arch}`;
  if (platform === "win32") return `win32-${arch}`;
  return "";
}

function resolveResourcesDir(appOutDir, platform) {
  if (platform !== "darwin") return path.join(appOutDir, "resources");

  const appBundle = fs.readdirSync(appOutDir)
    .find((entry) => entry.endsWith(".app"));

  return appBundle
    ? path.join(appOutDir, appBundle, "Contents", "Resources")
    : "";
}

module.exports = async function afterPackCleanup(context) {
  const platform = context.electronPlatformName || process.platform;
  const arch = normalizeArch(context.arch);
  const keepDir = nativeDirFor(platform, arch);
  if (!keepDir) return;

  const resourcesDir = resolveResourcesDir(context.appOutDir, platform);
  if (!resourcesDir) return;

  const addonDistDir = path.join(
    resourcesDir,
    "app.asar.unpacked",
    "node_modules",
    "@kutalia",
    "whisper-node-addon",
    "dist",
  );

  if (!fs.existsSync(addonDistDir)) return;

  for (const entry of fs.readdirSync(addonDistDir, { withFileTypes: true })) {
    if (!entry.isDirectory() || entry.name === keepDir || entry.name === "js") continue;
    if (!/^(mac|linux|win32)-/.test(entry.name)) continue;
    fs.rmSync(path.join(addonDistDir, entry.name), { recursive: true, force: true });
  }
};
