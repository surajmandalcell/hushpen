const { contextBridge, ipcRenderer } = require("electron");

// A sandboxed preload cannot read shared/ipc-channels.json, so these lists are
// copies. tests/unit/ipcContract.test.ts fails when they drift from the JSON.
const INVOKE_CHANNELS = new Set([
  "app:platform",
  "app:info",
  "window:control",
  "shell:open",
  "runtime:state",
  "settings:get-all",
  "settings:set",
  "settings:import-legacy",
  "recording:set",
  "recording:toggle",
  "engine:get-state",
  "engine:transcribe-audio",
  "models:list",
  "models:download",
  "models:delete",
  "transcript:open-text",
  "transcript:delete-text",
]);
const SEND_CHANNELS = new Set([
  "recording:waveform-frame",
]);
const PUSH_CHANNELS = new Set([
  "settings:changed",
  "recording:state",
  "engine:state",
]);

contextBridge.exposeInMainWorld("asrpro", {
  isScreenshotMode: process.env.ASRPRO_SCREENSHOT_MODE === "1",
  invoke: (channel, payload) => {
    if (!INVOKE_CHANNELS.has(channel)) {
      return Promise.resolve({ ok: false, error: { code: "INVALID_ARGUMENT", params: { path: "channel" } } });
    }
    return ipcRenderer.invoke(channel, payload);
  },
  send: (channel, payload) => {
    if (SEND_CHANNELS.has(channel)) ipcRenderer.send(channel, payload);
  },
  on: (channel, callback) => {
    if (!PUSH_CHANNELS.has(channel)) return () => {};
    const listener = (_event, payload) => callback(payload);
    ipcRenderer.on(channel, listener);
    return () => ipcRenderer.removeListener(channel, listener);
  },
});
