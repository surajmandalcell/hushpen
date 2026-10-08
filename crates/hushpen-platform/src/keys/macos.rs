//! macOS global keys: a listen-only `CGEventTap` on its own thread and run loop.
//!
//! The tap only listens, so Esc and every other key still reach the focused app. A tap that the
//! system disabled is turned on again; a tap whose port died is replaced.

// `CFMachPortIsValid` is a plain C call on a port that this module owns.
#![allow(unsafe_code)]

use super::tap::{
    Decoder, HEALTH_INTERVAL, Health, ShortcutKeys, Tap, TapAction, TapEvent, TapGuard,
};
use super::{HoldKey, Reason, Shortcut, Sink, Unavailable};
use core_foundation::base::TCFType;
use core_foundation::mach_port::CFMachPortIsValid;
use core_foundation::runloop::{
    CFRunLoop, CFRunLoopSource, kCFRunLoopCommonModes, kCFRunLoopDefaultMode,
};
use core_graphics::event::{
    CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
    CallbackResult, EventField,
};
use objc2_core_graphics::CGPreflightListenEventAccess;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// How long one run loop slice lasts before the thread looks at the flags again.
const SLICE: Duration = Duration::from_millis(250);

pub(super) struct MacKeys;

struct InstalledTap {
    tap: CGEventTap<'static>,
    source: CFRunLoopSource,
}

impl Tap for InstalledTap {
    fn alive(&self) -> bool {
        // SAFETY: the port belongs to `self.tap`, which is alive for this call.
        unsafe { CFMachPortIsValid(self.tap.mach_port().as_concrete_TypeRef()) != 0 }
    }

    fn enable(&self) {
        self.tap.enable();
    }
}

impl Drop for InstalledTap {
    fn drop(&mut self) {
        CFRunLoop::get_current().remove_source(&self.source, unsafe { kCFRunLoopCommonModes });
    }
}

fn install(
    decoder: &Arc<Mutex<Decoder>>,
    sink: &Sink,
    reenable: &Arc<AtomicBool>,
) -> Option<InstalledTap> {
    let callback = {
        let decoder = Arc::clone(decoder);
        let sink = Arc::clone(sink);
        let reenable = Arc::clone(reenable);
        move |_proxy, kind: CGEventType, event: &core_graphics::event::CGEvent| {
            let tap_event = match kind {
                CGEventType::FlagsChanged => TapEvent::FlagsChanged {
                    keycode: event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE),
                    flags: event.get_flags().bits(),
                },
                CGEventType::KeyDown => TapEvent::KeyDown {
                    keycode: event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE),
                    flags: event.get_flags().bits(),
                },
                CGEventType::KeyUp => TapEvent::KeyUp {
                    keycode: event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE),
                    flags: event.get_flags().bits(),
                },
                CGEventType::TapDisabledByTimeout => TapEvent::DisabledByTimeout,
                CGEventType::TapDisabledByUserInput => TapEvent::DisabledByUserInput,
                _ => return CallbackResult::Keep,
            };
            let action = match decoder.lock() {
                Ok(mut decoder) => decoder.decode(tap_event),
                Err(_) => TapAction::Ignore,
            };
            match action {
                TapAction::Emit(app_event) => sink(app_event),
                TapAction::Reenable => reenable.store(true, Ordering::Release),
                TapAction::Ignore => {}
            }
            CallbackResult::Keep
        }
    };
    let tap = CGEventTap::new(
        CGEventTapLocation::Session,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::ListenOnly,
        vec![
            CGEventType::FlagsChanged,
            CGEventType::KeyDown,
            CGEventType::KeyUp,
        ],
        callback,
    )
    .ok()?;
    let source = tap.mach_port().create_runloop_source(0).ok()?;
    // SAFETY: a constant of the CoreFoundation framework.
    CFRunLoop::get_current().add_source(&source, unsafe { kCFRunLoopCommonModes });
    tap.enable();
    Some(InstalledTap { tap, source })
}

impl MacKeys {
    /// The tap only listens, so there is nothing to grab for a session.
    pub(super) fn set_session_active(&self, _active: bool) {}

    pub(super) fn start(
        hold: HoldKey,
        paste_last: Option<Shortcut>,
        sink: Sink,
    ) -> Result<Self, Unavailable> {
        // Creating a tap without Input Monitoring makes macOS ask. The ask belongs to
        // onboarding, after a click, so here the grant is only read.
        if !CGPreflightListenEventAccess() {
            return Err(Unavailable::new(
                Reason::Permission,
                "Input Monitoring is not allowed for Hushpen, so global keys are off.",
            ));
        }
        let shortcut = paste_last.and_then(|shortcut| {
            let keycode = crate::insert::key_code_for(shortcut.key)?;
            Some(ShortcutKeys::new(i64::from(keycode), shortcut))
        });
        let (ready, started) = mpsc::channel();
        thread::Builder::new()
            .name("hushpen-keys".into())
            .spawn(move || run(hold, shortcut, sink, ready))
            .map_err(|error| {
                Unavailable::new(
                    Reason::Failed,
                    format!("The key listener did not start: {error}"),
                )
            })?;
        match started.recv() {
            Ok(true) => Ok(Self),
            _ => Err(Unavailable::new(
                Reason::Permission,
                "macOS did not let Hushpen listen for keys. Check Input Monitoring.",
            )),
        }
    }
}

fn run(hold: HoldKey, shortcut: Option<ShortcutKeys>, sink: Sink, ready: mpsc::Sender<bool>) {
    let decoder = Arc::new(Mutex::new(Decoder::new(hold).with_shortcut(shortcut)));
    let reenable = Arc::new(AtomicBool::new(false));
    let Some(first) = install(&decoder, &sink, &reenable) else {
        let _ = ready.send(false);
        return;
    };
    let _ = ready.send(true);
    let mut guard = TapGuard::new(first, Instant::now());
    loop {
        // SAFETY: a constant of the CoreFoundation framework.
        CFRunLoop::run_in_mode(unsafe { kCFRunLoopDefaultMode }, SLICE, false);
        if reenable.swap(false, Ordering::AcqRel) {
            guard.reenable();
        }
        let health = guard.health_check(Instant::now(), || {
            let released = decoder.lock().ok().and_then(|mut decoder| decoder.reset());
            if let Some(event) = released {
                sink(event);
            }
            install(&decoder, &sink, &reenable)
        });
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
