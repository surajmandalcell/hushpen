const path = require("node:path");

/** Both windows run with these flags; only the preload differs. */
function secureWebPreferences(preloadFileName) {
  return {
    preload: path.join(__dirname, "..", preloadFileName),
    contextIsolation: true,
    nodeIntegration: false,
    nodeIntegrationInSubFrames: false,
    sandbox: true,
    webSecurity: true,
    webviewTag: false,
    allowRunningInsecureContent: false,
    backgroundThrottling: false,
  };
}

module.exports = { secureWebPreferences };
