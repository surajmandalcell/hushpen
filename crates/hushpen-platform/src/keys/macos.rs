//! macOS global keys: a listen-only `CGEventTap` on its own thread and run loop.
//!
//! The tap only listens, so Esc and every other key still reach the focused app. A tap that the
//! system disabled is turned on again; a tap whose port died is replaced.

// `CFMachPortIsValid` is a plain C call on a port that this module owns.
#![allow(unsafe_code)]

use super::hub::Hub;
use super::tap::{Decoder, HEALTH_INTERVAL, Health, Tap, TapAction, TapEvent, TapGuard};
use super::{Reason, RecordSink, Sink, Unavailable};
use core_foundation::base::TCFType;
use core_foundation::mach_port::CFMachPortIsValid;
use core_foundation::runloop::{
    CFRunLoop, CFRunLoopSource, kCFRunLoopCommonModes, kCFRunLoopDefaultMode,
};
use core_graphics::event::{
    CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
    CallbackResult, EventField,
};
use hushpen_core::shortcut::Bindings;
use objc2_core_graphics::CGPreflightListenEventAccess;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// How long one run loop slice lasts before the thread looks at the flags again.
const SLICE: Duration = Duration::from_millis(250);

const SPACE_KEYCODE: i64 = 49;

pub(super) struct MacKeys {
    hub: Arc<Hub>,
}

/// The letter, digit, and Space keys on the layout in use, by key code.
fn layout_chars() -> HashMap<i64, char> {
    let mut chars: HashMap<i64, char> = ('a'..='z')
        .chain('0'..='9')
        .filter_map(|c| Some((i64::from(crate::insert::key_code_for(c)?), c)))
        .collect();
    chars.insert(SPACE_KEYCODE, ' ');
    chars
}

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
    hub: &Arc<Hub>,
    reenable: &Arc<AtomicBool>,
) -> Option<InstalledTap> {
    let callback = {
        let decoder = Arc::clone(decoder);
        let hub = Arc::clone(hub);
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
                TapAction::Key(key, true) => hub.press(key),
                TapAction::Key(key, false) => hub.release(key),
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
    /// The tap only listens, so there is nothing to grab for a session; Esc is only sent on.
    pub(super) fn set_session_active(&self, active: bool) {
        self.hub.set_escape(active);
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

    pub(super) fn start(
        bindings: Bindings,
        sink: Sink,
        record: RecordSink,
    ) -> Result<Self, Unavailable> {
        // Creating a tap without Input Monitoring makes macOS ask. The ask belongs to
        // onboarding, after a click, so here the grant is only read.
        if !CGPreflightListenEventAccess() {
            return Err(Unavailable::new(
                Reason::Permission,
                "Input Monitoring is not allowed for Hushpen, so global keys are off.",
            ));
        }
        let hub = Arc::new(Hub::new(bindings, sink, record));
        let (ready, started) = mpsc::channel();
        let thread_hub = Arc::clone(&hub);
        thread::Builder::new()
            .name("hushpen-keys".into())
            .spawn(move || run(&thread_hub, ready))
            .map_err(|error| {
                Unavailable::new(
                    Reason::Failed,
                    format!("The key listener did not start: {error}"),
                )
            })?;
        match started.recv() {
            Ok(true) => Ok(Self { hub }),
            _ => Err(Unavailable::new(
                Reason::Permission,
                "macOS did not let Hushpen listen for keys. Check Input Monitoring.",
            )),
        }
    }
}

fn run(hub: &Arc<Hub>, ready: mpsc::Sender<bool>) {
    let decoder = Arc::new(Mutex::new(Decoder::new(layout_chars())));
    let reenable = Arc::new(AtomicBool::new(false));
    let Some(first) = install(&decoder, hub, &reenable) else {
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
            if let Ok(mut decoder) = decoder.lock() {
                decoder.reset();
            }
            hub.reset();
            install(&decoder, hub, &reenable)
        });
        if health.is_some()
            && let Ok(mut decoder) = decoder.lock()
        {
            decoder.set_chars(layout_chars());
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
