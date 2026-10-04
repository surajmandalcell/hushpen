const { RECORDING_SHORTCUT, collectRuntimeStorageStats } = require("../runtime.cjs");
const { getModelById, listModels } = require("../whisper-engine.cjs");

function createRuntimeState({ ctx, recording, overlay, textEditors, startup }) {
  return async function getRuntimeState() {
    const values = ctx.settings.getAll();
    const model = getModelById(values["transcription.modelId"]);

    return {
      ...recording.getState(),
      dataDir: ctx.dataDir,
      defaultModel: model.displayName,
      defaultModelId: model.id,
      audioInputId: values["recording.audioInputId"],
      models: listModels(ctx.dataDir),
      defaultTextEditor: values["editor.defaultTextEditor"],
      autoCopyTranscripts: values["output.autoCopy"],
      launchAtStartup: values["startup.launchAtLogin"],
      startup: startup.getState(),
      textEditors: await textEditors.list(),
      overlaySettings: overlay.getOverlaySettings(),
      engine: ctx.state.engine,
      storageStats: collectRuntimeStorageStats(ctx.dataDir, process.memoryUsage(), ctx.state.engine.modelId),
      shortcut: RECORDING_SHORTCUT,
      shortcutRegistered: Boolean(ctx.state.shortcutRegistered),
      capabilities: {
        nativeWhisper: true,
      },
    };
  };
}

module.exports = { createRuntimeState };
