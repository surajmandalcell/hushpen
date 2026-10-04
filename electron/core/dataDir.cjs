const fs = require("node:fs");
const path = require("node:path");
const { buildModelPaths, resolveContainedDataDir } = require("../runtime.cjs");

function resolveDataDir({ app, env = process.env, platform = process.platform, executablePath }) {
  return resolveContainedDataDir({
    isPackaged: app.isPackaged,
    platform,
    resourcesPath: process.resourcesPath,
    exePath: executablePath,
    appPath: app.getAppPath(),
    userDataPath: app.getPath("userData"),
    dataDirOverride: env.ASRPRO_DATA_DIR,
    portableExecutableDir: env.PORTABLE_EXECUTABLE_DIR,
  });
}

function ensureDataLayout(dataDir) {
  const paths = buildModelPaths(dataDir);
  const directories = [
    dataDir,
    path.join(dataDir, "config"),
    path.join(dataDir, "session"),
    path.join(dataDir, "logs"),
    path.join(dataDir, "transcripts"),
    paths.modelsDir,
    paths.whisperModelsDir,
  ];
  for (const directory of directories) {
    fs.mkdirSync(directory, { recursive: true });
  }
  return {
    dataDir,
    configDir: path.join(dataDir, "config"),
    logsDir: path.join(dataDir, "logs"),
    sessionDir: path.join(dataDir, "session"),
    transcriptsDir: path.join(dataDir, "transcripts"),
    userDataDir: path.join(dataDir, "user-data"),
    cacheDir: path.join(dataDir, "cache"),
  };
}

module.exports = { ensureDataLayout, resolveDataDir };
