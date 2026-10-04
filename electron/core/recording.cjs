const { RECORDING_SHORTCUT, shouldShowRecordingOverlay } = require("../runtime.cjs");
const { getModelById } = require("../whisper-engine.cjs");

function createRecordingController({ ctx, overlay }) {
  function getState(source = "app") {
    return {
      isRecording: ctx.state.isRecording,
      source,
      model: getModelById(ctx.settings.get("transcription.modelId")).displayName,
      shortcut: RECORDING_SHORTCUT,
    };
  }

  function emitState(source) {
    ctx.pushToMain("recording:state", getState(source));
  }

  function set(active, source = "app") {
    if (ctx.state.isRecording === active) {
      if (active && shouldShowRecordingOverlay(source)) {
        overlay.show();
      }
      if (active && !shouldShowRecordingOverlay(source)) {
        overlay.hide();
      }
      emitState(source);
      return;
    }

    ctx.state.isRecording = active;
    if (active && shouldShowRecordingOverlay(source)) {
      overlay.show();
    } else {
      overlay.hide();
    }

    ctx.events.emit("recording-changed", active);
    emitState(source);
  }

  function toggle(source = "app") {
    set(!ctx.state.isRecording, source);
  }

  return { getState, set, toggle };
}

module.exports = { createRecordingController };
