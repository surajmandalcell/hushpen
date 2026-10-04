#!/usr/bin/env node

const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const { patchLinuxAddonDir } = require("./patch-elf-runpath.cjs");

function getPackageRoot() {
  return path.dirname(require.resolve("@kutalia/whisper-node-addon/package.json"));
}

function getNativeDir() {
  if (process.platform === "darwin") return `mac-${process.arch}`;
  if (process.platform === "linux") return `linux-${process.arch}`;
  if (process.platform === "win32") return `win32-${process.arch}`;
  return "";
}

function run(command, args) {
  const result = spawnSync(command, args, { encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed: ${result.stderr || result.stdout}`);
  }
  return result.stdout;
}

function hasLoaderPathRpath(filePath) {
  const output = run("otool", ["-l", filePath]);
  return output.includes("path @loader_path ");
}

function prepareDarwinAddon() {
  const nativeDir = getNativeDir();
  const addonPath = path.join(getPackageRoot(), "dist", nativeDir, "whisper.node");
  if (!fs.existsSync(addonPath)) {
    throw new Error(`Native Whisper addon binary is missing at ${addonPath}.`);
  }

  if (hasLoaderPathRpath(addonPath)) return;

  run("install_name_tool", ["-add_rpath", "@loader_path", addonPath]);
}

function prepareLinuxAddons() {
  // Runs on every host OS: Linux artifacts can be cross-built from macOS, and
  // the upstream binaries carry a CI-runner RUNPATH instead of $ORIGIN.
  const distDir = path.join(getPackageRoot(), "dist");
  if (!fs.existsSync(distDir)) return;

  for (const entry of fs.readdirSync(distDir, { withFileTypes: true })) {
    if (!entry.isDirectory() || !entry.name.startsWith("linux-")) continue;
    patchLinuxAddonDir(path.join(distDir, entry.name));
  }
}

try {
  if (process.platform === "darwin") {
    prepareDarwinAddon();
  }
  prepareLinuxAddons();
} catch (error) {
  console.error("Failed to prepare native Whisper addon.");
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
}
