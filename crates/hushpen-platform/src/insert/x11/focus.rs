//! Which window gets the keys: the X input focus, with the `WM_CLASS` of its top-level window.

use super::Atoms;
use crate::insert::Target;
use std::error::Error;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, Window};
use x11rb::rust_connection::RustConnection;

/// `InputFocus` replies that name no window: `None` and `PointerRoot`.
const NO_FOCUS_WINDOW: Window = 1;
/// How far to climb from a focused child window to the top-level window that has a class.
const MAX_CLIMB: usize = 8;

pub(super) struct Focus {
    conn: RustConnection,
    root: Window,
    atoms: Atoms,
    pid: u32,
}

/// The words of a `WM_CLASS` value: instance, then class.
pub(super) fn class_words(value: &[u8]) -> Vec<String> {
    value
        .split(|byte| *byte == 0)
        .filter(|word| !word.is_empty())
        .map(|word| String::from_utf8_lossy(word).into_owned())
        .collect()
}

impl Focus {
    pub(super) fn connect() -> Result<Self, Box<dyn Error>> {
        let (conn, screen) = x11rb::connect(None)?;
        let root = conn.setup().roots[screen].root;
        let atoms = Atoms::new(&conn)?.reply()?;
        Ok(Self {
            conn,
            root,
            atoms,
            pid: std::process::id(),
        })
    }

    /// The focused app, or an empty target when nothing has focus or the server does not answer.
    pub(super) fn current(&self, own_app: &str) -> Target {
        self.read(own_app).unwrap_or_default()
    }

    fn read(&self, own_app: &str) -> Result<Target, Box<dyn Error>> {
        let Some(focused) = self.focused_window()? else {
            return Ok(Target::default());
        };
        let (top, words) = self.top_level(focused)?;
        let own_class = words.iter().any(|word| word.eq_ignore_ascii_case(own_app));
        let own = own_class || self.window_pid(top)? == Some(self.pid);
        Ok(Target {
            window: Some(u64::from(top)),
            label: words.last().cloned().unwrap_or_default(),
            classes: words,
            own,
        })
    }

    fn focused_window(&self) -> Result<Option<Window>, Box<dyn Error>> {
        let focus = self.conn.get_input_focus()?.reply()?.focus;
        if focus > NO_FOCUS_WINDOW {
            return Ok(Some(focus));
        }
        let active = self
            .conn
            .get_property(
                false,
                self.root,
                self.atoms._NET_ACTIVE_WINDOW,
                AtomEnum::WINDOW,
                0,
                1,
            )?
            .reply()?;
        Ok(active
            .value32()
            .and_then(|mut ids| ids.next())
            .filter(|id| *id != 0))
    }

    /// The window at or above `window` that has a `WM_CLASS`, with its words.
    fn top_level(&self, window: Window) -> Result<(Window, Vec<String>), Box<dyn Error>> {
        let mut current = window;
        for _ in 0..MAX_CLIMB {
            let class = self
                .conn
                .get_property(false, current, self.atoms.WM_CLASS, AtomEnum::STRING, 0, 64)?
                .reply()?;
            let words = class_words(&class.value);
            if !words.is_empty() {
                return Ok((current, words));
            }
            let parent = self.conn.query_tree(current)?.reply()?.parent;
            if parent == 0 || parent == self.root {
                break;
            }
            current = parent;
        }
        Ok((window, Vec::new()))
    }

    fn window_pid(&self, window: Window) -> Result<Option<u32>, Box<dyn Error>> {
        let reply = self
            .conn
            .get_property(
                false,
                window,
                self.atoms._NET_WM_PID,
                AtomEnum::CARDINAL,
                0,
                1,
            )?
            .reply()?;
        Ok(reply.value32().and_then(|mut pids| pids.next()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wm_class_is_instance_then_class() {
        assert_eq!(class_words(b"xterm\0XTerm\0"), ["xterm", "XTerm"]);
        assert_eq!(class_words(b"kitty\0kitty"), ["kitty", "kitty"]);
        assert!(class_words(b"").is_empty());
    }
}
