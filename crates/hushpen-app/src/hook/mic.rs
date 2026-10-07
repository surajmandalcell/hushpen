//! Test hook for the microphone: the `capture` state section, the actions that
//! start, stop, and select, and the WAV feeder behind `hookctl feed-wav`.
//! Compiled only with the `test-automation` feature.

use super::{register_action, set_state_section, set_wav_feeder};
use crate::mic::Mic;
use gpui_kit::{App, Entity};
use serde_json::{Value, json};

pub fn attach(cx: &mut App, mic: Entity<Mic>) {
    set_state_section(cx, "capture", {
        let mic = mic.clone();
        move |cx| mic.read(cx).state_json()
    });

    let _ = register_action(
        cx,
        "capture-start",
        "Start a capture session like the Home test button. Returns the session WAV path.",
        {
            let mic = mic.clone();
            move |cx, _| {
                mic.update(cx, |mic, cx| mic.start(cx))
                    .map(|path| json!({"path": path.to_string_lossy()}))
                    .map_err(|error| error.to_string())
            }
        },
    );
    let _ = register_action(
        cx,
        "capture-stop",
        "Stop the capture session and fix the WAV header. Args: {\"keep\": true} (default true; false deletes the file).",
        {
            let mic = mic.clone();
            move |cx, args| {
                let keep = args.get("keep").and_then(Value::as_bool).unwrap_or(true);
                mic.update(cx, |mic, cx| mic.stop(keep, cx))
                    .map(|finished| match finished {
                        Some(finished) => json!({
                            "path": finished.path.to_string_lossy(),
                            "samples": finished.samples,
                            "duration_ms": finished.duration_ms,
                        }),
                        None => Value::Null,
                    })
                    .map_err(|error| error.to_string())
            }
        },
    );
    let _ = register_action(
        cx,
        "capture-select",
        "Choose the microphone like the picker does. Args: {\"id\": \"default\"} or an id from `capture.devices`.",
        {
            let mic = mic.clone();
            move |cx, args| {
                let id = args
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "needs args like {\"id\": \"default\"}".to_string())?;
                mic.update(cx, |mic, cx| mic.select(id, cx))?;
                Ok(json!({"id": id}))
            }
        },
    );
    let _ = register_action(
        cx,
        "capture-refresh",
        "List the microphones again now instead of waiting for the 2 s poll.",
        {
            let mic = mic.clone();
            move |cx, _| {
                mic.update(cx, |mic, cx| mic.refresh(cx));
                Ok(Value::Null)
            }
        },
    );
    set_wav_feeder(cx, move |cx, path| {
        mic.update(cx, |mic, _| mic.feed_wav(path))
    });
}
