#!/usr/bin/env node

const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const rootDir = path.resolve(__dirname, "..");
const sidecarDir = path.join(rootDir, "sidecar");
const localPython = process.platform === "win32"
  ? path.join(sidecarDir, ".venv", "Scripts", "python.exe")
  : path.join(sidecarDir, ".venv", "bin", "python");
const python = process.env.PYTHON || (fs.existsSync(localPython) ? localPython : (process.platform === "win32" ? "python" : "python3"));
const addData = `${path.join(sidecarDir, "models", "onnx")}${path.delimiter}${path.join("models", "onnx")}`;

const args = [
  "-m",
  "PyInstaller",
  "--name",
  "asrpro-sidecar",
  "--onefile",
  "--clean",
  "--distpath",
  "bin",
  "--workpath",
  "build",
  "--specpath",
  "build",
  "--add-data",
  addData,
  "--collect-submodules",
  "uvicorn",
  "--collect-submodules",
  "fastapi",
  "--collect-submodules",
  "onnx_asr",
  "--collect-submodules",
  "nemo",
  "--collect-submodules",
  "torch",
  "--collect-submodules",
  "torchaudio",
  "--copy-metadata",
  "onnx-asr",
  "--copy-metadata",
  "onnxruntime",
  "--copy-metadata",
  "nemo-toolkit",
  "--copy-metadata",
  "torch",
  "--copy-metadata",
  "torchaudio",
  "main.py",
];

const result = spawnSync(python, args, {
  cwd: sidecarDir,
  stdio: "inherit",
});

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);
