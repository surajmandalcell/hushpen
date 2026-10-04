const { BrowserWindow } = require("electron");
const { v } = require("./validate.cjs");

function registerAppIpc({ router, ctx, getRuntimeState }) {
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
