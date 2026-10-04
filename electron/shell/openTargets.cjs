const { AppError } = require("../core/errors.cjs");

const REPOSITORY_URL = "https://github.com/surajmandalcell/asrpro";
const REPOSITORY_PATH = new URL(REPOSITORY_URL).pathname;

const EXTERNAL_TARGETS = Object.freeze({
  repo: REPOSITORY_URL,
  issues: `${REPOSITORY_URL}/issues/new`,
  releases: `${REPOSITORY_URL}/releases`,
});
const FOLDER_TARGETS = Object.freeze(["data-folder", "log-folder"]);

/** The last check before a URL reaches the system browser; it holds even if a target table is edited wrongly. */
function isAllowedExternalUrl(value) {
  let url;
  try {
    url = new URL(value);
  } catch {
    return false;
  }
  return url.protocol === "https:"
    && url.hostname === "github.com"
    && !url.username
    && !url.password
    && !url.port
    && (url.pathname === REPOSITORY_PATH || url.pathname.startsWith(`${REPOSITORY_PATH}/`));
}

function createOpenTargets({ ctx, shell = require("electron").shell }) {
  const folders = {
    "data-folder": () => ctx.dataDir,
    "log-folder": () => ctx.layout.logsDir,
  };

  async function open(target) {
    if (Object.hasOwn(EXTERNAL_TARGETS, target)) {
      const url = EXTERNAL_TARGETS[target];
      if (!isAllowedExternalUrl(url)) throw new AppError("INTERNAL", { target }, "Target URL is not allow-listed.");
      await shell.openExternal(url);
      return;
    }

    if (Object.hasOwn(folders, target)) {
      const failure = await shell.openPath(folders[target]());
      if (failure) throw new AppError("INTERNAL", { target }, "The folder could not be opened.");
      return;
    }

    throw new AppError("INVALID_ARGUMENT", { path: "target" }, "target must be a known open target");
  }

  return { open };
}

module.exports = { EXTERNAL_TARGETS, FOLDER_TARGETS, createOpenTargets, isAllowedExternalUrl };
