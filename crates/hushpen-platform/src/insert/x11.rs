//! X11 insertion: an XFixes-free selection owner for the clipboard, XTest for the paste chord.
//!
//! The clipboard is a window of our own that owns CLIPBOARD (or PRIMARY for xterm) while the
//! text is pasted. Each read of that text arrives as a `SelectionRequest`, which is the receipt
//! the restore waits for. The product never starts `xdotool` or `xclip`.

mod chord;
mod clipboard;
mod focus;

use super::{Inserter, OWN_APP_ID, Target, plan};
use crate::keys::{Reason, Unavailable};
use clipboard::X11Board;
use hushpen_core::insert::flow::{self, Request, Step};
use hushpen_core::insert::{Overrides, Report, choose_x11};
use std::sync::Mutex;
use x11rb::atom_manager;

atom_manager! {
    pub(super) Atoms: AtomsCookie {
        CLIPBOARD,
        PRIMARY,
        TARGETS,
        MULTIPLE,
        TIMESTAMP,
        SAVE_TARGETS,
        DELETE,
        INCR,
        ATOM_PAIR,
        UTF8_STRING,
        STRING,
        TEXT,
        TEXT_PLAIN: b"text/plain",
        TEXT_PLAIN_UTF8: b"text/plain;charset=utf-8",
        HUSHPEN_SELECTION,
        _NET_ACTIVE_WINDOW,
        _NET_WM_PID,
        WM_CLASS,
    }
}

pub(super) struct X11Inserter {
    focus: focus::Focus,
    board: Mutex<X11Board>,
}

impl X11Inserter {
    pub(super) fn start() -> Result<Self, Unavailable> {
        let failed = |what: &str, error: &dyn std::fmt::Display| {
            Unavailable::new(Reason::Failed, format!("{what}: {error}"))
        };
        Ok(Self {
            focus: focus::Focus::connect().map_err(|e| failed("x11 connect", &e))?,
            board: Mutex::new(X11Board::start().map_err(|e| failed("the clipboard", &e))?),
        })
    }
}

impl Inserter for X11Inserter {
    fn target(&self) -> Target {
        self.focus.current(OWN_APP_ID)
    }

    fn insert(
        &self,
        text: &str,
        target: &Target,
        overrides: &Overrides,
        observe: &mut dyn FnMut(Step<'_>),
    ) -> Report {
        let (method, copy_note) = plan(target, |target| choose_x11(&target.classes, overrides));
        let mut board = self
            .board
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        board.count_reads_of(target.window.and_then(|window| u32::try_from(window).ok()));
        let request = Request {
            text,
            target: &target.label,
            method,
            copy_note,
        };
        flow::run(&mut *board, &super::SystemClock, &request, observe)
    }
}
