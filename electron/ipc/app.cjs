const { BrowserWindow } = require("electron");
const { EXTERNAL_TARGETS, FOLDER_TARGETS } = require("../shell/openTargets.cjs");
const { v } = require("./validate.cjs");

const OPEN_TARGETS = [...Object.keys(EXTERNAL_TARGETS), ...FOLDER_TARGETS];

function registerAppIpc({ router, ctx, getRuntimeState, openTargets }) {
  router.handle("app:platform", v.none(), () => ({
    platform: process.platform,
    arch: process.arch,
    versions: {
      electron: process.versions.electron,
      chrome: process.versions.chrome,
      node: process.versions.node,
    },
  }));

  router.handle("app:info", v.none(), () => ({
    name: ctx.app.getName(),
    version: ctx.app.getVersion(),
  }));

  router.handle("shell:open", v.object({ target: v.oneOf(OPEN_TARGETS) }), ({ target }) => openTargets.open(target));

  router.handle("runtime:state", v.none(), () => getRuntimeState());

  router.handle("engine:get-state", v.none(), () => ctx.state.engine);

  router.handle("window:control", v.object({ action: v.oneOf(["minimize", "close"]) }), ({ action }, event) => {
    const senderWindow = BrowserWindow.fromWebContents(event.sender);
    if (!senderWindow) return;

    if (action === "minimize") senderWindow.minimize();
    if (action === "close") senderWindow.close();
  });
}

module.exports = { registerAppIpc };
