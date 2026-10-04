const fs = require("node:fs");
const path = require("node:path");

let tempCounter = 0;

function writeFileAtomic(filePath, contents) {
  const directory = path.dirname(filePath);
  fs.mkdirSync(directory, { recursive: true });

  tempCounter += 1;
  const tempPath = path.join(directory, `.${path.basename(filePath)}.${process.pid}.${tempCounter}.tmp`);
  let descriptor;
  try {
    descriptor = fs.openSync(tempPath, "w");
    fs.writeFileSync(descriptor, contents);
    fs.fsyncSync(descriptor);
    fs.closeSync(descriptor);
    descriptor = undefined;
    fs.renameSync(tempPath, filePath);
  } catch (error) {
    if (descriptor !== undefined) {
      try { fs.closeSync(descriptor); } catch { /* already closed */ }
    }
    fs.rmSync(tempPath, { force: true });
    throw error;
  }

  syncDirectory(directory);
}

// Directory fsync is unsupported on Windows and some network drives; the rename itself already happened.
function syncDirectory(directory) {
  let descriptor;
  try {
    descriptor = fs.openSync(directory, "r");
    fs.fsyncSync(descriptor);
  } catch {
    // Best effort only.
  } finally {
    if (descriptor !== undefined) {
      try { fs.closeSync(descriptor); } catch { /* ignore */ }
    }
  }
}

/**
 * Reads a JSON file. A missing file is `{ status: "missing" }`. A file that
 * cannot be parsed, or that fails `isValid`, is renamed to
 * `<name>.corrupt-<timestamp>` and reported as `{ status: "corrupt", backupPath }`.
 */
function readJsonFile(filePath, { isValid = () => true, now = () => Date.now() } = {}) {
  let source;
  try {
    source = fs.readFileSync(filePath, "utf8");
  } catch (error) {
    if (error && error.code === "ENOENT") return { status: "missing" };
    return { status: "corrupt", backupPath: backupCorruptFile(filePath, now), reason: error.message };
  }

  try {
    const value = JSON.parse(source);
    if (!isValid(value)) {
      return { status: "corrupt", backupPath: backupCorruptFile(filePath, now), reason: "unexpected shape" };
    }
    return { status: "ok", value };
  } catch (error) {
    return { status: "corrupt", backupPath: backupCorruptFile(filePath, now), reason: error.message };
  }
}

function backupCorruptFile(filePath, now) {
  const backupPath = `${filePath}.corrupt-${now()}`;
  try {
    fs.renameSync(filePath, backupPath);
    return backupPath;
  } catch {
    return undefined;
  }
}

module.exports = { readJsonFile, writeFileAtomic };
