const { contextBridge, ipcRenderer } = require("electron");

const allowedWindowActions = new Set(["minimize", "maximize", "close"]);

contextBridge.exposeInMainWorld("asrpro", {
  getPlatform: () => ipcRenderer.invoke("app:platform"),
  getAppInfo: () => ipcRenderer.invoke("app:info"),
  getRuntimeState: () => ipcRenderer.invoke("runtime:state"),
  getOverlaySettings: () => ipcRenderer.invoke("overlay-settings:get"),
  setOverlaySettings: (settings) => ipcRenderer.invoke("overlay-settings:update", settings),
  selectAudioFiles: () => ipcRenderer.invoke("dialog:select-audio"),
  onAddFiles: (callback) => {
    const listener = () => callback();
    ipcRenderer.on("menu:add-files", listener);
    return () => ipcRenderer.removeListener("menu:add-files", listener);
  },
  setRecording: (active) => ipcRenderer.invoke("recording:set", Boolean(active)),
  toggleRecording: () => ipcRenderer.invoke("recording:toggle"),
  setWaveformFrame: (frame) => {
    ipcRenderer.send("recording:waveform-frame", Array.isArray(frame) ? frame : []);
  },
  onRecordingState: (callback) => {
    const listener = (_event, state) => callback(state);
    ipcRenderer.on("recording:state", listener);
    return () => ipcRenderer.removeListener("recording:state", listener);
  },
  windowControl: (action) => {
    if (!allowedWindowActions.has(action)) {
      return Promise.resolve();
    }
    return ipcRenderer.invoke("window:control", action);
  },
});
