//! Test hook for About: the `about` state section with the version, the
//! folders as `~/...`, and the open requests so a test sees what a click
//! opened. Compiled only with the `test-automation` feature.

use super::set_state_section;
use crate::about::About;
use gpui_kit::{App, Entity};

pub fn attach(cx: &mut App, about: Entity<About>) {
    set_state_section(cx, "about", move |cx| about.read(cx).hook_state_json());
}
