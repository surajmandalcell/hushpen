#!/usr/bin/env node

const { spawnSync } = require("node:child_process");
const path = require("node:path");

const platforms = {
  mac: "--mac",
  win: "--win",
  linux: "--linux",
};

const [platform, ...builderArgs] = process.argv.slice(2);
const platformFlag = platforms[platform];

if (!platformFlag) {
  console.error("Usage: node scripts/build-electron.cjs <mac|win|linux> [...electron-builder args]");
  process.exit(1);
}

const rootDir = path.resolve(__dirname, "..");

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: rootDir,
    stdio: "inherit",
    shell: process.platform === "win32",
  });

  if (result.error) {
    console.error(result.error.message);
    process.exit(1);
  }

  if (result.status !== 0) {
    process.exit(result.status || 1);
  }
}

const electronBuilderBin = path.join(
  rootDir,
  "node_modules",
  ".bin",
  process.platform === "win32" ? "electron-builder.cmd" : "electron-builder",
);

run("npm", ["run", "build"]);
run("npm", ["run", "engine:check"]);
run(electronBuilderBin, [platformFlag, ...builderArgs]);
