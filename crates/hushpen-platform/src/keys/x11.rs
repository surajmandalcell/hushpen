//! X11 global keys.
//!
//! Every key is read from XInput2 raw key events on the root window and handed to the shared
//! [`Hub`]. They need no grab, so the focused app still gets every key, and they include XTest
//! input. Esc must also stay away from the focused app while a session runs, so it is grabbed
//! with `XGrabKey` when a session starts and released when it ends. The grab only swallows the
//! key: a window manager may already hold Alt+Esc, which makes that one grab fail, so the
//! cancel comes from the raw event, and only while a session runs.
//!
//! Raw events reach us even when another client holds a chord with a passive grab. Whether a
//! chord is free is therefore asked once, when it is registered, by grabbing it and letting go.

use super::hub::Hub;
use super::keymap;
use super::{Reason, RecordSink, Sink, Unavailable};
use hushpen_core::shortcut::{Bindings, Combo, Key, Modifier, Phys};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use x11rb::connection::Connection;
use x11rb::errors::ReplyError;
use x11rb::protocol::xinput::{self, ConnectionExt as _, KeyEventFlags, XIEventMask};
use x11rb::protocol::xproto::{ConnectionExt as _, GrabMode, ModMask, Window};
use x11rb::protocol::{ErrorKind, Event};
use x11rb::rust_connection::RustConnection;

/// `XIAllDevices`, not `XIAllMasterDevices`: while another client's passive grab is active the
/// server stops sending the master's raw releases, so the keys would look held forever. Every
/// event then arrives twice (slave and master); the hub ignores the second one.
const ALL_DEVICES: u16 = 0;

type Table = Arc<Mutex<HashMap<u32, Phys>>>;

pub(super) struct X11Keys {
    conn: Arc<RustConnection>,
    root: Window,
    hub: Arc<Hub>,
    table: Table,
    /// Whether Esc is grabbed for a running session.
    grabbed: Mutex<bool>,
}

fn unavailable(what: &str, error: impl std::fmt::Display) -> Unavailable {
    Unavailable::new(Reason::Failed, format!("{what}: {error}"))
}

/// Caps Lock and Num Lock in any state, which the server counts as different modifiers for a
/// grab. `ModMask::ANY` is not used: it also asks for the combinations other clients hold.
fn lock_states() -> [ModMask; 4] {
    [
        ModMask::from(0u16),
        ModMask::LOCK,
        ModMask::M2,
        ModMask::LOCK | ModMask::M2,
    ]
}

fn mask_of(modifier: Modifier) -> ModMask {
    match modifier {
        Modifier::Ctrl => ModMask::CONTROL,
        Modifier::Alt => ModMask::M1,
        Modifier::Shift => ModMask::SHIFT,
        Modifier::Cmd => ModMask::M4,
    }
}

fn read_table(conn: &RustConnection) -> Option<HashMap<u32, Phys>> {
    let setup = conn.setup();
    let (min, max) = (setup.min_keycode, setup.max_keycode);
    let map = conn
        .get_keyboard_mapping(min, max - min + 1)
        .ok()?
        .reply()
        .ok()?;
    Some(keymap::build(
        min,
        usize::from(map.keysyms_per_keycode),
        &map.keysyms,
    ))
}

impl X11Keys {
    pub(super) fn start(
        bindings: Bindings,
        sink: Sink,
        record: RecordSink,
    ) -> Result<Self, Unavailable> {
        let (conn, screen) = x11rb::connect(None).map_err(|e| unavailable("x11 connect", e))?;
        let root = conn.setup().roots[screen].root;
        let version = conn
            .xinput_xi_query_version(2, 0)
            .map_err(|e| unavailable("XInput2", e))?
            .reply()
            .map_err(|e| unavailable("XInput2", e))?;
        if version.major_version < 2 {
            return Err(Unavailable::new(
                Reason::Unsupported,
                "This X server has no XInput2, so global keys are off.",
            ));
        }
        let mask = xinput::EventMask {
            deviceid: ALL_DEVICES,
            mask: vec![XIEventMask::RAW_KEY_PRESS | XIEventMask::RAW_KEY_RELEASE],
        };
        conn.xinput_xi_select_events(root, &[mask])
            .map_err(|e| unavailable("select raw key events", e))?
            .check()
            .map_err(|e| unavailable("select raw key events", e))?;
        let table = read_table(&conn).ok_or_else(|| {
            Unavailable::new(Reason::Failed, "The keyboard mapping is unreadable.")
        })?;
        let table: Table = Arc::new(Mutex::new(table));
        let hub = Arc::new(Hub::new(bindings, sink, record));
        let conn = Arc::new(conn);
        let reader = Arc::clone(&conn);
        let reader_hub = Arc::clone(&hub);
        let reader_table = Arc::clone(&table);
        thread::Builder::new()
            .name("hushpen-keys".into())
            .spawn(move || read_events(&reader, &reader_hub, &reader_table))
            .map_err(|e| unavailable("start the key listener", e))?;
        Ok(Self {
            conn,
            root,
            hub,
            table,
            grabbed: Mutex::new(false),
        })
    }

