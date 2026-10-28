//! X11 global keys.
//!
//! Every key is read from XInput2 raw key events on the root window and handed to the shared
//! [`Hub`]. They need no grab and include XTest input. A grab only keeps a key from the focused
//! app: each live shortcut with a key that is not a modifier (Ctrl+Alt+V, Ctrl+Tab, F5) is
//! grabbed with `XGrabKey`, the whole keyboard is grabbed while the recorder is open, and Esc
//! is grabbed while a session runs. A window manager may already hold a chord such as Alt+Esc,
//! which makes that one grab fail, so what Hushpen does always comes from the raw event.
//!
//! Raw events reach us even when another client holds a chord with a passive grab. Whether a
//! chord is free is therefore asked once, when it is registered, by grabbing it and letting go.

use super::hub::Hub;
use super::keymap;
use super::{Reason, RecordSink, Sink, Unavailable};
use hushpen_core::shortcut::{
    Bindings, Combo, Conflict, HandsFree, Key, Modifier, Opener, Phys, Recording,
};
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
use std::thread;
use x11rb::connection::Connection;
use x11rb::errors::ReplyError;
use x11rb::protocol::xinput::{self, ConnectionExt as _, KeyEventFlags, XIEventMask};
use x11rb::protocol::xproto::{ConnectionExt as _, GrabMode, GrabStatus, ModMask, Window};
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
    /// The key code and modifier mask of each grab that keeps a live shortcut's key.
    chord_grabs: Mutex<BTreeSet<(u8, u16)>>,
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

fn modifier_mask(combo: &Combo) -> ModMask {
    combo
        .keys()
        .iter()
        .fold(ModMask::from(0u16), |mask, key| match key {
            Key::Any(modifier) | Key::Right(modifier) => mask | mask_of(*modifier),
            Key::Fn | Key::Char(_) | Key::Named(_) | Key::Code(_) => mask,
        })
}

fn phys_of(key: Key) -> Option<Phys> {
    match key {
        Key::Char(c) => Some(Phys::Char(c)),
        Key::Named(named) => Some(Phys::Named(named)),
        Key::Code(code) => Some(Phys::Code(code)),
        _ => None,
    }
}

