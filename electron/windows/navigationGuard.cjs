const { isAppUrl } = require("./appUrl.cjs");

function guardWebContents(contents, appUrl) {
  const blockForeign = (event, url) => {
    if (!isAppUrl(url ?? event.url, appUrl)) event.preventDefault();
  };

  contents.on("will-navigate", blockForeign);
  contents.on("will-frame-navigate", blockForeign);
  contents.on("will-redirect", blockForeign);
  contents.on("will-attach-webview", (event) => event.preventDefault());
  contents.setWindowOpenHandler(() => ({ action: "deny" }));
}

/** Guards every web contents the app ever creates, so a window added later cannot skip it. */
function installNavigationGuard({ app, appUrl }) {
  app.on("web-contents-created", (_event, contents) => guardWebContents(contents, appUrl));
}

module.exports = { guardWebContents, installNavigationGuard };
