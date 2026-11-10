//! The focus handles of Devices. Rows come and go with the paired devices, the found phones,
//! and the SSH hosts, so each handle is made on first use and found by a key such as
//! `speech.<id>.up`.

use gpui_kit::{App, FocusHandle};
use std::cell::RefCell;
use std::collections::HashMap;

/// The open sheet itself, which keeps Tab inside it. It is no tab stop of its own.
const TRAP: &str = "sheet.trap";
/// Done in a sheet, which has the focus while the sheet opens.
pub const DONE: &str = "sheet.done";
/// Add Device, where focus goes back when no other control opened the sheet.
pub const ADD: &str = "add";

#[derive(Default)]
pub struct Focus {
    handles: RefCell<HashMap<String, FocusHandle>>,
    /// The key of the control that opened the sheet.
    opener: RefCell<Option<String>>,
}

impl Focus {
    /// The handle of the control `key`.
    pub fn get(&self, key: &str, cx: &App) -> FocusHandle {
        self.handles
            .borrow_mut()
            .entry(key.to_owned())
            .or_insert_with(|| {
                let handle = cx.focus_handle();
                if key == TRAP {
                    handle
                } else {
                    handle.tab_stop(true)
                }
            })
            .clone()
    }

    pub fn trap(&self, cx: &App) -> FocusHandle {
        self.get(TRAP, cx)
    }

    pub fn opened_by(&self, key: &str) {
        *self.opener.borrow_mut() = Some(key.to_owned());
    }

    /// The control that opened the sheet, or Add Device once that control is gone.
    pub fn opener(&self, cx: &App) -> FocusHandle {
        let key = self.opener.borrow_mut().take();
        self.get(key.as_deref().unwrap_or(ADD), cx)
    }
}
