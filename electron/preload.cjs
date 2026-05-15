const { contextBridge, ipcRenderer } = require("electron");

const allowedWindowActions = new Set(["minimize", "close"]);

contextBridge.exposeInMainWorld("asrpro", {
  isScreenshotMode: process.env.ASRPRO_SCREENSHOT_MODE === "1",
  getPlatform: () => ipcRenderer.invoke("app:platform"),
  getAppInfo: () => ipcRenderer.invoke("app:info"),
  getRuntimeState: () => ipcRenderer.invoke("runtime:state"),
  getEngineState: () => ipcRenderer.invoke("engine:state"),
  getModels: () => ipcRenderer.invoke("engine:models"),
  transcribeAudio: (request) => ipcRenderer.invoke("engine:transcribe-audio", request),
  openTranscriptText: (request) => ipcRenderer.invoke("transcript:open-text", request),
  deleteTranscriptText: (request) => ipcRenderer.invoke("transcript:delete-text", request),
  setDefaultTextEditor: (editorId) => ipcRenderer.invoke("settings:text-editor", editorId),
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
  onEngineState: (callback) => {
    const listener = (_event, state) => callback(state);
    ipcRenderer.on("engine:state", listener);
    return () => ipcRenderer.removeListener("engine:state", listener);
  },
  windowControl: (action) => {
    if (!allowedWindowActions.has(action)) {
      return Promise.resolve();
    }
    return ipcRenderer.invoke("window:control", action);
  },
});
