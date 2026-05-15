const fs = require("node:fs");
const path = require("node:path");

const expectedSize = { width: 780, height: 520 };
const imageChecks = [
  { label: "asrpro-home.png", pathParts: ["docs", "screenshots", "asrpro-home.png"] },
  { label: "asrpro-configuration.png", pathParts: ["docs", "screenshots", "asrpro-configuration.png"] },
  { label: "asrpro-sound.png", pathParts: ["docs", "screenshots", "asrpro-sound.png"] },
  { label: "asrpro-history.png", pathParts: ["docs", "screenshots", "asrpro-history.png"] },
  { label: "asrpro-models.png", pathParts: ["docs", "screenshots", "asrpro-models.png"] },
  { label: "asrpro-about.png", pathParts: ["docs", "screenshots", "asrpro-about.png"] },
  { label: "asrpro_home.jpg", pathParts: ["_evidence", "asrpro_home.jpg"] },
  { label: "asrpro_configuration.jpg", pathParts: ["_evidence", "asrpro_configuration.jpg"] },
  { label: "asrpro_sound.jpg", pathParts: ["_evidence", "asrpro_sound.jpg"] },
  { label: "asrpro_models_library.jpg", pathParts: ["_evidence", "asrpro_models_library.jpg"] },
  { label: "asrpro_history.jpg", pathParts: ["_evidence", "asrpro_history.jpg"] },
  { label: "asrpro_about.jpg", pathParts: ["_evidence", "asrpro_about.jpg"] },
];
const expectedEvidenceFiles = new Set(
  imageChecks
    .filter((check) => check.pathParts[0] === "_evidence")
    .map((check) => check.pathParts.at(-1)),
);
const snakeCaseImageNamePattern = /^[a-z0-9_]+\.(jpg|jpeg|png)$/;

function readImageSize(filePath) {
  const buffer = fs.readFileSync(filePath);

  if (buffer.subarray(0, 8).toString("hex") === "89504e470d0a1a0a") {
    return {
      width: buffer.readUInt32BE(16),
      height: buffer.readUInt32BE(20),
    };
  }

  if (buffer[0] === 0xff && buffer[1] === 0xd8) {
    return readJpegSize(filePath, buffer);
  }

  throw new Error(`${filePath} is not a supported PNG or JPEG file`);
}

function readJpegSize(filePath, buffer) {
  let offset = 2;
  while (offset < buffer.length) {
    while (buffer[offset] === 0xff) offset += 1;
    const marker = buffer[offset];
    offset += 1;

    if (marker === 0xd9 || marker === 0xda) break;
    if (marker >= 0xd0 && marker <= 0xd7) continue;

    const length = buffer.readUInt16BE(offset);
    const isStartOfFrame = marker >= 0xc0 && marker <= 0xcf && ![0xc4, 0xc8, 0xcc].includes(marker);
    if (isStartOfFrame) {
      return {
        height: buffer.readUInt16BE(offset + 3),
        width: buffer.readUInt16BE(offset + 5),
      };
    }
    offset += length;
  }

  throw new Error(`${filePath} does not contain a JPEG size marker`);
}

function main() {
  const repoRoot = path.resolve(__dirname, "..");
  const evidenceDir = path.join(repoRoot, "_evidence");
  let failed = false;

  if (fs.existsSync(evidenceDir)) {
    const evidenceFiles = fs.readdirSync(evidenceDir).filter((file) => !file.startsWith("."));
    for (const file of evidenceFiles) {
      if (!snakeCaseImageNamePattern.test(file)) {
        console.error(`${file}: evidence filename is not snake_case`);
        failed = true;
      }
      if (!expectedEvidenceFiles.has(file)) {
        console.error(`${file}: unexpected evidence file`);
        failed = true;
      }
    }
  }

  for (const check of imageChecks) {
    const filePath = path.join(repoRoot, ...check.pathParts);
    if (!fs.existsSync(filePath)) {
      console.error(`${check.label}: missing`);
      failed = true;
      continue;
    }

    const size = readImageSize(filePath);
    const matches = size.width === expectedSize.width && size.height === expectedSize.height;
    console.log(`${check.label}: ${size.width}x${size.height}${matches ? "" : `, expected ${expectedSize.width}x${expectedSize.height}`}`);
    if (!matches) failed = true;
  }

  if (failed) {
    process.exitCode = 1;
  }
}

main();