    pub(super) fn set_bindings(&self, bindings: Bindings) {
        self.hub.set_bindings(bindings);
    }

    pub(super) fn start_recording(&self) {
        self.hub.start_recording();
    }

    pub(super) fn stop_recording(&self) {
        self.hub.stop_recording();
    }

    fn code_for(&self, phys: Phys) -> Option<u8> {
        let table = self.table.lock().ok()?;
        keymap::code_of(&table, phys)
    }

    /// Whether another client holds `combo` with a passive grab. A chord with no letter has no
    /// key a client could grab, so it is always free.
    pub(super) fn in_use(&self, combo: &Combo) -> bool {
        let Some(code) = combo.letter().and_then(|c| self.code_for(Phys::Char(c))) else {
            return false;
        };
        let base = combo
            .keys()
            .iter()
            .fold(ModMask::from(0u16), |mask, key| match key {
                Key::Any(modifier) | Key::Right(modifier) => mask | mask_of(*modifier),
                Key::Fn | Key::Char(_) => mask,
            });
        let mut taken = false;
        let mut ours = Vec::new();
        for lock in lock_states() {
            let modifiers = base | lock;
            let result = self
                .conn
                .grab_key(
                    false,
                    self.root,
                    modifiers,
                    code,
                    GrabMode::ASYNC,
                    GrabMode::ASYNC,
                )
                .map(|cookie| cookie.check());
            match result {
                Ok(Ok(())) => ours.push(modifiers),
                Ok(Err(ReplyError::X11Error(error))) if error.error_kind == ErrorKind::Access => {
                    taken = true;
                }
                Ok(Err(error)) => log::warn!("the in-use check for {combo:?} failed: {error}"),
                Err(error) => log::warn!("the in-use check for {combo:?} failed: {error}"),
            }
        }
        for modifiers in ours {
            let _ = self.conn.ungrab_key(code, self.root, modifiers);
        }
        let _ = self.conn.flush();
        taken
    }

    pub(super) fn set_session_active(&self, active: bool) {
        self.hub.set_escape(active);
        let Ok(mut grabbed) = self.grabbed.lock() else {
            return;
        };
        if *grabbed == active {
            return;
        }
        let Some(escape) = self.code_for(Phys::Esc) else {
            return;
        };
        // Esc with or without Alt held (the hold key is Alt).
        let mut done = 0;
        for base in [ModMask::from(0u16), ModMask::M1] {
            for lock in lock_states() {
                let modifiers = base | lock;
                let result = if active {
                    self.conn
                        .grab_key(
                            false,
                            self.root,
                            modifiers,
                            escape,
                            GrabMode::ASYNC,
                            GrabMode::ASYNC,
                        )
                        .map(|cookie| cookie.check())
                } else {
                    self.conn
                        .ungrab_key(escape, self.root, modifiers)
                        .map(|cookie| cookie.check())
                };
                match result {
                    Ok(Ok(())) => done += 1,
                    Ok(Err(error)) => {
                        log::debug!("Esc grab change refused for {modifiers:?}: {error}");
                    }
                    Err(error) => log::warn!("Esc grab change failed: {error}"),
                }
            }
        }
        if done == 0 && active {
            log::warn!("Esc could not be grabbed, so it reaches the focused app during a session");
        }
        *grabbed = active;
        // The requests are queued on the connection; the server must see them now, not at the
        // next event.
        let _ = self.conn.flush();
    }
}

fn read_events(conn: &RustConnection, hub: &Hub, table: &Table) {
    loop {
        let event = match conn.wait_for_event() {
            Ok(event) => event,
            Err(error) => {
                log::warn!("the key listener lost the X connection: {error}");
                return;
            }
        };
        let phys_of = |detail: u32| table.lock().ok().and_then(|t| t.get(&detail).copied());
        match event {
            Event::XinputRawKeyPress(press) => {
                if press.flags.contains(KeyEventFlags::KEY_REPEAT) {
                    continue;
                }
                if let Some(phys) = phys_of(press.detail) {
                    hub.press(phys);
                }
            }
            Event::XinputRawKeyRelease(release) => {
                if let Some(phys) = phys_of(release.detail) {
                    hub.release(phys);
                }
            }
            Event::MappingNotify(_) => {
                if let (Some(fresh), Ok(mut table)) = (read_table(conn), table.lock()) {
                    *table = fresh;
                }
            }
            _ => {}
        }
    }
}
