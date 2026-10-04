const { pathToFileURL } = require("node:url");

function resolveAppUrl({ isPackaged, devServerUrl, distIndexPath }) {
  return isPackaged ? pathToFileURL(distIndexPath).href : devServerUrl;
}

function parse(value) {
  if (typeof value !== "string" || !value) return undefined;
  try {
    return new URL(value);
  } catch {
    return undefined;
  }
}

/**
 * True when `candidate` is the app page. A file page is compared by path, since
 * every file URL shares one opaque origin; the bare "file:///" origin that
 * permission checks report is accepted for the same reason. Any other page is
 * compared by origin.
 */
function isAppUrl(candidate, appUrl) {
  const url = parse(candidate);
  const app = parse(appUrl);
  if (!url || !app || url.protocol !== app.protocol) return false;

  if (app.protocol === "file:") {
    return url.pathname === app.pathname || (url.pathname === "/" && !url.search && !url.hash);
  }
  return url.origin === app.origin;
}

module.exports = { isAppUrl, resolveAppUrl };
