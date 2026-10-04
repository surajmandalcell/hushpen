const fs = require("node:fs");
const path = require("node:path");
const { normalizeOverlaySettings } = require("../runtime.cjs");

const APP_SETTINGS_FILE = "app-settings.json";
const OVERLAY_SETTINGS_FILE = "overlay-settings.json";

const APP_SETTING_KEYS = Object.freeze({
  defaultTextEditor: "editor.defaultTextEditor",
  autoCopyTranscripts: "output.autoCopy",
  launchAtStartup: "startup.launchAtLogin",
  startupExecutablePath: "startup.executablePath",
});

function readLegacyJson(filePath, problems) {
  let source;
  try {
    source = fs.readFileSync(filePath, "utf8");
  } catch (error) {
    if (error && error.code === "ENOENT") return undefined;
    problems.push(`${path.basename(filePath)} could not be read: ${error.message}`);
    return undefined;
  }

  try {
    const value = JSON.parse(source);
    if (value && typeof value === "object" && !Array.isArray(value)) return value;
    problems.push(`${path.basename(filePath)} is not an object`);
  } catch (error) {
    problems.push(`${path.basename(filePath)} is not valid JSON: ${error.message}`);
  }
  return undefined;
}

/**
 * Reads the 1.x `app-settings.json` and `overlay-settings.json` files.
 * Returns the registry keys they map to and the files that were understood;
 * values are validated by the registry, not here.
 */
function readLegacySettings(configDir) {
  const problems = [];
  const patch = {};
  const files = [];

  const appPath = path.join(configDir, APP_SETTINGS_FILE);
  const app = readLegacyJson(appPath, problems);
  if (app) {
    files.push(appPath);
    for (const [legacyKey, key] of Object.entries(APP_SETTING_KEYS)) {
      if (Object.hasOwn(app, legacyKey)) patch[key] = app[legacyKey];
    }
  }

  const overlayPath = path.join(configDir, OVERLAY_SETTINGS_FILE);
  const overlay = readLegacyJson(overlayPath, problems);
  if (overlay) {
    files.push(overlayPath);
    const normalized = normalizeOverlaySettings(overlay);
    patch["overlay.placement"] = normalized.placement;
    patch["overlay.customBounds"] = normalized.customBounds;
  }

  return { patch, files, problems };
}

function markLegacyFilesMigrated(files) {
  for (const file of files) {
    const target = `${file}.migrated`;
    fs.rmSync(target, { force: true });
    fs.renameSync(file, target);
  }
}

module.exports = { markLegacyFilesMigrated, readLegacySettings };
