//! Test hook for the tray: the `tray_menu` state section, the menu as it is built now. A test
//! clicks the real entry over D-Bus; the section says what the menu holds and whether the icon
//! was made. Compiled only with the `test-automation` feature.

use super::set_state_section;
use crate::controller::Controller;
use crate::tray;
use gpui_kit::{App, Entity};

pub fn attach(cx: &mut App, controller: Entity<Controller>, created: bool) {
    set_state_section(cx, "tray_menu", move |cx| {
        tray::menu_json(tray::listening(&controller, cx), created)
    });
}
