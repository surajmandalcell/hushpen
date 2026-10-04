const { Menu } = require("electron");
const { APP_NAME } = require("../identity.cjs");
const { RECORDING_SHORTCUT } = require("../runtime.cjs");

function createMenu({ ctx, actions }) {
  return Menu.buildFromTemplate([
    {
      label: APP_NAME,
      submenu: [
        { label: `About ${APP_NAME}`, click: () => ctx.app.showAboutPanel() },
        { type: "separator" },
        { role: "hide" },
        { role: "hideOthers" },
        { role: "unhide" },
        { type: "separator" },
        { label: `Quit ${APP_NAME}`, accelerator: "CmdOrCtrl+Q", click: actions.quit },
      ],
    },
    {
      label: "File",
      submenu: [
        { label: "Start or Stop Recording", accelerator: RECORDING_SHORTCUT, click: () => actions.toggleRecording("menu") },
      ],
    },
    {
      label: "Edit",
      submenu: [{ role: "undo" }, { role: "redo" }, { type: "separator" }, { role: "cut" }, { role: "copy" }, { role: "paste" }],
    },
    {
      label: "View",
      submenu: [{ role: "reload" }, { role: "toggleDevTools" }, { type: "separator" }, { role: "resetZoom" }, { role: "zoomIn" }, { role: "zoomOut" }],
    },
    {
      label: "Window",
      submenu: [{ label: `Show ${APP_NAME}`, click: actions.showMainWindow }, { role: "minimize" }, { role: "close" }],
    },
  ]);
}

module.exports = { createMenu };
