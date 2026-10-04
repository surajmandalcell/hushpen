const defaults = require("../../shared/settings-defaults.json");
const { AVAILABLE_MODELS } = require("../whisper-engine.cjs");
const { normalizeOverlaySettings } = require("../runtime.cjs");

const INVALID = Symbol("invalid-setting");
const SCHEMA_VERSION = 2;
const UI_LANGUAGES = ["en", "es", "fr", "de", "pt-BR", "hi", "ja", "zh-CN"];
const TEXT_EDITOR_IDS = ["system", "textedit", "vscode", "cursor"];
const MODEL_IDS = AVAILABLE_MODELS.map((model) => model.id);
const VOCABULARY_MAX_LENGTH = 400;

const boolean = (value) => (typeof value === "boolean" ? value : INVALID);
const oneOf = (allowed) => (value) => (allowed.includes(value) ? value : INVALID);
const text = (maxLength, { allowEmpty = false, trim = true } = {}) => (value) => {
  if (typeof value !== "string") return INVALID;
  const next = trim ? value.trim() : value;
  if (next.length > maxLength || (!allowEmpty && next.length === 0)) return INVALID;
  return next;
};
const autoOrInteger = (min, max) => (value) => {
  if (value === "auto") return value;
  return Number.isInteger(value) && value >= min && value <= max ? value : INVALID;
};
const nullable = (inner) => (value) => (value === null ? null : inner(value));
const finiteNumber = (value) => (typeof value === "number" && Number.isFinite(value) ? value : INVALID);
const customBounds = (value) => {
  if (value === null) return null;
  const normalized = normalizeOverlaySettings({ customBounds: value }).customBounds;
  return normalized ?? INVALID;
};
const language = (value) => {
  if (value === "auto") return value;
  return typeof value === "string" && /^[a-z]{2,3}$/.test(value) ? value : INVALID;
};

// `internal` keys are written by the main process only; the renderer cannot set them.
const SCHEMA = Object.freeze({
  "ui.language": { normalize: oneOf(UI_LANGUAGES) },
  "transcription.modelId": { normalize: oneOf(MODEL_IDS) },
  "transcription.language": { normalize: language },
  "transcription.translate": { normalize: boolean },
  "transcription.vocabulary": { normalize: text(VOCABULARY_MAX_LENGTH, { allowEmpty: true }) },
  "engine.useGpu": { normalize: boolean },
  "engine.threads": { normalize: autoOrInteger(1, 16) },
  "captions.enabled": { normalize: boolean },
  "captions.modelId": { normalize: (value) => (value === "auto" ? value : oneOf(MODEL_IDS)(value)) },
  "captions.showInOverlay": { normalize: boolean },
  "recording.audioInputId": { normalize: text(512) },
  "recording.keepAudio": { normalize: boolean },
  "recording.shortcut": { normalize: text(100) },
  "overlay.enabled": { normalize: boolean },
  "overlay.placement": { normalize: oneOf(["top", "bottom"]) },
  "overlay.customBounds": { normalize: customBounds, internal: true },
  "output.autoCopy": { normalize: boolean },
  "editor.defaultTextEditor": { normalize: oneOf(TEXT_EDITOR_IDS) },
  "startup.launchAtLogin": { normalize: boolean },
  "startup.executablePath": { normalize: text(4096, { allowEmpty: true, trim: false }), internal: true },
  "speakers.defaultCount": { normalize: autoOrInteger(2, 6) },
  "imports.labelSpeakers": { normalize: boolean },
  "updates.autoCheck": { normalize: boolean },
  "updates.lastCheckAt": { normalize: nullable(finiteNumber), internal: true },
  "updates.dismissedVersion": { normalize: nullable(text(64)), internal: true },
  "migrations.legacyLocalStorage": { normalize: boolean, internal: true },
});

function getDefaults() {
  return structuredClone(defaults);
}

module.exports = {
  INVALID,
  MODEL_IDS,
  SCHEMA,
  SCHEMA_VERSION,
  TEXT_EDITOR_IDS,
  getDefaults,
};
