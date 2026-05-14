const { contextBridge, ipcRenderer } = require("electron");

const allowedWindowActions = new Set(["minimize", "maximize", "close"]);

contextBridge.exposeInMainWorld("asrpro", {
  getPlatform: () => ipcRenderer.invoke("app:platform"),
  getAppInfo: () => ipcRenderer.invoke("app:info"),
  getRuntimeState: () => ipcRenderer.invoke("runtime:state"),
  getSidecarState: () => ipcRenderer.invoke("sidecar:state"),
  getOverlaySettings: () => ipcRenderer.invoke("overlay-settings:get"),
  setOverlaySettings: (settings) => ipcRenderer.invoke("overlay-settings:update", settings),
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
  onSidecarState: (callback) => {
    const listener = (_event, state) => callback(state);
    ipcRenderer.on("sidecar:state", listener);
    return () => ipcRenderer.removeListener("sidecar:state", listener);
  },
  windowControl: (action) => {
    if (!allowedWindowActions.has(action)) {
      return Promise.resolve();
    }
    return ipcRenderer.invoke("window:control", action);
  },
});
