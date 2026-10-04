const { v } = require("./validate.cjs");

function registerRecordingIpc({ router, recording, overlay }) {
  router.handle("recording:set", v.object({ active: v.boolean() }), ({ active }) => {
    recording.set(active, "renderer");
    return recording.getState();
  });

  router.handle("recording:toggle", v.none(), () => {
    recording.toggle("renderer");
    return recording.getState();
  });

  router.on("recording:waveform-frame", v.numberArray(), (frame) => {
    overlay.updateWaveformFrame(frame);
  });
}

module.exports = { registerRecordingIpc };
