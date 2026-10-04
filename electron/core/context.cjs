const { EventEmitter } = require("node:events");
const { getModelById } = require("../whisper-engine.cjs");
const { createLog } = require("./log.cjs");
const { ensureDataLayout, resolveDataDir } = require("./dataDir.cjs");
const { createSettingsRegistry } = require("../settings/registry.cjs");

/**
 * Shared main-process state. Modules receive this object instead of reaching
 * for module-level variables, so each one can be built and tested alone.
 */
function createContext({ app, env = process.env, platform = process.platform, executablePath }) {
  const dataDir = resolveDataDir({ app, env, platform, executablePath });
  const layout = ensureDataLayout(dataDir);
  const log = createLog({ dir: layout.logsDir });
  const settings = createSettingsRegistry({ configDir: layout.configDir, log });
  const activeModel = getModelById(settings.get("transcription.modelId"));

  const windows = { main: undefined, overlay: undefined };

  return {
    app,
    env,
    platform,
    dataDir,
    layout,
    log,
    settings,
    events: new EventEmitter(),
    windows,
    pushToMain(channel, payload) {
      const win = windows.main;
      if (win && !win.isDestroyed()) win.webContents.send(channel, payload);
    },
    state: {
      isQuitting: false,
      isRecording: false,
      lastWaveformFrame: [],
      engine: {
        status: "idle",
        mode: "native-node",
        modelId: activeModel.id,
        model: activeModel.displayName,
        progress: null,
        error: null,
      },
    },
  };
}

module.exports = { createContext };
