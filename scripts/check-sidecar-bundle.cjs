#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");

const rootDir = path.resolve(__dirname, "..");
const executableName = process.platform === "win32" ? "asrpro-sidecar.exe" : "asrpro-sidecar";
const executablePath = path.join(rootDir, "sidecar", "bin", executableName);

if (!fs.existsSync(executablePath)) {
  console.error(`Missing bundled ASR engine executable: ${executablePath}`);
  console.error("Run `npm run sidecar:build` on this OS before creating release installers.");
  process.exit(1);
}

console.log(`Found bundled ASR engine executable: ${executablePath}`);
