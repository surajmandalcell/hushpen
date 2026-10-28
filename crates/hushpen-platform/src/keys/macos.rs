//! macOS global keys: a `CGEventTap` on its own thread and run loop.
//!
//! The tap sees every key, the media keys (`NX_SYSDEFINED` events) included. With
//! Accessibility allowed it filters: the key that completes a live shortcut, and every key but a
//! modifier while the recorder is open, stop here, so Cmd+Tab or Play runs Hushpen and not the
//! app switcher or the music app. Every other key, Esc during a session included, still reaches
//! the focused app. With only Input Monitoring the tap listens, and the front app also gets the
//! shortcut's key. A listening tap is swapped for a filtering one once Accessibility is allowed.
//! A tap that the system disabled is turned on again; a tap whose port died is replaced.
//!
//! The app switcher runs before any tap, so while a live shortcut is its chord (Cmd+Tab) the
//! switcher is turned off for the session, and turned on again when the shortcut changes or the
//! app quits.

// The tap is created and read through plain C calls on pointers this module owns.
#![allow(unsafe_code)]

use super::hub::Hub;
use super::symbolic;
use super::tap::{
    Decoder, HEALTH_INTERVAL, Health, KEYCODE_SPACE, MEDIA_CODE, Tap, TapAction, TapEvent,
    TapGuard, named_keycode,
};
use super::{Reason, RecordSink, Sink, Unavailable};
use crate::keyboard_layout::{self, LayoutCache};
use core_foundation::base::TCFType;
use core_foundation::mach_port::{
    CFMachPort, CFMachPortInvalidate, CFMachPortIsValid, CFMachPortRef,
};
use core_foundation::runloop::{
    CFRunLoop, CFRunLoopSource, kCFRunLoopCommonModes, kCFRunLoopDefaultMode,
};
use hushpen_core::shortcut::{Bindings, Combo, Conflict, HandsFree, Key, Opener};
use std::collections::{BTreeSet, HashMap};
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// How long one run loop slice lasts before the thread looks at the flags again.
const SLICE: Duration = Duration::from_millis(250);

/// System Settings > Keyboard, where the Keyboard Shortcuts button is.
const KEYBOARD_SETTINGS: &str = "x-apple.systempreferences:com.apple.Keyboard-Settings.extension";

type EventRef = *mut c_void;
type TapCallback = unsafe extern "C" fn(
    proxy: *mut c_void,
    kind: u32,
    event: EventRef,
    info: *mut c_void,
) -> EventRef;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events: u64,
        callback: TapCallback,
        info: *mut c_void,
    ) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventGetIntegerValueField(event: EventRef, field: u32) -> i64;
    fn CGEventGetFlags(event: EventRef) -> u64;
}

/// `kCGSessionEventTap`, `kCGHeadInsertEventTap`, and the tap options.
const SESSION_TAP: u32 = 1;
const HEAD_INSERT: u32 = 0;
const OPTION_DEFAULT: u32 = 0;
const OPTION_LISTEN_ONLY: u32 = 1;
/// The event types: `kCGEventKeyDown` and so on, `NX_SYSDEFINED` for the media keys.
const KEY_DOWN: u32 = 10;
const KEY_UP: u32 = 11;
const FLAGS_CHANGED: u32 = 12;
const SYSTEM_DEFINED: u32 = 14;
const DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
const DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;
/// `kCGKeyboardEventKeycode`.
const FIELD_KEYCODE: u32 = 9;

pub(super) struct MacKeys {
    hub: Arc<Hub>,
    /// The app switcher hotkeys Hushpen turned off.
    switcher_off: Mutex<BTreeSet<i32>>,
}

/// The character keys and Space on the layout in use, by key code. The table is the copy
/// that the main thread keeps; this thread never asks the system.
fn layout_chars(layout: &LayoutCache) -> HashMap<i64, char> {
    layout.snapshot().1.chars(KEYCODE_SPACE)
}

/// The key map again, when the main thread published another layout since `seen`.
fn changed_chars(layout: &LayoutCache, seen: &mut u64) -> Option<HashMap<i64, char>> {
    let (generation, table) = layout.snapshot();
    if generation == *seen {
        return None;
    }
    *seen = generation;
    Some(table.chars(KEYCODE_SPACE))
}

