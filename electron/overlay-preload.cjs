const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld("asrproOverlay", {
  onWaveformFrame: (callback) => {
    const listener = (_event, frame) => {
      callback(Array.isArray(frame) ? frame : []);
    };

    ipcRenderer.on("overlay:waveform-frame", listener);
    return () => ipcRenderer.removeListener("overlay:waveform-frame", listener);
  },
});