/// Where this desktop keeps its shortcuts, and the program that opens them.
fn desktop_settings() -> (&'static str, Option<&'static [&'static str]>) {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let has = |part: &str| desktop.split(':').any(|name| name == part);
    if has("kde") {
        (
            "System Settings > Keyboard > Shortcuts",
            Some(&["systemsettings", "kcm_keys"]),
        )
    } else if has("gnome") || has("unity") || has("budgie") {
        (
            "Settings > Keyboard > Keyboard Shortcuts",
            Some(&["gnome-control-center", "keyboard"]),
        )
    } else if has("x-cinnamon") || has("cinnamon") {
        (
            "System Settings > Keyboard > Shortcuts",
            Some(&["cinnamon-settings", "keyboard"]),
        )
    } else if has("xfce") {
        (
            "Settings > Keyboard > Application Shortcuts, or Window Manager > Keyboard",
            Some(&["xfce4-keyboard-settings"]),
        )
    } else if has("mate") {
        (
            "Control Center > Keyboard Shortcuts",
            Some(&["mate-keybinding-properties"]),
        )
    } else if has("lxqt") {
        (
            "Preferences > Shortcut Keys",
            Some(&["lxqt-config-globalkeyshortcuts"]),
        )
    } else {
        ("your desktop's keyboard shortcut settings", None)
    }
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

/// The conflict of a chord another X client grabbed. X11 does not say which client.
fn grab_conflict() -> Conflict {
    let (place, program) = desktop_settings();
    let open = program
        .filter(|argv| on_path(argv[0]))
        .map(|argv| Opener::Command(argv.iter().map(|arg| (*arg).to_owned()).collect()));
    Conflict {
        message: "Another app or the desktop uses this shortcut.".to_owned(),
        steps: format!("Remove it in {place}. Then it works here."),
        open,
    }
}

/// The live shortcuts that have a key a grab can hold.
fn live_chords(bindings: &Bindings) -> impl Iterator<Item = &Combo> {
    let custom = bindings.hands_free.as_ref().and_then(HandsFree::combo);
    [
        bindings.hold.as_ref(),
        bindings.command.as_ref(),
        bindings.paste_last.as_ref(),
        custom,
    ]
    .into_iter()
    .flatten()
    // A grab holds one key with modifiers, so a chord of two keys only listens.
    .filter(|combo| combo.mains().count() == 1)
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
        hushpen_core::shortcut::set_code_names(keymap::code_name);
        let table: Table = Arc::new(Mutex::new(table));
        let conn = Arc::new(conn);
        // The engine closes the recorder itself when the keys come up or Esc goes down, so the
        // keyboard grab of the recorder ends here too.
        let record: RecordSink = {
            let conn = Arc::clone(&conn);
            Arc::new(move |recording: Recording| {
                if !matches!(recording, Recording::Progress(_)) {
                    let _ = conn.ungrab_keyboard(x11rb::CURRENT_TIME);
                    let _ = conn.flush();
                }
                record(recording);
            })
        };
        let hub = Arc::new(Hub::new(bindings.clone(), sink, record));
        let reader = Arc::clone(&conn);
        let reader_hub = Arc::clone(&hub);
        let reader_table = Arc::clone(&table);
        thread::Builder::new()
            .name("hushpen-keys".into())
            .spawn(move || read_events(&reader, &reader_hub, &reader_table))
            .map_err(|e| unavailable("start the key listener", e))?;
        let keys = Self {
            conn,
            root,
            hub,
            table,
            grabbed: Mutex::new(false),
            chord_grabs: Mutex::new(BTreeSet::new()),
        };
        keys.grab_chords(&bindings);
        Ok(keys)
    }

    pub(super) fn set_bindings(&self, bindings: Bindings) {
        self.grab_chords(&bindings);
        self.hub.set_bindings(bindings);
    }

    /// Grabs the key of each live shortcut and lets go of the ones no longer live. A chord
    /// another client holds cannot be grabbed; it still runs from the raw event.
    fn grab_chords(&self, bindings: &Bindings) {
        let wanted: BTreeSet<(u8, u16)> = live_chords(bindings)
            .filter_map(|combo| {
                let code = combo
                    .main()
                    .and_then(phys_of)
                    .and_then(|p| self.code_for(p))?;
                Some((code, modifier_mask(combo)))
            })
            .flat_map(|(code, base)| {
                lock_states()
                    .into_iter()
                    .map(move |lock| (code, u16::from(base | lock)))
            })
            .collect();
        let Ok(mut held) = self.chord_grabs.lock() else {
            return;
        };
        for (code, mask) in held.difference(&wanted) {
            let _ = self.conn.ungrab_key(*code, self.root, ModMask::from(*mask));
        }
        held.retain(|grab| wanted.contains(grab));
        for (code, mask) in wanted.difference(&held.clone()) {
            let result = self
                .conn
                .grab_key(
                    false,
                    self.root,
                    ModMask::from(*mask),
                    *code,
                    GrabMode::ASYNC,
                    GrabMode::ASYNC,
                )
                .map(|cookie| cookie.check());
            match result {
                Ok(Ok(())) => {
                    held.insert((*code, *mask));
                }
                Ok(Err(error)) => {
                    log::debug!("the grab of key {code} with {mask:#x} failed: {error}")
                }
                Err(error) => log::warn!("the grab of key {code} failed: {error}"),
            }
        }
        let _ = self.conn.flush();
    }

    /// The recorder takes the whole keyboard, so Ctrl+Tab or Alt+Tab is recorded and does not
    /// also move the focus or switch windows.
    pub(super) fn start_recording(&self) {
        self.hub.start_recording();
        let status = self
            .conn
            .grab_keyboard(
                false,
                self.root,
                x11rb::CURRENT_TIME,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
            )
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .map(|reply| reply.status);
        if status != Some(GrabStatus::SUCCESS) {
            log::debug!("the recorder could not grab the keyboard: {status:?}");
        }
    }

    pub(super) fn stop_recording(&self) {
        self.hub.stop_recording();
        let _ = self.conn.ungrab_keyboard(x11rb::CURRENT_TIME);
        let _ = self.conn.flush();
    }

    fn code_for(&self, phys: Phys) -> Option<u8> {
        let table = self.table.lock().ok()?;
        keymap::code_of(&table, phys)
    }

    /// Whether another client holds `combo` with a passive grab, and how to free it. A chord of
    /// modifiers alone has no key a client could grab, so it is always free.
    pub(super) fn conflict(&self, combo: &Combo) -> Option<Conflict> {
        self.grabbed_elsewhere(combo).then(grab_conflict)
    }

    fn grabbed_elsewhere(&self, combo: &Combo) -> bool {
        if combo.mains().count() != 1 {
            return false;
        }
        let Some(code) = combo
            .main()
            .and_then(phys_of)
            .and_then(|p| self.code_for(p))
        else {
            return false;
        };
        let base = modifier_mask(combo);
        let ours = self
            .chord_grabs
            .lock()
            .map(|held| held.clone())
            .unwrap_or_default();
        let mut taken = false;
        let mut probes = Vec::new();
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
                Ok(Ok(())) => probes.push(modifiers),
                Ok(Err(ReplyError::X11Error(error))) if error.error_kind == ErrorKind::Access => {
                    taken = true;
                }
                Ok(Err(error)) => log::warn!("the in-use check for {combo:?} failed: {error}"),
                Err(error) => log::warn!("the in-use check for {combo:?} failed: {error}"),
            }
        }
        // A probe of a live shortcut's own grab must not let that grab go.
        for modifiers in probes {
            if !ours.contains(&(code, u16::from(modifiers))) {
                let _ = self.conn.ungrab_key(code, self.root, modifiers);
            }
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