/// What the tap callback reaches through its `info` pointer.
struct Context {
    decoder: Arc<Mutex<Decoder>>,
    hub: Arc<Hub>,
    reenable: Arc<AtomicBool>,
    filters: bool,
}

struct InstalledTap {
    port: CFMachPort,
    source: CFRunLoopSource,
    context: *mut Context,
    /// The tap may drop events; a listen-only tap may not.
    filters: bool,
}

impl Tap for InstalledTap {
    fn alive(&self) -> bool {
        // SAFETY: the port belongs to `self`, which is alive for this call.
        unsafe { CFMachPortIsValid(self.port.as_concrete_TypeRef()) != 0 }
    }

    fn enable(&self) {
        // SAFETY: as above.
        unsafe { CGEventTapEnable(self.port.as_concrete_TypeRef(), true) };
    }
}

impl Drop for InstalledTap {
    fn drop(&mut self) {
        CFRunLoop::get_current().remove_source(&self.source, unsafe { kCFRunLoopCommonModes });
        // SAFETY: the port is invalidated on this thread, the only one that runs its callback,
        // so no callback can still use the context that is freed after it.
        unsafe {
            CFMachPortInvalidate(self.port.as_concrete_TypeRef());
            drop(Box::from_raw(self.context));
        }
    }
}

/// The subtype and `data1` of a system defined event, which only AppKit reads.
fn media_fields(event: EventRef) -> Option<(i64, i64)> {
    objc2::rc::autoreleasepool(|_| {
        // SAFETY: `event` is the live event the tap was handed; the AppKit type is the same
        // CoreGraphics object.
        let event = unsafe { &*(event as *const objc2_core_graphics::CGEvent) };
        let ns = objc2_app_kit::NSEvent::eventWithCGEvent(event)?;
        Some((i64::from(ns.subtype().0), ns.data1() as i64))
    })
}

fn tap_event(kind: u32, event: EventRef) -> Option<TapEvent> {
    let fields = || {
        // SAFETY: a key event the tap was handed, alive for this callback.
        unsafe {
            (
                CGEventGetIntegerValueField(event, FIELD_KEYCODE),
                CGEventGetFlags(event),
            )
        }
    };
    if event.is_null() && !matches!(kind, DISABLED_BY_TIMEOUT | DISABLED_BY_USER_INPUT) {
        return None;
    }
    Some(match kind {
        FLAGS_CHANGED => {
            let (keycode, flags) = fields();
            TapEvent::FlagsChanged { keycode, flags }
        }
        KEY_DOWN => {
            let (keycode, flags) = fields();
            TapEvent::KeyDown { keycode, flags }
        }
        KEY_UP => {
            let (keycode, flags) = fields();
            TapEvent::KeyUp { keycode, flags }
        }
        SYSTEM_DEFINED => {
            let (subtype, data1) = media_fields(event)?;
            TapEvent::Media { subtype, data1 }
        }
        DISABLED_BY_TIMEOUT => TapEvent::DisabledByTimeout,
        DISABLED_BY_USER_INPUT => TapEvent::DisabledByUserInput,
        _ => return None,
    })
}

/// Whether the event stops here.
fn handle(context: &Context, kind: u32, event: EventRef) -> bool {
    let Some(tap_event) = tap_event(kind, event) else {
        return false;
    };
    let action = match context.decoder.lock() {
        Ok(mut decoder) => decoder.decode(tap_event),
        Err(_) => TapAction::Ignore,
    };
    let swallow = match action {
        TapAction::Key(key, true) => context.hub.press(key),
        TapAction::Key(key, false) => context.hub.release(key),
        TapAction::Tap(key) => {
            context.hub.press(key);
            context.hub.release(key);
            false
        }
        TapAction::Reenable => {
            context.reenable.store(true, Ordering::Release);
            false
        }
        TapAction::Ignore => false,
    };
    // A modifier is a flags event and is never swallowed, so the system's idea of which
    // modifiers are down stays right.
    context.filters && swallow && matches!(kind, KEY_DOWN | KEY_UP | SYSTEM_DEFINED)
}

unsafe extern "C" fn callback(
    _proxy: *mut c_void,
    kind: u32,
    event: EventRef,
    info: *mut c_void,
) -> EventRef {
    // SAFETY: `info` is the context of this tap, freed only after the tap is invalidated.
    let context = unsafe { &*(info as *const Context) };
    let drop = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        handle(context, kind, event)
    }))
    .unwrap_or(false);
    if drop { std::ptr::null_mut() } else { event }
}

