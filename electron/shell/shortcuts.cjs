const { globalShortcut } = require("electron");
const { RECORDING_SHORTCUT } = require("../runtime.cjs");

function registerGlobalShortcut({ actions }) {
  return globalShortcut.register(RECORDING_SHORTCUT, () => {
    actions.toggleRecording("shortcut");
  });
}

function unregisterGlobalShortcuts() {
  globalShortcut.unregisterAll();
}

module.exports = { registerGlobalShortcut, unregisterGlobalShortcuts };
