const fs = require("node:fs");
const path = require("node:path");
const { APP_NAME } = require("../identity.cjs");
const { buildLinuxAutostartDesktopEntry } = require("../runtime.cjs");

function readTextFile(filePath) {
  try {
    return fs.readFileSync(filePath, "utf8");
  } catch {
    return "";
  }
}

function getLoginItemOptions(platform, executablePath) {
  if (platform !== "win32") return {};

  return {
    path: executablePath,
    args: [],
  };
}

function readLinuxAutostartExecutablePath(autostartPath) {
  const source = readTextFile(autostartPath);
  const execLine = source.split(/\r?\n/).find((line) => line.startsWith("Exec="));
  if (!execLine) return "";

  const execValue = execLine.slice("Exec=".length).trim();
  const quotedMatch = execValue.match(/^"((?:\\.|[^"])*)"/);
  if (quotedMatch) {
    return quotedMatch[1].replace(/\\"/g, '"').replace(/\\\\/g, "\\");
  }

  return execValue.split(/\s+/)[0] || "";
}

function resolveExecutablePath({ app, platform, env }) {
  if (platform === "win32" && env.PORTABLE_EXECUTABLE_FILE) {
    return env.PORTABLE_EXECUTABLE_FILE;
  }

  return app.getPath("exe");
}

function createStartup({ ctx }) {
  const { app, platform, env } = ctx;

  function getCurrentExecutablePath() {
    return resolveExecutablePath({ app, platform, env });
  }

  function getLinuxAutostartFilePath() {
    const configHome = env.XDG_CONFIG_HOME || path.join(app.getPath("home"), ".config");
    return path.join(configHome, "autostart", "asrpro.desktop");
  }

  function setLoginItem(executablePath, openAtLogin) {
    if (platform === "linux") {
      if (openAtLogin) {
        const autostartPath = getLinuxAutostartFilePath();
        fs.mkdirSync(path.dirname(autostartPath), { recursive: true });
        fs.writeFileSync(autostartPath, buildLinuxAutostartDesktopEntry({
          appName: APP_NAME,
          executablePath,
        }), "utf8");
      } else {
        fs.rmSync(getLinuxAutostartFilePath(), { force: true });
      }
      return;
    }

    if (platform === "darwin" || platform === "win32") {
      const loginItemSettings = {
        ...getLoginItemOptions(platform, executablePath),
        openAtLogin,
      };
      if (platform === "win32") {
        loginItemSettings.enabled = openAtLogin;
      }
      app.setLoginItemSettings(loginItemSettings);
    }
  }

  function getLinuxState(executablePath) {
    const autostartPath = getLinuxAutostartFilePath();
    const registeredExecutablePath = readLinuxAutostartExecutablePath(autostartPath);
    const enabled = fs.existsSync(autostartPath)
      && !readTextFile(autostartPath).includes("X-GNOME-Autostart-enabled=false")
      && registeredExecutablePath === executablePath;

    return {
      supported: true,
      enabled,
      executablePath,
      registeredExecutablePath,
      autostartPath,
    };
  }

  function getState() {
    const executablePath = getCurrentExecutablePath();
    const supported = ["darwin", "win32", "linux"].includes(platform);

    if (!supported) {
      return {
        supported: false,
        enabled: false,
        executablePath,
        detail: "Startup launch is not available on this platform.",
      };
    }

    if (platform === "linux") {
      return getLinuxState(executablePath);
    }

    const loginSettings = app.getLoginItemSettings(getLoginItemOptions(platform, executablePath));
    return {
      supported: true,
      enabled: Boolean(loginSettings.openAtLogin),
      executablePath,
      registeredExecutablePath: ctx.settings.get("startup.executablePath") || executablePath,
      status: loginSettings.status,
      requiresApproval: loginSettings.status === "requires-approval",
    };
  }

  /** Registers or removes the login item and returns the settings patch to store. */
  function apply(enabled) {
    const executablePath = getCurrentExecutablePath();
    const previousExecutablePath = ctx.settings.get("startup.executablePath");

    if (previousExecutablePath && previousExecutablePath !== executablePath) {
      setLoginItem(previousExecutablePath, false);
    }

    setLoginItem(executablePath, Boolean(enabled));

    return {
      "startup.launchAtLogin": Boolean(enabled),
      "startup.executablePath": executablePath,
    };
  }

  return { apply, getCurrentExecutablePath, getState };
}

module.exports = { createStartup, readLinuxAutostartExecutablePath, resolveExecutablePath };
