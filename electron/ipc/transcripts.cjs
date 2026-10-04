const fs = require("node:fs");
const path = require("node:path");
const { AppError } = require("../core/errors.cjs");
const { v } = require("./validate.cjs");

const RESERVED_TRANSCRIPT_FILE_NAME_CHARACTERS = new Set(["<", ">", ":", "\"", "/", "\\", "|", "?", "*"]);

function sanitizeTranscriptFileName(value = "transcript") {
  const normalized = Array.from(String(value), (character) => {
    if (RESERVED_TRANSCRIPT_FILE_NAME_CHARACTERS.has(character) || character.charCodeAt(0) < 32) {
      return " ";
    }

    return character;
  }).join("")
    .replace(/\s+/g, " ")
    .trim();

  return (normalized || "transcript").slice(0, 80);
}

function isPathInside(parentDir, candidatePath) {
  const relativePath = path.relative(path.resolve(parentDir), path.resolve(candidatePath));
  return relativePath === "" || (relativePath && !relativePath.startsWith("..") && !path.isAbsolute(relativePath));
}

function getTranscriptTextPath(transcriptDir, title) {
  return path.join(transcriptDir, `${sanitizeTranscriptFileName(title)}.txt`);
}

function resolveTranscriptDeletePath(transcriptDir, request = {}) {
  const requestedFilePath = typeof request.filePath === "string" ? request.filePath.trim() : "";
  const filePath = requestedFilePath
    ? path.resolve(requestedFilePath)
    : getTranscriptTextPath(transcriptDir, typeof request.title === "string" && request.title.trim() ? request.title : "Transcript");

  if (!isPathInside(transcriptDir, filePath)) {
    throw new AppError("INVALID_ARGUMENT", undefined, "Transcript file path is outside ASR Pro data.");
  }

  return filePath;
}

function registerTranscriptIpc({ router, ctx, textEditors }) {
  const transcriptDir = ctx.layout.transcriptsDir;

  router.handle("transcript:open-text", v.object({
    title: v.optional(v.string({ max: 400 })),
    text: v.optional(v.string({ max: 20 * 1024 * 1024 })),
  }), async (request) => {
    const title = request.title && request.title.trim() ? request.title : "Transcript";
    const text = request.text ? request.text.trim() : "";
    const filePath = getTranscriptTextPath(transcriptDir, title);

    fs.mkdirSync(transcriptDir, { recursive: true });
    fs.writeFileSync(filePath, `${text || "No transcript text available."}\n`, "utf8");

    await textEditors.openFile(filePath, ctx.settings.get("editor.defaultTextEditor"));

    return { filePath };
  });

  router.handle("transcript:delete-text", v.object({
    title: v.optional(v.string({ max: 400 })),
    filePath: v.optional(v.string({ max: 4096 })),
  }), (request) => {
    const filePath = resolveTranscriptDeletePath(transcriptDir, request);
    const existed = fs.existsSync(filePath);
    fs.rmSync(filePath, { force: true });

    return { deleted: existed, filePath };
  });
}

module.exports = {
  getTranscriptTextPath,
  isPathInside,
  registerTranscriptIpc,
  resolveTranscriptDeletePath,
  sanitizeTranscriptFileName,
};
