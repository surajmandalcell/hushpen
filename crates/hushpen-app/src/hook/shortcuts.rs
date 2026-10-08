//! Test hook for the Shortcuts section: the `shortcuts` state section and the actions that open
//! the recorder, close it, and reset the shortcuts. The keys themselves are driven with
//! `xdotool`, because the recorder listens to the global key stream.
//! Compiled only with the `test-automation` feature.

use super::{register_action, set_state_section};
use crate::shortcuts::Shortcuts;
use gpui_kit::{App, Entity};
use hushpen_core::shortcut::Slot;
use serde_json::{Value, json};

pub fn attach(cx: &mut App, shortcuts: Entity<Shortcuts>) {
    set_state_section(cx, "shortcuts", {
        let shortcuts = shortcuts.clone();
        move |cx| shortcuts.read(cx).state_json()
    });
    let _ = register_action(
        cx,
        "shortcut-record",
        "Open the recorder for one shortcut, like clicking its field. \
         Args: {\"slot\": \"hold\" | \"handsFree\" | \"pasteLast\" | \"command\"}.",
        {
            let shortcuts = shortcuts.clone();
            move |cx, args| {
                let name = args
                    .get("slot")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "needs args like {\"slot\": \"hold\"}".to_string())?;
                let slot = Slot::from_key(name)
                    .ok_or_else(|| format!("'{name}' is not a shortcut slot"))?;
                shortcuts.update(cx, |shortcuts, cx| shortcuts.start_recording(slot, cx))?;
                Ok(json!({"slot": name}))
            }
        },
    );
    let _ = register_action(
        cx,
        "shortcut-cancel",
        "Close the open recorder without a change, like clicking its field again.",
        {
            let shortcuts = shortcuts.clone();
            move |cx, _| {
                shortcuts.update(cx, |shortcuts, cx| shortcuts.cancel_recording(cx));
                Ok(Value::Null)
            }
        },
    );
    let _ = register_action(
        cx,
        "shortcut-reset",
        "Put all four shortcuts back to their defaults, like the Reset to default button.",
        move |cx, _| {
            shortcuts.update(cx, |shortcuts, cx| shortcuts.reset_all(cx))?;
            Ok(Value::Null)
        },
    );
}
