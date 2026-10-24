//! Test hook for the Settings view: the `settings_view` state section and the search. The
//! view is driven by its own controls through their element ids, like a user would. Compiled
//! only with the `test-automation` feature.

use super::{register_action, set_state_section};
use crate::settings::Settings;
use gpui_kit::{AnyWindowHandle, App, Entity};
use serde_json::Value;

pub fn attach(cx: &mut App, settings: Entity<Settings>, window: AnyWindowHandle) {
    set_state_section(cx, "settings_view", {
        let settings = settings.clone();
        move |cx| settings.read(cx).hook_state_json(cx)
    });
    let _ = register_action(
        cx,
        "settings-search",
        "Type a query into the Settings search field, so only the matching rows show. Args: \
         {\"query\": \"mic\"}. An empty query shows every row again.",
        move |cx, args| {
            let query = args.get("query").and_then(Value::as_str).unwrap_or("");
            window
                .update(cx, |_, window, cx| {
                    settings.update(cx, |settings, cx| settings.search_for(query, window, cx));
                })
                .map_err(|error| error.to_string())?;
            Ok(Value::Null)
        },
    );
}
