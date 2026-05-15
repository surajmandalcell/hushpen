const crypto = require("node:crypto");
const fs = require("node:fs");
const https = require("node:https");
const os = require("node:os");
const path = require("node:path");
const { promisify } = require("node:util");

const WHISPER_MODEL_BASE_URL = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

const AVAILABLE_MODELS = Object.freeze([
  {
    id: "whisper-tiny-en",
    displayName: "Whisper Tiny English",
    detail: "Fastest, smallest English model",
    fileName: "ggml-tiny.en.bin",
    language: "en",
    sizeLabel: "75 MB",
    sha1: "c78c86eb1a8faa21b369bcd33207cc90d64ae9df",
  },
  {
    id: "whisper-base-en",
    displayName: "Whisper Base English",
    detail: "Balanced local English dictation",
    fileName: "ggml-base.en.bin",
    language: "en",
    sizeLabel: "142 MB",
    sha1: "137c40403d78fd54d454da0f9bd998f78703390c",
  },
  {
    id: "whisper-small-en",
    displayName: "Whisper Small English",
    detail: "Better accuracy, larger local model",
    fileName: "ggml-small.en.bin",
    language: "en",
    sizeLabel: "466 MB",
    sha1: "db8a495a91d927739e50b3fc1cc4c6b8f6c2d022",
  },
  {
    id: "whisper-base",
    displayName: "Whisper Base Multilingual",
    detail: "Small multilingual Whisper model",
    fileName: "ggml-base.bin",
    language: "auto",
    sizeLabel: "142 MB",
    sha1: "465707469ff3a37a2b9b8d8f89f2f99de7299dac",
  },
]);

const DEFAULT_MODEL = AVAILABLE_MODELS[1];

let addon;

function getModelById(modelId) {
  return AVAILABLE_MODELS.find((model) => model.id === modelId) || DEFAULT_MODEL;
}

function getWhisperModelsDir(dataDir) {
  return path.join(dataDir, "models", "whisper");
}

function getModelPath(dataDir, modelId = DEFAULT_MODEL.id) {
  const model = getModelById(modelId);
  return path.join(getWhisperModelsDir(dataDir), model.fileName);
}

function listModels(dataDir) {
  return AVAILABLE_MODELS.map((model) => ({
    ...model,
    path: getModelPath(dataDir, model.id),
    installed: fs.existsSync(getModelPath(dataDir, model.id)),
    downloadUrl: `${WHISPER_MODEL_BASE_URL}/${model.fileName}`,
  }));
}

function loadAddon() {
  if (!addon) {
    try {
      addon = require("@kutalia/whisper-node-addon");
    } catch (error) {
      addon = loadAddonFromPackagedBinary(error);
    }
  }
  return addon;
}

function loadAddonFromPackagedBinary(originalError) {
  const packageRoot = path.dirname(require.resolve("@kutalia/whisper-node-addon/package.json"));
  const nativeDir = getNativeAddonDir();
  const addonPath = path.join(packageRoot, "dist", nativeDir, "whisper.node");

  try {
    const nativeAddon = require(addonPath);
    if (typeof nativeAddon.whisper !== "function") {
      throw new Error(`Native addon at ${addonPath} does not export whisper.`);
    }
    return {
      transcribe: promisify(nativeAddon.whisper),
    };
  } catch (fallbackError) {
    const originalMessage = originalError instanceof Error ? originalError.message : String(originalError);
    const fallbackMessage = fallbackError instanceof Error ? fallbackError.message : String(fallbackError);
    throw new Error(`Failed to load native Whisper addon. Package loader: ${originalMessage}. Binary loader: ${fallbackMessage}`);
  }
}

function getNativeAddonDir() {
  const platformMap = {
    darwin: "mac",
    linux: "linux",
    win32: "win32",
  };
  const platform = platformMap[process.platform];
  if (!platform) {
    throw new Error(`Unsupported platform for native Whisper addon: ${process.platform}`);
  }
  return `${platform}-${process.arch}`;
}

