const { AVAILABLE_MODELS } = require("../whisper-engine.cjs");
const { AppError } = require("../core/errors.cjs");
const { SCHEMA_VERSION } = require("../settings/schema.cjs");
const { v } = require("./validate.cjs");

function resolveLegacyModelId(name) {
  const trimmed = typeof name === "string" ? name.trim() : "";
  return AVAILABLE_MODELS.find((model) => model.displayName === trimmed || model.id === trimmed)?.id;
}

function registerSettingsIpc({ router, ctx, overlay, startup }) {
  const { settings } = ctx;

  settings.onChange(({ changed, values }) => {
    ctx.pushToMain("settings:changed", { changed, values });
  });

  router.handle("settings:get-all", v.none(), () => ({
    schemaVersion: SCHEMA_VERSION,
    values: settings.getAll(),
  }));

  router.handle("settings:set", v.object({
    key: v.string({ min: 1, max: 80 }),
    value: v.any(),
  }), ({ key, value }) => {
    settings.check(key, value, { source: "renderer" });

    if (key === "startup.launchAtLogin") {
      const values = settings.update(startup.apply(value));
      return { values, startup: startup.getState() };
    }

    if (key === "overlay.placement") {
      const values = settings.update({ "overlay.placement": value, "overlay.customBounds": null });
      overlay.position();
      return { values };
    }

    return { values: settings.set(key, value, { source: "renderer" }) };
  });

  // The 1.x renderer kept the model and microphone choice in localStorage. The
  // renderer reads those keys and hands them over once; the keys stay in place.
  router.handle("settings:import-legacy", v.object({
    selectedModelName: v.optional(v.string({ max: 200 })),
    audioInputId: v.optional(v.string({ max: 4096 })),
  }), ({ selectedModelName, audioInputId }) => {
    if (settings.get("migrations.legacyLocalStorage")) {
      return { imported: false, values: settings.getAll() };
    }

    const patch = {};
    const modelId = resolveLegacyModelId(selectedModelName);
    if (modelId) patch["transcription.modelId"] = modelId;

    const deviceId = typeof audioInputId === "string" ? audioInputId.trim() : "";
    if (deviceId) {
      try {
        settings.check("recording.audioInputId", deviceId);
        patch["recording.audioInputId"] = deviceId;
      } catch (error) {
        if (!(error instanceof AppError)) throw error;
        ctx.log.warn("migration", error.code, "Legacy microphone id was not imported.");
      }
    }

    if (Object.keys(patch).length === 0) {
      return { imported: false, values: settings.getAll() };
    }

    patch["migrations.legacyLocalStorage"] = true;
    return { imported: true, values: settings.update(patch) };
  });
}

module.exports = { registerSettingsIpc, resolveLegacyModelId };
