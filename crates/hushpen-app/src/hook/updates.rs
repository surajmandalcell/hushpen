//! Test hook for the update check: the `updates` state section with the state, the offered
//! version, the error code, the settings it follows, and the last release notes open. Compiled
//! only with the `test-automation` feature.

use super::set_state_section;
use crate::updater::Updater;
use gpui_kit::{App, Entity};

pub fn attach(cx: &mut App, updater: Entity<Updater>) {
    set_state_section(cx, "updates", move |cx| updater.read(cx).hook_state_json());
}
