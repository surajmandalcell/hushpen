const { contextBridge, ipcRenderer } = require("electron");

const allowedWindowActions = new Set(["minimize", "maximize", "close"]);

contextBridge.exposeInMainWorld("asrpro", {
  getPlatform: () => ipcRenderer.invoke("app:platform"),
  getAppInfo: () => ipcRenderer.invoke("app:info"),
  selectAudioFiles: () => ipcRenderer.invoke("dialog:select-audio"),
  onAddFiles: (callback) => {
    const listener = () => callback();
    ipcRenderer.on("menu:add-files", listener);
    return () => ipcRenderer.removeListener("menu:add-files", listener);
  },
  windowControl: (action) => {
    if (!allowedWindowActions.has(action)) {
      return Promise.resolve();
    }
    return ipcRenderer.invoke("window:control", action);
  },
});
