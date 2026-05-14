#!/usr/bin/env node

const { spawnSync } = require("node:child_process");
const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");

const rootDir = path.resolve(__dirname, "..");
const sidecarDir = path.join(rootDir, "sidecar");
const venvDir = path.join(sidecarDir, ".venv");
const requirementsPath = path.join(sidecarDir, "requirements.txt");
const markerPath = path.join(venvDir, ".asrpro-requirements.sha256");
const sidecarMainPath = path.join(sidecarDir, "main.py");
const venvPython = process.platform === "win32"
  ? path.join(venvDir, "Scripts", "python.exe")
  : path.join(venvDir, "bin", "python");
const bootstrapPython = process.env.PYTHON || (process.platform === "win32" ? "python" : "python3");
const shouldRunSidecar = process.argv.includes("--run");

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: rootDir,
    stdio: "inherit",
    ...options,
  });

  if (result.error) {
    console.error(result.error.message);
    process.exit(1);
  }

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function getRequirementsHash() {
  return crypto
    .createHash("sha256")
    .update(fs.readFileSync(requirementsPath))
    .digest("hex");
}

function canImportCoreSidecarDeps() {
  if (!fs.existsSync(venvPython)) return false;

  const cacheDir = path.join(venvDir, ".cache");
  fs.mkdirSync(path.join(cacheDir, "matplotlib"), { recursive: true });
  const result = spawnSync(venvPython, ["-c", "import fastapi, uvicorn, nemo.collections.asr, torch, torchaudio"], {
    cwd: sidecarDir,
    env: {
      ...process.env,
      MPLCONFIGDIR: path.join(cacheDir, "matplotlib"),
      XDG_CACHE_HOME: cacheDir,
    },
    stdio: "ignore",
  });

  return result.status === 0;
}

function hasCurrentRequirements(requirementsHash) {
  if (!fs.existsSync(markerPath)) return false;

  return fs.readFileSync(markerPath, "utf8").trim() === requirementsHash;
}

function createVirtualEnvIfMissing() {
  if (fs.existsSync(venvPython)) return;

  console.log("Creating ASR engine Python environment in sidecar/.venv...");
  run(bootstrapPython, ["-m", "venv", venvDir]);
}

function installRequirements(requirementsHash) {
  console.log("Installing ASR engine Python dependencies from sidecar/requirements.txt...");
  run(venvPython, [
    "-m",
    "pip",
    "--disable-pip-version-check",
    "install",
    "-r",
    requirementsPath,
  ]);
  fs.writeFileSync(markerPath, `${requirementsHash}\n`);
}

function ensureSidecarEnv() {
  const requirementsHash = getRequirementsHash();

  createVirtualEnvIfMissing();

  if (hasCurrentRequirements(requirementsHash) && canImportCoreSidecarDeps()) {
    return;
  }

  installRequirements(requirementsHash);
}

ensureSidecarEnv();

if (shouldRunSidecar) {
  run(venvPython, [sidecarMainPath], { cwd: sidecarDir });
}
