//! The global keys after a late Input Monitoring grant: they start without an app restart, and
//! a listener that keeps failing asks for a reopen. A fake permission source flips the grant;
//! nothing asks the system.

use super::keys::{KeysRestart, RETRIES};
use super::testkit::*;
use super::{Controller, KeysStatus};
use gpui_kit::{Entity, TestAppContext};
use hushpen_core::permission::{Access, Permissions, Preflight};
use hushpen_platform::keys::{Reason, Unavailable};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

struct Source(Mutex<Permissions>);

impl Source {
    fn input_monitoring(access: Access) -> Arc<Self> {
        Arc::new(Self(Mutex::new(Permissions {
            microphone: Access::Granted,
            accessibility: Access::Granted,
            input_monitoring: access,
        })))
    }

    fn set_input_monitoring(&self, access: Access) {
        self.0.lock().unwrap().input_monitoring = access;
    }
}

impl Preflight for Source {
    fn microphone(&self) -> Access {
        self.0.lock().unwrap().microphone
    }

    fn post_event(&self) -> Access {
        self.0.lock().unwrap().accessibility
    }

    fn listen_event(&self) -> Access {
        self.0.lock().unwrap().input_monitoring
    }
}

fn permission_off() -> KeysStatus {
    KeysStatus::Unavailable(Unavailable::new(
        Reason::Permission,
        "Input Monitoring is off.",
    ))
}

/// A restart that records its calls and answers `starts` (false: the tap cannot start).
fn restart(calls: &Rc<Cell<u32>>, starts: &Rc<Cell<bool>>) -> KeysRestart {
    let calls = Rc::clone(calls);
    let starts = Rc::clone(starts);
    Rc::new(move |_cx| {
        calls.set(calls.get() + 1);
        if starts.get() {
            Ok(Rc::new(|_: bool| {}) as Rc<dyn Fn(bool)>)
        } else {
            Err(Unavailable::new(
                Reason::Permission,
                "The tap did not start.",
            ))
        }
    })
}

struct Setup {
    rig: Rig,
    source: Arc<Source>,
    calls: Rc<Cell<u32>>,
    starts: Rc<Cell<bool>>,
}

impl Setup {
    fn controller(&self) -> &Entity<Controller> {
        &self.rig.controller
    }

    fn refresh(&self, cx: &mut TestAppContext) {
        self.controller()
            .update(cx, |controller, cx| controller.refresh_permissions(cx));
    }

    fn keys(&self, cx: &mut TestAppContext) -> serde_json::Value {
        self.controller().read_with(cx, |c, _| c.keys_json())
    }

    fn reopen(&self, cx: &mut TestAppContext) -> bool {
        self.controller()
            .read_with(cx, |c, _| c.keys_reopen_needed())
    }
}

fn setup(cx: &mut TestAppContext, status: KeysStatus, starts: bool) -> Setup {
    let rig = rig(cx, &["base"], "base");
    let source = Source::input_monitoring(Access::Denied);
    let calls = Rc::new(Cell::new(0));
    let starts = Rc::new(Cell::new(starts));
    let restart = restart(&calls, &starts);
    let attached = source.clone();
    rig.controller.update(cx, |controller, _| {
        controller.attach_permissions(attached);
        controller.attach_keys(status, Rc::new(|_| {}));
        controller.attach_keys_restart(restart);
    });
    Setup {
        rig,
        source,
        calls,
        starts,
    }
}

#[gpui_kit::test]
fn a_late_input_monitoring_grant_starts_the_keys_without_a_restart(cx: &mut TestAppContext) {
    let setup = setup(cx, permission_off(), true);
    assert_eq!(setup.keys(cx)["available"], false);

    setup.refresh(cx);
    assert_eq!(setup.calls.get(), 0, "no start while the grant is missing");

    setup.source.set_input_monitoring(Access::Granted);
    setup.refresh(cx);

    assert_eq!(setup.calls.get(), 1);
    assert_eq!(setup.keys(cx)["available"], true);
    assert!(!setup.reopen(cx));

    setup.refresh(cx);
    assert_eq!(
        setup.calls.get(),
        1,
        "a running listener is not started again"
    );
}

#[gpui_kit::test]
fn a_listener_that_keeps_failing_after_the_grant_asks_for_a_reopen(cx: &mut TestAppContext) {
    let setup = setup(cx, permission_off(), false);
    setup.source.set_input_monitoring(Access::Granted);

    for tries in 1..RETRIES {
        setup.refresh(cx);
        assert_eq!(setup.calls.get(), tries);
        assert!(!setup.reopen(cx), "{tries} failed starts are not enough");
    }
    setup.refresh(cx);

    assert!(setup.reopen(cx));
    assert_eq!(setup.keys(cx)["available"], false);
    assert_eq!(setup.keys(cx)["reopen"], true);
}

#[gpui_kit::test]
fn a_start_that_works_late_ends_the_reopen_request(cx: &mut TestAppContext) {
    let setup = setup(cx, permission_off(), false);
    setup.source.set_input_monitoring(Access::Granted);
    for _ in 0..RETRIES {
        setup.refresh(cx);
    }
    assert!(setup.reopen(cx));

    setup.starts.set(true);
    setup.refresh(cx);

    assert!(!setup.reopen(cx));
    assert_eq!(setup.keys(cx)["available"], true);
}

#[gpui_kit::test]
fn losing_the_grant_again_resets_the_failures(cx: &mut TestAppContext) {
    let setup = setup(cx, permission_off(), false);
    setup.source.set_input_monitoring(Access::Granted);
    for _ in 0..RETRIES {
        setup.refresh(cx);
    }
    assert!(setup.reopen(cx));

    setup.source.set_input_monitoring(Access::Denied);
    setup.refresh(cx);

    assert!(!setup.reopen(cx));
}

#[gpui_kit::test]
fn keys_that_are_off_for_another_reason_are_left_alone(cx: &mut TestAppContext) {
    let wayland = KeysStatus::Unavailable(Unavailable::wayland());
    let setup = setup(cx, wayland, true);
    setup.source.set_input_monitoring(Access::Granted);

    setup.refresh(cx);

    assert_eq!(setup.calls.get(), 0);
}

#[gpui_kit::test]
fn a_system_with_no_restart_attached_changes_nothing(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let source = Source::input_monitoring(Access::Granted);
    rig.controller.update(cx, |controller, _| {
        controller.attach_permissions(source);
        controller.attach_keys(permission_off(), Rc::new(|_| {}));
    });

    rig.controller
        .update(cx, |controller, cx| controller.refresh_permissions(cx));

    let json = rig.controller.read_with(cx, |c, _| c.keys_json());
    assert_eq!(json["available"], false);
}