/// A filtering tap when Accessibility is allowed, else a listening one. Creating a filtering tap
/// without the grant can make macOS ask, so it is only tried with the grant.
fn install_best(
    decoder: &Arc<Mutex<Decoder>>,
    hub: &Arc<Hub>,
    reenable: &Arc<AtomicBool>,
) -> Option<InstalledTap> {
    if crate::permissions::accessibility_trusted()
        && let Some(tap) = install(decoder, hub, reenable, true)
    {
        return Some(tap);
    }
    install(decoder, hub, reenable, false)
}

fn install(
    decoder: &Arc<Mutex<Decoder>>,
    hub: &Arc<Hub>,
    reenable: &Arc<AtomicBool>,
    filters: bool,
) -> Option<InstalledTap> {
    let context = Box::into_raw(Box::new(Context {
        decoder: Arc::clone(decoder),
        hub: Arc::clone(hub),
        reenable: Arc::clone(reenable),
        filters,
    }));
    let mask = [KEY_DOWN, KEY_UP, FLAGS_CHANGED, SYSTEM_DEFINED]
        .into_iter()
        .fold(0u64, |mask, kind| mask | (1 << kind));
    let options = if filters {
        OPTION_DEFAULT
    } else {
        OPTION_LISTEN_ONLY
    };
    // SAFETY: the callback matches `CGEventTapCallBack`, and `context` lives until the tap is
    // dropped.
    let raw = unsafe {
        CGEventTapCreate(
            SESSION_TAP,
            HEAD_INSERT,
            options,
            mask,
            callback,
            context.cast(),
        )
    };
    if raw.is_null() {
        // SAFETY: no tap holds the context.
        drop(unsafe { Box::from_raw(context) });
        return None;
    }
    // SAFETY: `CGEventTapCreate` returns an owned port (create rule).
    let port = unsafe { CFMachPort::wrap_under_create_rule(raw) };
    let Ok(source) = port.create_runloop_source(0) else {
        // SAFETY: the port is new and has no run loop source, so its callback never ran.
        unsafe {
            CFMachPortInvalidate(port.as_concrete_TypeRef());
            drop(Box::from_raw(context));
        }
        return None;
    };
    // SAFETY: a constant of the CoreFoundation framework.
    CFRunLoop::get_current().add_source(&source, unsafe { kCFRunLoopCommonModes });
    // SAFETY: the port is the live tap.
    unsafe { CGEventTapEnable(port.as_concrete_TypeRef(), true) };
    Some(InstalledTap {
        port,
        source,
        context,
        filters,
    })
}

/// The macOS key code of the first key of `combo` that is not a modifier.
fn main_keycode(combo: &Combo) -> Option<i64> {
    match combo.main()? {
        Key::Named(named) => named_keycode(named),
        Key::Char(' ') => Some(KEYCODE_SPACE),
        Key::Char(c) => keyboard_layout::shared().key_code_for(c).map(i64::from),
        Key::Code(code) if code < MEDIA_CODE => Some(i64::from(code)),
        _ => None,
    }
}

/// Where Hushpen notes the switcher hotkeys it turned off, so a start after a crash turns them
/// on again. The temporary folder lasts about as long as the login session that the window
/// server keeps the setting for.
fn switcher_note() -> PathBuf {
    std::env::temp_dir().join("hushpen-app-switcher-off")
}

fn live_combos(bindings: &Bindings) -> impl Iterator<Item = &Combo> {
    let custom = bindings.hands_free.as_ref().and_then(HandsFree::combo);
    [
        bindings.hold.as_ref(),
        bindings.command.as_ref(),
        bindings.paste_last.as_ref(),
        custom,
    ]
    .into_iter()
    .flatten()
}

impl MacKeys {
    /// The tap only listens, so there is nothing to grab for a session; Esc is only sent on.
    pub(super) fn set_session_active(&self, active: bool) {
        self.hub.set_escape(active);
    }

    pub(super) fn set_bindings(&self, bindings: Bindings) {
        self.follow_switcher(&bindings);
        self.hub.set_bindings(bindings);
    }

