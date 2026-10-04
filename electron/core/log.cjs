const fs = require("node:fs");
const path = require("node:path");

const DEFAULT_MAX_BYTES = 2 * 1024 * 1024;
const DEFAULT_MAX_FILES = 3;
const DETAIL_LIMIT = 500;
const SCOPE_PATTERN = /^[a-z][a-z0-9-]{0,31}$/;

function singleLine(value, limit) {
  return String(value ?? "").replace(/[\r\n\t]+/g, " ").slice(0, limit);
}

/**
 * Local error log with size-based rotation (`name`, `name.1`, `name.2`).
 * Entries carry a timestamp, scope, error code, and a short detail. Callers
 * pass codes and technical details only, never transcript text or audio.
 */
function createLog({
  dir,
  fileName = "asrpro.log",
  maxBytes = DEFAULT_MAX_BYTES,
  maxFiles = DEFAULT_MAX_FILES,
  now = () => new Date(),
}) {
  const filePath = path.join(dir, fileName);
  let size;

  function currentSize() {
    if (size === undefined) {
      try {
        size = fs.statSync(filePath).size;
      } catch {
        size = 0;
      }
    }
    return size;
  }

  function rotate() {
    const oldest = maxFiles - 1;
    fs.rmSync(`${filePath}.${oldest}`, { force: true });
    for (let index = oldest - 1; index >= 1; index -= 1) {
      if (fs.existsSync(`${filePath}.${index}`)) fs.renameSync(`${filePath}.${index}`, `${filePath}.${index + 1}`);
    }
    if (fs.existsSync(filePath)) {
      if (oldest >= 1) fs.renameSync(filePath, `${filePath}.1`);
      else fs.rmSync(filePath, { force: true });
    }
    size = 0;
  }

  function write(level, scope, code, detail) {
    const safeScope = SCOPE_PATTERN.test(scope) ? scope : "app";
    const line = `${now().toISOString()} ${level} ${safeScope} ${singleLine(code, 64)} ${singleLine(detail, DETAIL_LIMIT)}\n`;
    const lineBytes = Buffer.byteLength(line);

    try {
      fs.mkdirSync(dir, { recursive: true });
      if (currentSize() > 0 && currentSize() + lineBytes > maxBytes) rotate();
      fs.appendFileSync(filePath, line);
      size = currentSize() + lineBytes;
    } catch {
      // Logging must never break the app.
    }
  }

  return {
    filePath,
    error: (scope, code, detail) => write("ERROR", scope, code, detail),
    warn: (scope, code, detail) => write("WARN", scope, code, detail),
  };
}

module.exports = { createLog, DEFAULT_MAX_BYTES, DEFAULT_MAX_FILES };
