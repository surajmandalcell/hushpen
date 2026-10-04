const { Menu, Tray, nativeImage, nativeTheme } = require("electron");
const { APP_NAME } = require("../identity.cjs");
const { RECORDING_SHORTCUT, resolveTrayIconPath } = require("../runtime.cjs");

function createTrayController({ ctx, actions, getAssetRoot }) {
  let tray;

  function createIcon() {
    const iconPath = resolveTrayIconPath(ctx.platform, getAssetRoot(), nativeTheme.shouldUseDarkColors);
    const icon = nativeImage.createFromPath(iconPath);
    if (ctx.platform === "darwin") {
      icon.setTemplateImage(true);
    }
    return icon;
  }

  function updateMenu() {
    if (!tray) return;

    tray.setContextMenu(Menu.buildFromTemplate([
      { label: `Show ${APP_NAME}`, click: actions.showMainWindow },
      { type: "separator" },
      {
        label: ctx.state.isRecording ? "Stop Recording" : "Start Recording",
        accelerator: RECORDING_SHORTCUT,
        click: () => actions.toggleRecording("tray"),
      },
      { type: "separator" },
      { label: `Quit ${APP_NAME}`, click: actions.quit },
    ]));
  }

  function create() {
    if (tray) return;

    tray = new Tray(createIcon());
    tray.setToolTip(APP_NAME);
    tray.on("click", actions.showMainWindow);
    updateMenu();
  }

  function updateIcon() {
    if (!tray) return;
    tray.setImage(createIcon());
  }

  function destroy() {
    if (tray) {
      tray.destroy();
      tray = undefined;
    }
  }

  return { create, destroy, updateIcon, updateMenu };
}

module.exports = { createTrayController };