    /// Turns the app switcher off while a live shortcut is its chord, and on again after.
    fn follow_switcher(&self, bindings: &Bindings) {
        let wanted: BTreeSet<i32> = live_combos(bindings)
            .filter(|combo| combo.mains().count() == 1)
            .filter_map(|combo| Some((main_keycode(combo)?, symbolic::carbon_modifiers(combo))))
            .flat_map(|(code, modifiers)| symbolic::switcher_hotkeys(code, modifiers))
            .collect();
        let Ok(mut off) = self.switcher_off.lock() else {
            return;
        };
        for id in off.difference(&wanted).copied().collect::<Vec<_>>() {
            if symbolic::set_enabled(id, true) {
                off.remove(&id);
                log::info!("the app switcher hotkey {id} is on again");
            }
        }
        for id in wanted {
            if !off.contains(&id) && symbolic::is_enabled(id) && symbolic::set_enabled(id, false) {
                off.insert(id);
                log::info!("the app switcher hotkey {id} is off while Hushpen owns its chord");
            }
        }
        let note = switcher_note();
        let written = if off.is_empty() {
            std::fs::remove_file(&note).or_else(|error| match error.kind() {
                std::io::ErrorKind::NotFound => Ok(()),
                _ => Err(error),
            })
        } else {
            let ids: Vec<String> = off.iter().map(i32::to_string).collect();
            std::fs::write(&note, ids.join(" "))
        };
        if let Err(error) = written {
            log::warn!("the app switcher note was not written: {error}");
        }
    }

    /// Turns on what Hushpen turned off in the system. For the app's quit.
    pub(super) fn restore_system(&self) {
        self.follow_switcher(&Bindings::default());
    }

    pub(super) fn start_recording(&self) {
        self.hub.start_recording();
    }

    pub(super) fn stop_recording(&self) {
        self.hub.stop_recording();
    }

    /// The macOS system shortcut that is on and uses `combo`, with where to turn it off. Other
    /// apps cannot be asked: the system keeps no list of their shortcuts.
    pub(super) fn conflict(&self, combo: &Combo) -> Option<Conflict> {
        let code = main_keycode(combo)?;
        let modifiers = symbolic::carbon_modifiers(combo);
        if !symbolic::taken(&symbolic::system_hotkeys(), code, modifiers) {
            return None;
        }
        let (message, steps) = match symbolic::owner(code, modifiers) {
            Some((name, section)) => (
                format!("macOS uses this shortcut for \"{name}\"."),
                format!("Turn it off in Keyboard Shortcuts > {section}. Then it works here."),
            ),
            None => (
                "macOS uses this shortcut.".to_owned(),
                "Turn it off in Keyboard > Keyboard Shortcuts. Then it works here.".to_owned(),
            ),
        };
        Some(Conflict {
            message,
            steps,
            open: Some(Opener::Url(KEYBOARD_SETTINGS.to_owned())),
        })
    }

    pub(super) fn start(
        bindings: Bindings,
        sink: Sink,
        record: RecordSink,
    ) -> Result<Self, Unavailable> {
        // Creating a tap with neither Input Monitoring nor Accessibility can make macOS ask. The
        // ask belongs to onboarding, after a click, so here the grants are only read.
        if !crate::permissions::input_monitoring_granted() {
            return Err(Unavailable::new(
                Reason::Permission,
                "Input Monitoring is not allowed for Hushpen, so global keys are off.",
            ));
        }
        hushpen_core::shortcut::set_code_names(super::tap::code_name);
        // The layout is read here, on the main thread; the tap thread only reads the copy.
        keyboard_layout::follow_system();
        let hub = Arc::new(Hub::new(bindings.clone(), sink, record));
        let (ready, started) = mpsc::channel();
        let thread_hub = Arc::clone(&hub);
        let layout = Arc::clone(keyboard_layout::shared());
        thread::Builder::new()
            .name("hushpen-keys".into())
            .spawn(move || run(&thread_hub, &layout, ready))
            .map_err(|error| {
                Unavailable::new(
                    Reason::Failed,
                    format!("The key listener did not start: {error}"),
                )
            })?;
        match started.recv() {
            Ok(true) => {
                // A crash may have left the switcher off; this start owns it now.
                let left_off: BTreeSet<i32> = std::fs::read_to_string(switcher_note())
                    .unwrap_or_default()
                    .split_whitespace()
                    .filter_map(|id| id.parse().ok())
                    .filter(|id| symbolic::APP_SWITCHER.contains(id))
                    .collect();
                let keys = Self {
                    hub,
                    switcher_off: Mutex::new(left_off),
                };
                keys.follow_switcher(&bindings);
                Ok(keys)
            }
            _ => Err(Unavailable::new(
                Reason::Permission,
                "macOS did not let Hushpen listen for keys. Check Input Monitoring.",
            )),
        }
    }
}

