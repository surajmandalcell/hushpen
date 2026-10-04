const fs = require("node:fs");
const path = require("node:path");
const { AppError, toErrorShape } = require("../core/errors.cjs");
const {
  AVAILABLE_MODELS,
  deleteModelFile,
  downloadModelFile,
  getModelById,
  listModels,
  transcribeAudioFile,
} = require("../whisper-engine.cjs");
const { v } = require("./validate.cjs");

function toAudioBuffer(audioData) {
  if (Buffer.isBuffer(audioData)) return audioData;
  if (audioData instanceof ArrayBuffer) return Buffer.from(audioData);
  if (ArrayBuffer.isView(audioData)) {
    return Buffer.from(audioData.buffer, audioData.byteOffset, audioData.byteLength);
  }
  throw new AppError("ENGINE_AUDIO_INVALID", undefined, "Transcription audio payload is missing.");
}

function findModel(modelId) {
  const model = AVAILABLE_MODELS.find((candidate) => candidate.id === modelId);
  if (!model) {
    throw new AppError("INVALID_ARGUMENT", { modelId: String(modelId).slice(0, 80) }, "Unsupported recognition model.");
  }
  return model;
}

function createEngineService({ ctx, getRuntimeState }) {
  function setState(nextState) {
    ctx.state.engine = {
      ...ctx.state.engine,
      ...nextState,
      updatedAt: new Date().toISOString(),
    };
    ctx.pushToMain("engine:state", ctx.state.engine);
    return ctx.state.engine;
  }

  function setReady(model) {
    setState({
      status: "ready",
      mode: "native-node",
      modelId: model.id,
      model: model.displayName,
      progress: null,
      error: null,
      errorCode: null,
    });
  }

  function setFailed(model, error) {
    const shape = toErrorShape(error);
    setState({
      status: "failed",
      mode: "native-node",
      modelId: model.id,
      model: model.displayName,
      progress: null,
      error: shape.detail || null,
      errorCode: shape.code,
    });
  }

  async function downloadModel(modelId) {
    const model = findModel(modelId);

    try {
      await downloadModelFile({ modelId: model.id, dataDir: ctx.dataDir, onState: setState });
      setReady(model);
    } catch (error) {
      const coded = error instanceof AppError ? error : new AppError("MODEL_DOWNLOAD_FAILED", undefined, error.message);
      setFailed(model, coded);
      throw coded;
    }

    return getRuntimeState();
  }

  async function deleteModel(modelId) {
    const model = findModel(modelId);
    const { status, modelId: activeModelId } = ctx.state.engine;

    if ((status === "downloading" || status === "transcribing") && activeModelId === model.id) {
      throw new AppError("MODEL_IN_USE", { modelId: model.id }, `${model.displayName} is currently in use.`);
    }

    deleteModelFile({ modelId: model.id, dataDir: ctx.dataDir });
    return getRuntimeState();
  }

  async function transcribe({ audioData, mimeType, modelId }) {
    const model = modelId ? findModel(modelId) : getModelById(ctx.settings.get("transcription.modelId"));
    const extension = mimeType.includes("wav") ? "wav" : "audio";
    const workDir = fs.mkdtempSync(path.join(ctx.app.getPath("temp"), "asrpro-whisper-"));

    try {
      const filePath = path.join(workDir, `recording.${extension}`);
      fs.writeFileSync(filePath, toAudioBuffer(audioData));
      const result = await transcribeAudioFile({
        filePath,
        modelId: model.id,
        dataDir: ctx.dataDir,
        onState: setState,
      });
      setReady(model);
      return result;
    } catch (error) {
      const coded = error instanceof AppError ? error : new AppError("INTERNAL", undefined, error.message);
      setFailed(model, coded);
      throw coded;
    } finally {
      fs.rmSync(workDir, { recursive: true, force: true });
    }
  }

  return { deleteModel, downloadModel, setState, transcribe };
}

function registerEngineIpc({ router, ctx, engine }) {
  router.handle("models:list", v.none(), () => listModels(ctx.dataDir));
  router.handle("models:download", v.object({ modelId: v.string({ min: 1, max: 80 }) }), ({ modelId }) => engine.downloadModel(modelId));
  router.handle("models:delete", v.object({ modelId: v.string({ min: 1, max: 80 }) }), ({ modelId }) => engine.deleteModel(modelId));
  router.handle("engine:transcribe-audio", v.object({
    audioData: v.binary(),
    mimeType: v.string({ max: 100 }),
    modelId: v.optional(v.string({ min: 1, max: 80 })),
  }), (request) => engine.transcribe(request));
}

module.exports = { createEngineService, registerEngineIpc };
