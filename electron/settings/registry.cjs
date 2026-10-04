const path = require("node:path");
const { AppError } = require("../core/errors.cjs");
const { readJsonFile, writeFileAtomic } = require("../core/atomicFile.cjs");
const { markLegacyFilesMigrated, readLegacySettings } = require("./legacy.cjs");
const { INVALID, SCHEMA, SCHEMA_VERSION, getDefaults } = require("./schema.cjs");

const UNKNOWN_KEY = "_unknown";
const SETTINGS_FILE_NAME = "settings.json";

function isPlainObject(value) {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function isSettingsFile(value) {
  return isPlainObject(value)
    && typeof value.schemaVersion === "number"
    && Number.isFinite(value.schemaVersion)
    && isPlainObject(value.values);
}

function invalid(key) {
  return new AppError("SETTINGS_INVALID", { key: String(key).slice(0, 80) });
}

/** Returns the normalized value or throws SETTINGS_INVALID; the renderer cannot touch internal keys. */
function check(key, raw, { source = "main" } = {}) {
  if (!Object.hasOwn(SCHEMA, key)) throw invalid(key);
  const spec = SCHEMA[key];
  if (spec.internal && source !== "main") throw invalid(key);
  const normalized = spec.normalize(raw);
  if (normalized === INVALID) throw invalid(key);
  return normalized;
}

function createSettingsRegistry({ configDir, log }) {
  const filePath = path.join(configDir, SETTINGS_FILE_NAME);
  const listeners = new Set();
  let values = getDefaults();
  let unknown = {};
  let fileVersion = SCHEMA_VERSION;

  function serialize() {
    const output = { ...values };
    if (Object.keys(unknown).length > 0) output[UNKNOWN_KEY] = unknown;
    return { schemaVersion: fileVersion, values: output };
  }

  function persist() {
    writeFileAtomic(filePath, `${JSON.stringify(serialize(), null, 2)}\n`);
  }

  function tryPersist() {
    try {
      persist();
    } catch (error) {
      log.error("settings", "INTERNAL", `settings.json could not be written: ${error.message}`);
    }
  }

  function ingest(fileValues) {
    let changed = false;
    const nextValues = getDefaults();
    const nextUnknown = {};

    const adopt = (key, raw) => {
      const normalized = SCHEMA[key].normalize(raw);
      if (normalized === INVALID) {
        log.warn("settings", "SETTINGS_INVALID", `Stored value for ${key} was reset to its default.`);
        changed = true;
        return;
      }
      nextValues[key] = normalized;
    };

    for (const [key, raw] of Object.entries(fileValues)) {
      if (key === UNKNOWN_KEY) continue;
      if (Object.hasOwn(SCHEMA, key)) adopt(key, raw);
      else nextUnknown[key] = raw;
    }

    if (isPlainObject(fileValues[UNKNOWN_KEY])) {
      for (const [key, raw] of Object.entries(fileValues[UNKNOWN_KEY])) {
        // A key stored by a newer build becomes known when this build learns it.
        if (Object.hasOwn(SCHEMA, key) && !Object.hasOwn(fileValues, key)) adopt(key, raw);
        else if (!Object.hasOwn(SCHEMA, key)) nextUnknown[key] = raw;
      }
    }

    values = nextValues;
    unknown = nextUnknown;
    return changed;
  }

  function migrateLegacyFiles() {
    const { patch, files, problems } = readLegacySettings(configDir);
    for (const problem of problems) {
      log.error("migration", "SETTINGS_INVALID", `Legacy settings skipped: ${problem}`);
    }
    for (const [key, raw] of Object.entries(patch)) {
      const normalized = SCHEMA[key].normalize(raw);
      if (normalized !== INVALID) values[key] = normalized;
    }
    return files;
  }

  function load() {
    const result = readJsonFile(filePath, { isValid: isSettingsFile });

    if (result.status === "corrupt") {
      const backupName = result.backupPath ? path.basename(result.backupPath) : "(not renamed)";
      log.error("settings", "SETTINGS_INVALID", `settings.json is corrupt (${result.reason}); backed up as ${backupName}, defaults restored.`);
      values = getDefaults();
      unknown = {};
      tryPersist();
      return;
    }

    if (result.status === "missing") {
      values = getDefaults();
      unknown = {};
      let legacyFiles = [];
      try {
        legacyFiles = migrateLegacyFiles();
      } catch (error) {
        log.error("migration", "INTERNAL", `Legacy settings migration failed: ${error.message}`);
      }
      tryPersist();
      try {
        markLegacyFilesMigrated(legacyFiles);
      } catch (error) {
        log.error("migration", "INTERNAL", `Legacy settings files could not be renamed: ${error.message}`);
      }
      return;
    }

    fileVersion = Math.max(result.value.schemaVersion, SCHEMA_VERSION);
    const sanitized = ingest(result.value.values);
    if (sanitized || JSON.stringify(serialize()) !== JSON.stringify(result.value)) tryPersist();
  }

  function emitChange(changedKeys) {
    for (const listener of listeners) {
      try {
        listener({ changed: changedKeys, values: getAll() });
      } catch (error) {
        log.error("settings", "INTERNAL", `Settings listener failed: ${error.message}`);
      }
    }
  }

  function getAll() {
    return structuredClone(values);
  }

  function get(key) {
    if (!Object.hasOwn(SCHEMA, key)) throw invalid(key);
    return structuredClone(values[key]);
  }

  function update(patch, { source = "main" } = {}) {
    if (!isPlainObject(patch)) throw new AppError("INVALID_ARGUMENT");
    const next = {};
    for (const [key, raw] of Object.entries(patch)) {
      next[key] = check(key, raw, { source });
    }

    const changedKeys = Object.keys(next).filter((key) => JSON.stringify(next[key]) !== JSON.stringify(values[key]));
    if (changedKeys.length === 0) return getAll();

    const previous = values;
    values = { ...values, ...next };
    try {
      persist();
    } catch (error) {
      values = previous;
      throw error;
    }
    emitChange(changedKeys);
    return getAll();
  }

  function set(key, value, options) {
    return update({ [key]: value }, options);
  }

  function onChange(listener) {
    listeners.add(listener);
    return () => listeners.delete(listener);
  }

  load();

  return { filePath, check, get, getAll, set, update, onChange };
}

module.exports = { createSettingsRegistry, SETTINGS_FILE_NAME };