fn run(hub: &Arc<Hub>, layout: &LayoutCache, ready: mpsc::Sender<bool>) {
    let (mut seen, _) = layout.snapshot();
    let decoder = Arc::new(Mutex::new(Decoder::new(layout_chars(layout))));
    let reenable = Arc::new(AtomicBool::new(false));
    let Some(first) = install_best(&decoder, hub, &reenable) else {
        let _ = ready.send(false);
        return;
    };
    let _ = ready.send(true);
    log::info!(
        "the key tap {}",
        if first.filters {
            "filters: a live shortcut's key stops at Hushpen"
        } else {
            "only listens: Accessibility is not allowed"
        }
    );
    let mut guard = TapGuard::new(first, Instant::now());
    loop {
        // SAFETY: a constant of the CoreFoundation framework.
        CFRunLoop::run_in_mode(unsafe { kCFRunLoopDefaultMode }, SLICE, false);
        if reenable.swap(false, Ordering::AcqRel) {
            guard.reenable();
        }
        let health = guard.health_check(Instant::now(), || {
            if let Ok(mut decoder) = decoder.lock() {
                decoder.reset();
            }
            hub.reset();
            install_best(&decoder, hub, &reenable)
        });
        if matches!(health, Some(Health::Fine))
            && !guard.tap().filters
            && crate::permissions::accessibility_trusted()
            && let Some(filtering) = install(&decoder, hub, &reenable, true)
        {
            log::info!("Accessibility is allowed now, so the key tap filters");
            guard.replace(filtering);
        }
        if let Some(chars) = changed_chars(layout, &mut seen)
            && let Ok(mut decoder) = decoder.lock()
        {
            decoder.set_chars(chars);
        }
        match health {
            Some(Health::Reinstalled) => log::warn!("the key tap was dead and was installed again"),
            Some(Health::Dead) => log::warn!(
                "the key tap is dead and could not be installed again; trying again in {} s",
                HEALTH_INTERVAL.as_secs()
            ),
            Some(Health::Fine) | None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard_layout::{Table, layouts};

    /// The tap thread is never the main thread, so a TIS call there would trip the debug
    /// assertion of the layout read. It gets and follows its key map from the shared table.
    #[test]
    fn the_tap_thread_gets_and_follows_its_keys_from_the_shared_table() {
        let layout = Arc::new(LayoutCache::new());
        layout.publish(Table::from_typed(layouts::qwerty));
        let thread_layout = Arc::clone(&layout);
        let (ready, published) = mpsc::channel::<()>();
        let (answer, answers) = mpsc::channel();
        let tap = thread::Builder::new()
            .name("hushpen-keys-test".into())
            .spawn(move || {
                assert!(!keyboard_layout::is_main_thread());
                let mut seen = thread_layout.snapshot().0;
                let first = layout_chars(&thread_layout);
                assert_eq!(changed_chars(&thread_layout, &mut seen), None);
                let _ = answer.send(first);
                published.recv().unwrap();
                let _ = answer.send(changed_chars(&thread_layout, &mut seen).unwrap_or_default());
                assert_eq!(changed_chars(&thread_layout, &mut seen), None);
            })
            .unwrap();
        let first = answers.recv().unwrap();
        assert_eq!(first.get(&9), Some(&'v'));
        assert_eq!(first.get(&KEYCODE_SPACE), Some(&' '));
        // The main thread saw a layout change and published the new table.
        layout.publish(Table::from_typed(layouts::dvorak));
        ready.send(()).unwrap();
        let second = answers.recv().unwrap();
        assert_eq!(second.get(&40), Some(&'v'));
        assert_eq!(second.get(&9), Some(&'.'));
        tap.join()
            .expect("the tap thread reads the table and calls no TIS function");
    }
}
