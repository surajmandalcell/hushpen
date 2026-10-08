//! X11 global keys.
//!
//! Both keys are read from XInput2 raw key events on the root window. They need no grab, so the
//! focused app still gets every key, and they include XTest input. Esc must also stay away from
//! the focused app while a session runs, so it is grabbed with `XGrabKey` when a session starts
//! and released when it ends. The grab only swallows the key: a window manager may already hold
//! Alt+Esc, which makes that one grab fail, so the cancel comes from the raw event, and only
//! while a session runs.

use super::{HoldKey, Reason, Sink, Unavailable};
use hushpen_core::dictation::AppEvent;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xinput::{self, ConnectionExt as _, KeyEventFlags, XIEventMask};
use x11rb::protocol::xproto::{ConnectionExt as _, GrabMode, ModMask, Window};
use x11rb::rust_connection::RustConnection;

/// X11 keycode of the Right Alt key on evdev keyboards (evdev 100 plus the X offset of 8).
const KEYCODE_RIGHT_ALT: u8 = 108;
const KEYSYM_ESCAPE: u32 = 0xff1b;
/// `XIAllMasterDevices`: every master keyboard and pointer, so no device is missed.
const ALL_MASTER_DEVICES: u16 = 1;

pub(super) struct X11Keys {
    conn: Arc<RustConnection>,
    root: Window,
    escape: u8,
    /// Whether a session runs, so the reader knows an Esc belongs to the pipeline.
    active: Arc<AtomicBool>,
    grabbed: Mutex<bool>,
}

fn unavailable(what: &str, error: impl std::fmt::Display) -> Unavailable {
    Unavailable::new(Reason::Failed, format!("{what}: {error}"))
}

fn keycode_of(conn: &RustConnection, keysym: u32) -> Option<u8> {
    let setup = conn.setup();
    let (min, max) = (setup.min_keycode, setup.max_keycode);
    let map = conn
        .get_keyboard_mapping(min, max - min + 1)
        .ok()?
        .reply()
        .ok()?;
    let per = usize::from(map.keysyms_per_keycode).max(1);
    map.keysyms
        .chunks(per)
        .position(|keysyms| keysyms.contains(&keysym))
        .and_then(|index| u8::try_from(index).ok())
        .and_then(|index| min.checked_add(index))
}

/// Esc with or without Alt held (the hold key is Alt), and with Caps Lock and Num Lock in any
/// state. `ModMask::ANY` is not used: it also asks for the combinations other clients hold.
fn escape_modifiers() -> Vec<ModMask> {
    let locks = [
        ModMask::from(0u16),
        ModMask::LOCK,
        ModMask::M2,
        ModMask::LOCK | ModMask::M2,
    ];
    [ModMask::from(0u16), ModMask::M1]
        .into_iter()
        .flat_map(|base| locks.iter().map(move |lock| base | *lock))
        .collect()
}

impl X11Keys {
    pub(super) fn start(hold: HoldKey, sink: Sink) -> Result<Self, Unavailable> {
        if hold != HoldKey::RightOption {
            return Err(Unavailable::new(
                Reason::Unsupported,
                "This hold key is not available on Linux. Use Right Alt.",
            ));
        }
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
            deviceid: ALL_MASTER_DEVICES,
            mask: vec![XIEventMask::RAW_KEY_PRESS | XIEventMask::RAW_KEY_RELEASE],
        };
        conn.xinput_xi_select_events(root, &[mask])
            .map_err(|e| unavailable("select raw key events", e))?
            .check()
            .map_err(|e| unavailable("select raw key events", e))?;
        let escape = keycode_of(&conn, KEYSYM_ESCAPE)
            .ok_or_else(|| Unavailable::new(Reason::Failed, "This keyboard has no Esc key."))?;

        let conn = Arc::new(conn);
        let active = Arc::new(AtomicBool::new(false));
        let reader = Arc::clone(&conn);
        let reading = Arc::clone(&active);
        thread::Builder::new()
            .name("hushpen-keys".into())
            .spawn(move || read_events(&reader, KEYCODE_RIGHT_ALT, escape, &reading, &sink))
            .map_err(|e| unavailable("start the key listener", e))?;
        Ok(Self {
            conn,
            root,
            escape,
            active,
            grabbed: Mutex::new(false),
        })
    }

    pub(super) fn set_session_active(&self, active: bool) {
        self.active.store(active, Ordering::SeqCst);
        let Ok(mut grabbed) = self.grabbed.lock() else {
            return;
        };
        if *grabbed == active {
            return;
        }
        let mut done = 0;
        for modifiers in escape_modifiers() {
            let result = if active {
                self.conn
                    .grab_key(
                        false,
                        self.root,
                        modifiers,
                        self.escape,
                        GrabMode::ASYNC,
                        GrabMode::ASYNC,
                    )
                    .map(|cookie| cookie.check())
            } else {
                self.conn
                    .ungrab_key(self.escape, self.root, modifiers)
                    .map(|cookie| cookie.check())
            };
            match result {
                Ok(Ok(())) => done += 1,
                Ok(Err(error)) => log::debug!("Esc grab change refused for {modifiers:?}: {error}"),
                Err(error) => log::warn!("Esc grab change failed: {error}"),
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

fn read_events(conn: &RustConnection, hold: u8, escape: u8, session: &AtomicBool, sink: &Sink) {
    let mut down = false;
    loop {
        let event = match conn.wait_for_event() {
            Ok(event) => event,
            Err(error) => {
                log::warn!("the key listener lost the X connection: {error}");
                return;
            }
        };
        match event {
            Event::XinputRawKeyPress(press) => {
                if press.flags.contains(KeyEventFlags::KEY_REPEAT) {
                    continue;
                }
                if press.detail == u32::from(hold) && !down {
                    down = true;
                    sink(AppEvent::HoldDown);
                } else if press.detail == u32::from(escape) && session.load(Ordering::SeqCst) {
                    sink(AppEvent::Esc);
                }
            }
            Event::XinputRawKeyRelease(release) if release.detail == u32::from(hold) && down => {
                down = false;
                sink(AppEvent::HoldUp);
            }
            _ => {}
        }
    }
}
