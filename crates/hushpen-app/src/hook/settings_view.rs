//! Test hook for the Settings view: the `settings_view` state section. The
//! view is driven by its own buttons through their element ids, like a user
//! would. Compiled only with the `test-automation` feature.

use super::set_state_section;
use crate::settings::Settings;
use gpui_kit::{App, Entity};

pub fn attach(cx: &mut App, settings: Entity<Settings>) {
    set_state_section(cx, "settings_view", move |cx| {
        settings.read(cx).hook_state_json(cx)
    });
}
