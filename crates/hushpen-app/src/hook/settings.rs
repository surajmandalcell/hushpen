//! Test hook for settings and for the responsiveness of the main thread.
//! Compiled only with the `test-automation` feature.

use super::{record_event, register_action};
use crate::storage::Storage;
use gpui_kit::App;
use serde_json::{Value, json};
use std::rc::Rc;
use std::time::{Duration, Instant};

/// How often the main thread looks at the clock.
const BEAT: Duration = Duration::from_millis(10);
/// A gap between two beats longer than this is a stall the user could feel.
const STALL: Duration = Duration::from_millis(100);

pub fn attach(cx: &mut App, storage: Rc<Storage>) {
    let _ = register_action(
        cx,
        "set-setting",
        "Set one user-facing setting like the Settings view does. Args: {\"key\": \"audio.cueSounds\", \"value\": false}. \
         The value is checked against the setting's kind and range.",
        move |_, args| {
            let key = args.get("key").and_then(Value::as_str).ok_or_else(|| {
                "needs args like {\"key\": \"audio.cueSounds\", \"value\": false}".to_string()
            })?;
            let value = args
                .get("value")
                .cloned()
                .ok_or_else(|| "needs a \"value\"".to_string())?;
            storage
                .settings
                .set(key, value)
                .map_err(|error| error.to_string())?;
            Ok(json!({"key": key, "value": storage.settings.get(key)}))
        },
    );
    watch_stalls(cx);
}

/// Records `ui-stall <ms>` for every time the main thread did not get back to a timer for
/// longer than [`STALL`], so a test can prove that a key press never freezes the window.
fn watch_stalls(cx: &mut App) {
    cx.spawn(async move |cx| {
        let mut last = Instant::now();
        loop {
            cx.background_executor().timer(BEAT).await;
            let now = Instant::now();
            let gap = now.duration_since(last);
            if gap > STALL {
                record_event("ui-stall", &format!("{}ms", gap.as_millis()));
            }
            last = now;
        }
    })
    .detach();
}
