//! Shortcuts: the recorder rows the Shortcuts view draws. Without that view there is nothing to
//! show.

use super::Page;
use crate::shortcuts::panel;
use gpui_kit::{AnyElement, App};
use hushpen_core::shortcut::Slot;

pub fn keys_off(page: &Page, cx: &mut App) -> Option<AnyElement> {
    panel::keys_off_row(page.shortcuts.as_ref()?, cx)
}

pub fn slot(page: &Page, slot: Slot, cx: &mut App) -> Vec<AnyElement> {
    page.shortcuts
        .as_ref()
        .map_or_else(Vec::new, |shortcuts| panel::slot_rows(shortcuts, slot, cx))
}

pub fn reset(page: &Page, cx: &mut App) -> Vec<AnyElement> {
    page.shortcuts
        .as_ref()
        .map(|shortcuts| panel::reset_row(shortcuts, cx))
        .into_iter()
        .collect()
}
