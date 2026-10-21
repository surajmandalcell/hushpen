//! Test hook for the tray: the `tray_menu` state section, the menu as it is built now, and the
//! `tray-click` action. A test on Linux clicks the real entry over D-Bus; the macOS status item
//! menu has no reachable accessibility tree, so `tray-click` runs an entry the way a click does.
//! Compiled only with the `test-automation` feature.

use super::{register_action, set_state_section};
use crate::controller::Controller;
use crate::tray;
use gpui_kit::{App, Entity};
use serde_json::{Value, json};

pub fn attach(cx: &mut App, controller: Entity<Controller>, created: bool) {
    set_state_section(cx, "tray_menu", move |cx| {
        tray::menu_json(tray::listening(&controller, cx), created)
    });
    let _ = register_action(
        cx,
        "tray-click",
        "Run a tray menu entry the way a click on it does. Args: {\"id\": \"toggle-dictation\" | \
         \"paste-last\" | \"show\" | \"settings\" | \"quit\"}, the ids of the tray_menu section.",
        |cx, args| {
            let id = args
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| "needs args like {\"id\": \"paste-last\"}".to_string())?;
            tray::click(id, cx)?;
            Ok(json!({"id": id}))
        },
    );
}