async function ensureModel(model, dataDir, onState = () => {}) {
  const modelPath = getModelPath(dataDir, model.id);
  if (fs.existsSync(modelPath)) {
    await verifySha1(modelPath, model.sha1);
    return modelPath;
  }

  fs.mkdirSync(path.dirname(modelPath), { recursive: true });
  onState({
    status: "downloading",
    modelId: model.id,
    model: model.displayName,
    detail: `Downloading ${model.displayName}`,
    progress: 0,
  });

  await downloadFile(`${WHISPER_MODEL_BASE_URL}/${model.fileName}`, modelPath, (progress) => {
    onState({
      status: "downloading",
      modelId: model.id,
      model: model.displayName,
      detail: `Downloading ${model.displayName}`,
      progress,
    });
  });

  await verifySha1(modelPath, model.sha1);
  return modelPath;
}

function downloadFile(url, destination, onProgress = () => {}) {
  const tempPath = `${destination}.download`;

  return new Promise((resolve, reject) => {
    const request = https.get(url, (response) => {
      if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
        response.resume();
        downloadFile(response.headers.location, destination, onProgress).then(resolve, reject);
        return;
      }

      if (response.statusCode !== 200) {
        response.resume();
        reject(new Error(`Model download failed with HTTP ${response.statusCode}`));
        return;
      }

      const totalBytes = Number(response.headers["content-length"]) || 0;
      let downloadedBytes = 0;
      const output = fs.createWriteStream(tempPath);

      response.on("data", (chunk) => {
        downloadedBytes += chunk.length;
        if (totalBytes > 0) {
          onProgress(Math.round((downloadedBytes / totalBytes) * 100));
        }
      });
      response.pipe(output);

      output.on("finish", () => {
        output.close(() => {
          fs.renameSync(tempPath, destination);
          onProgress(100);
          resolve();
        });
      });
      output.on("error", (error) => {
        fs.rmSync(tempPath, { force: true });
        reject(error);
      });
    });

    request.on("error", (error) => {
      fs.rmSync(tempPath, { force: true });
      reject(error);
    });
  });
}

function verifySha1(filePath, expectedSha1) {
  if (!expectedSha1) return Promise.resolve();

  return new Promise((resolve, reject) => {
    const hash = crypto.createHash("sha1");
    const input = fs.createReadStream(filePath);

    input.on("data", (chunk) => hash.update(chunk));
    input.on("error", reject);
    input.on("end", () => {
      const actualSha1 = hash.digest("hex");
      if (actualSha1 !== expectedSha1) {
        fs.rmSync(filePath, { force: true });
        reject(new Error(`Downloaded model checksum mismatch for ${path.basename(filePath)}.`));
        return;
      }
      resolve();
    });
  });
}

function normalizeTranscriptionResult(result) {
  if (typeof result === "string") return result.trim();
  if (!result) return "";

  const transcription = result.transcription ?? result.text ?? result;
  if (typeof transcription === "string") return transcription.trim();
  if (Array.isArray(transcription)) {
    return transcription.flat(Infinity).join(" ").replace(/\s+/g, " ").trim();
  }

  return String(transcription).trim();
}

async function transcribeAudioFile({ filePath, modelId, dataDir, onState = () => {} }) {
  const model = getModelById(modelId);
  const modelPath = await ensureModel(model, dataDir, onState);
  const whisper = loadAddon();

  onState({
    status: "transcribing",
    modelId: model.id,
    model: model.displayName,
    detail: `Transcribing with ${model.displayName}`,
    progress: null,
  });

  const result = await whisper.transcribe({
    fname_inp: filePath,
    model: modelPath,
    language: model.language === "auto" ? "en" : model.language,
    detect_language: model.language === "auto",
    translate: false,
    no_timestamps: true,
    no_prints: true,
    use_gpu: true,
    n_threads: Math.max(2, Math.min(os.cpus().length, 8)),
  });

  return {
    text: normalizeTranscriptionResult(result),
    model: model.id,
    modelName: model.displayName,
  };
}

module.exports = {
  AVAILABLE_MODELS,
  DEFAULT_MODEL,
  WHISPER_MODEL_BASE_URL,
  getModelById,
  getModelPath,
  getWhisperModelsDir,
  listModels,
  normalizeTranscriptionResult,
  loadAddon,
  transcribeAudioFile,
};
