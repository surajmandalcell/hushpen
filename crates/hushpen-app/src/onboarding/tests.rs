//! Onboarding against the controller rig: the permission buttons, the order of the steps,
//! where a start resumes, the practice field, and the repair of a lost grant. No test opens a
//! window or asks the system for anything.

use super::*;
use crate::controller::testkit::{
    FakeInserter, Rig, attach_inserter, outcome_text, rig, say, send_at, settle,
};
use crate::mic::MicEvent;
use gpui_kit::{AppContext as _, TestAppContext};
use hushpen_audio::{CaptureEvent, InputDevice};
use hushpen_core::dictation::AppEvent;
use hushpen_core::insert::Chord;
use hushpen_core::onboarding::RowState;
use hushpen_core::permission::{Access, Preflight};
use std::cell::RefCell;
use std::sync::Mutex;

#[derive(Default)]
struct FakeGuide {
    requested: RefCell<Vec<Permission>>,
    opened: RefCell<Vec<String>>,
    refuse: std::cell::Cell<bool>,
    relaunched: std::cell::Cell<u32>,
    refuse_relaunch: std::cell::Cell<bool>,
    /// The permission source that a request changes, and what it turns into.
    answers: RefCell<Option<(Arc<FakeSystem>, Access)>>,
}

impl Guide for FakeGuide {
    fn request(&self, permission: Permission) {
        self.requested.borrow_mut().push(permission);
        if let Some((system, access)) = &*self.answers.borrow() {
            system.set(permission, *access);
        }
    }

    fn open(&self, url: &str) -> Result<(), String> {
        self.opened.borrow_mut().push(url.to_owned());
        if self.refuse.get() {
            Err("no System Settings".into())
        } else {
            Ok(())
        }
    }

    fn relaunch(&self) -> Result<(), String> {
        self.relaunched.set(self.relaunched.get() + 1);
        if self.refuse_relaunch.get() {
            Err("no bundle".into())
        } else {
            Ok(())
        }
    }
}

/// A permission provider the test changes while the app runs. It counts the reads.
struct FakeSystem {
    state: Mutex<[Access; 3]>,
    reads: std::sync::atomic::AtomicUsize,
}

impl FakeSystem {
    fn all(access: Access) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new([access; 3]),
            reads: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    fn set(&self, permission: Permission, access: Access) {
        let index = Permission::ALL
            .iter()
            .position(|each| *each == permission)
            .unwrap();
        self.state.lock().unwrap()[index] = access;
    }

    fn reads(&self) -> usize {
        self.reads.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn read(&self, index: usize) -> Access {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.state.lock().unwrap()[index]
    }
}

impl Preflight for FakeSystem {
    fn microphone(&self) -> Access {
        self.read(0)
    }

    fn post_event(&self) -> Access {
        self.read(1)
    }

    fn listen_event(&self) -> Access {
        self.read(2)
    }
}

struct Fixture {
    rig: Rig,
    onboarding: Entity<Onboarding>,
    guide: Rc<FakeGuide>,
    system: Arc<FakeSystem>,
}

impl Fixture {
    fn settings(&self) -> &hushpen_store::settings::SettingsStore {
        &self.rig.storage.settings
    }

    fn read<T>(&self, cx: &mut TestAppContext, read: impl FnOnce(&Onboarding, &App) -> T) -> T {
        self.onboarding.read_with(cx, |me, cx| read(me, cx))
    }

    fn act<T>(
        &self,
        cx: &mut TestAppContext,
        act: impl FnOnce(&mut Onboarding, &mut Context<Onboarding>) -> T,
    ) -> T {
        self.onboarding.update(cx, act)
    }

    fn step(&self, cx: &mut TestAppContext) -> Step {
        self.read(cx, |me, _| me.step())
    }

    fn mode(&self, cx: &mut TestAppContext) -> Mode {
        self.read(cx, |me, _| me.mode())
    }

    fn gate(&self, cx: &mut TestAppContext) -> KeyGate {
        self.rig.controller.read_with(cx, |c, _| c.gate())
    }
}

/// Starts onboarding on `rig`. `system` answers the permission reads; on Linux the controller
/// gets working keys and paste, as on an X11 session.
fn start(
    cx: &mut TestAppContext,
    rig: Rig,
    platform: Platform,
    system: Arc<FakeSystem>,
) -> Fixture {
    rig.controller.update(cx, |controller, _| {
        controller.attach_permissions(system.clone());
        controller.attach_keys(crate::controller::KeysStatus::Available, Rc::new(|_| {}));
    });
    if platform == Platform::Linux {
        attach_inserter(cx, &rig, &FakeInserter::pasted_into("gtk", Chord::CtrlV));
    }
    let guide = Rc::new(FakeGuide::default());
    let parts = Parts {
        storage: Rc::clone(&rig.storage),
        mic: rig.mic.clone(),
        models: rig.models.clone(),
        controller: rig.controller.clone(),
        guide: guide.clone(),
        platform,
        session: Session::X11,
    };
    let onboarding = cx.new(|cx| Onboarding::new(parts, cx));
    Fixture {
        rig,
        onboarding,
        guide,
        system,
    }
}

fn mac(cx: &mut TestAppContext, files: &[&str], chosen: &str) -> Fixture {
    let rig = rig(cx, files, chosen);
    let fixture = start(cx, rig, Platform::MacOs, FakeSystem::all(Access::Granted));
    list_microphones(cx, &fixture, vec![device()]);
    fixture
}

fn mac_with(cx: &mut TestAppContext, system: Arc<FakeSystem>) -> Fixture {
    let rig = rig(cx, &["base"], "base");
    let fixture = start(cx, rig, Platform::MacOs, system);
    list_microphones(cx, &fixture, vec![device()]);
    fixture
}

fn device() -> InputDevice {
    InputDevice {
        id: "default".into(),
        name: "Built-in microphone".into(),
        is_default: true,
    }
}

/// What the system answers to the list of microphones. The real refresh reads on a thread,
/// which a test scheduler does not allow.
fn list_microphones(cx: &mut TestAppContext, fixture: &Fixture, devices: Vec<InputDevice>) {
    fixture.rig.mic.update(cx, |mic, cx| {
        mic.handle(MicEvent::Devices(Ok(devices)), cx);
        // The timers of the page and of Mic would start a reading thread.
        mic.hold_listing();
    });
    cx.run_until_parked();
}

fn go_to(cx: &mut TestAppContext, fixture: &Fixture, step: Step) {
    while fixture.step(cx) != step {
        fixture
            .act(cx, |me, cx| me.advance(cx))
            .expect("the step can pass");
    }
}

fn level(cx: &mut TestAppContext, fixture: &Fixture, level: f32) {
    let sink = fixture
        .rig
        .mic_backend
        .sink
        .lock()
        .unwrap()
        .clone()
        .unwrap();
    sink(CaptureEvent::Level(level));
    cx.run_until_parked();
}

fn hold(cx: &mut TestAppContext, fixture: &Fixture, from: u64) -> Result<(), String> {
    send_at(cx, &fixture.rig, from, AppEvent::HoldDown)?;
    send_at(cx, &fixture.rig, from + 2_000, AppEvent::HoldUp)?;
    settle(cx, &fixture.rig);
    Ok(())
}

fn row(cx: &mut TestAppContext, fixture: &Fixture, key: &str) -> RowState {
    fixture
        .read(cx, |me, cx| me.rows(cx))
        .into_iter()
        .find(|row| row.key.key() == key)
        .unwrap_or_else(|| panic!("no {key} row"))
        .state
}

#[gpui_kit::test]
fn an_empty_data_folder_opens_setup_at_permissions_with_the_keys_closed(cx: &mut TestAppContext) {
    let fixture = mac(cx, &["base"], "base");

    assert_eq!(fixture.mode(cx), Mode::Setup);
    assert_eq!(fixture.step(cx), Step::Permissions);
    assert_eq!(fixture.gate(cx), KeyGate::Closed);
    assert!(send_at(cx, &fixture.rig, 1_000, AppEvent::HoldDown).is_err());
    assert_eq!(
        fixture
            .rig
            .mic_backend
            .starts
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
}

#[gpui_kit::test]
fn a_finished_onboarding_does_not_open(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    rig.storage
        .settings
        .set_internal(COMPLETED, json!(true))
        .unwrap();
    let fixture = start(cx, rig, Platform::MacOs, FakeSystem::all(Access::Granted));

    assert_eq!(fixture.mode(cx), Mode::Hidden);
    assert!(!fixture.read(cx, |me, _| me.active()));
    assert_eq!(fixture.gate(cx), KeyGate::Open);
}

#[gpui_kit::test]
fn each_mac_button_opens_its_own_privacy_pane(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let fixture = start(
        cx,
        rig,
        Platform::MacOs,
        FakeSystem::all(Access::NotDetermined),
    );
    let urls = [
        (Permission::Microphone, "Privacy_Microphone"),
        (Permission::Accessibility, "Privacy_Accessibility"),
        (Permission::InputMonitoring, "Privacy_ListenEvent"),
    ];

    for (permission, pane) in urls {
        fixture.act(cx, |me, cx| me.open_settings(permission, cx));
        let opened = fixture.guide.opened.borrow().last().cloned().unwrap();
        assert!(opened.starts_with("x-apple.systempreferences:"), "{opened}");
        assert!(opened.ends_with(pane), "{permission:?} opened {opened}");
    }
    assert_eq!(fixture.guide.opened.borrow().len(), 3);
    assert!(
        fixture.guide.requested.borrow().is_empty(),
        "Open Settings calls no request API"
    );
}

#[gpui_kit::test]
fn allow_calls_the_request_once_on_every_click_in_every_state(cx: &mut TestAppContext) {
    let fixture = mac_with(cx, FakeSystem::all(Access::NotDetermined));

    for access in [Access::NotDetermined, Access::Denied, Access::Granted] {
        for permission in Permission::ALL {
            fixture.system.set(permission, access);
            fixture.act(cx, |me, cx| me.refresh(cx));
            let before = fixture.guide.requested.borrow().len();

            fixture.act(cx, |me, cx| me.allow(permission, cx));

            let requested = fixture.guide.requested.borrow();
            assert_eq!(
                requested.len(),
                before + 1,
                "{permission:?} when {access:?}"
            );
            assert_eq!(requested.last(), Some(&permission));
        }
    }
    assert_eq!(
        *fixture.guide.opened.borrow(),
        [Permission::InputMonitoring.settings_url()],
        "only Input Monitoring that stayed denied after its request opens a pane"
    );
}

fn im_url() -> &'static str {
    Permission::InputMonitoring.settings_url()
}

fn ax_url() -> &'static str {
    Permission::Accessibility.settings_url()
}

fn hint(
    cx: &mut TestAppContext,
    fixture: &Fixture,
    permission: Permission,
) -> Option<&'static str> {
    fixture.read(cx, |me, _| me.stale_hint(permission))
}

#[gpui_kit::test]
fn allow_on_input_monitoring_that_stays_denied_opens_its_pane_and_shows_the_remove_line(
    cx: &mut TestAppContext,
) {
    let fixture = mac_with(cx, FakeSystem::all(Access::Denied));

    fixture.act(cx, |me, cx| me.allow(Permission::InputMonitoring, cx));

    assert_eq!(
        *fixture.guide.requested.borrow(),
        [Permission::InputMonitoring],
        "the request API is still called once"
    );
    assert_eq!(
        *fixture.guide.opened.borrow(),
        [im_url()],
        "one pane, opened once"
    );
    assert_eq!(
        hint(cx, &fixture, Permission::InputMonitoring),
        Some(
            "If Hushpen is already in the list, remove it with the minus button, then click Allow again."
        )
    );
    assert_eq!(hint(cx, &fixture, Permission::Accessibility), None);
    assert_eq!(hint(cx, &fixture, Permission::Microphone), None);
}

#[gpui_kit::test]
fn allow_on_input_monitoring_that_turns_granted_opens_no_pane_and_the_row_is_ready(
    cx: &mut TestAppContext,
) {
    let fixture = mac_with(cx, FakeSystem::all(Access::Denied));
    *fixture.guide.answers.borrow_mut() = Some((fixture.system.clone(), Access::Granted));

    fixture.act(cx, |me, cx| me.allow(Permission::InputMonitoring, cx));

    assert!(fixture.guide.opened.borrow().is_empty());
    assert_eq!(hint(cx, &fixture, Permission::InputMonitoring), None);
    assert_eq!(row(cx, &fixture, "inputMonitoring"), RowState::Ready);
}

#[gpui_kit::test]
fn allow_on_input_monitoring_that_waits_for_the_prompt_opens_no_pane(cx: &mut TestAppContext) {
    let fixture = mac_with(cx, FakeSystem::all(Access::NotDetermined));

    fixture.act(cx, |me, cx| me.allow(Permission::InputMonitoring, cx));

    assert!(fixture.guide.opened.borrow().is_empty());
    assert_eq!(hint(cx, &fixture, Permission::InputMonitoring), None);
}

#[gpui_kit::test]
fn the_remove_line_goes_when_the_grant_arrives(cx: &mut TestAppContext) {
    let fixture = mac_with(cx, FakeSystem::all(Access::Denied));
    fixture.act(cx, |me, cx| me.allow(Permission::InputMonitoring, cx));
    assert!(hint(cx, &fixture, Permission::InputMonitoring).is_some());

    fixture
        .system
        .set(Permission::InputMonitoring, Access::Granted);
    fixture.act(cx, |me, cx| me.app_activated(cx));

    assert_eq!(hint(cx, &fixture, Permission::InputMonitoring), None);
}

#[gpui_kit::test]
fn accessibility_that_stays_false_after_the_wait_and_a_return_opens_its_pane_and_shows_the_line(
    cx: &mut TestAppContext,
) {
    let fixture = mac_with(cx, FakeSystem::all(Access::Denied));

    fixture.act(cx, |me, cx| me.allow(Permission::Accessibility, cx));
    assert!(
        fixture.guide.opened.borrow().is_empty(),
        "the alert of macOS gets its time first"
    );
    fixture.act(cx, |me, cx| me.app_activated(cx));
    assert!(
        fixture.guide.opened.borrow().is_empty(),
        "a return before the wait is over opens nothing"
    );
    cx.executor().advance_clock(Duration::from_millis(2_100));
    cx.run_until_parked();

    assert_eq!(*fixture.guide.opened.borrow(), [ax_url()]);
    assert_eq!(
        *fixture.guide.requested.borrow(),
        [Permission::Accessibility]
    );
    assert!(hint(cx, &fixture, Permission::Accessibility).is_some());
    assert_eq!(hint(cx, &fixture, Permission::InputMonitoring), None);

    cx.executor().advance_clock(Duration::from_millis(3_000));
    fixture.act(cx, |me, cx| me.app_activated(cx));
    assert_eq!(
        fixture.guide.opened.borrow().len(),
        1,
        "one Allow click opens the pane once"
    );
}

#[gpui_kit::test]
fn accessibility_opens_its_pane_when_the_return_comes_after_the_wait(cx: &mut TestAppContext) {
    let fixture = mac_with(cx, FakeSystem::all(Access::Denied));

    fixture.act(cx, |me, cx| me.allow(Permission::Accessibility, cx));
    cx.executor().advance_clock(Duration::from_millis(2_100));
    cx.run_until_parked();
    assert!(
        fixture.guide.opened.borrow().is_empty(),
        "the user has not come back yet"
    );
    fixture.act(cx, |me, cx| me.app_activated(cx));

    assert_eq!(*fixture.guide.opened.borrow(), [ax_url()]);
    assert!(hint(cx, &fixture, Permission::Accessibility).is_some());
}

#[gpui_kit::test]
fn accessibility_that_turns_trusted_opens_no_pane(cx: &mut TestAppContext) {
    let fixture = mac_with(cx, FakeSystem::all(Access::Denied));

    fixture.act(cx, |me, cx| me.allow(Permission::Accessibility, cx));
    fixture
        .system
        .set(Permission::Accessibility, Access::Granted);
    cx.executor().advance_clock(Duration::from_millis(2_100));
    cx.run_until_parked();
    fixture.act(cx, |me, cx| me.app_activated(cx));

    assert!(fixture.guide.opened.borrow().is_empty());
    assert_eq!(hint(cx, &fixture, Permission::Accessibility), None);
    assert_eq!(row(cx, &fixture, "accessibility"), RowState::Ready);
}

#[gpui_kit::test]
fn a_return_without_an_accessibility_click_opens_nothing(cx: &mut TestAppContext) {
    let fixture = mac_with(cx, FakeSystem::all(Access::Denied));

    cx.executor().advance_clock(Duration::from_millis(5_000));
    fixture.act(cx, |me, cx| me.app_activated(cx));

    assert!(fixture.guide.opened.borrow().is_empty());
    assert_eq!(hint(cx, &fixture, Permission::Accessibility), None);
}

#[gpui_kit::test]
fn the_state_shows_the_remove_line_on_its_row(cx: &mut TestAppContext) {
    let fixture = mac_with(cx, FakeSystem::all(Access::Denied));
    fixture.act(cx, |me, cx| me.allow(Permission::InputMonitoring, cx));

    let state = fixture.read(cx, |me, cx| me.state_json(cx));

    let rows = state["permissions"]["rows"].as_array().unwrap();
    let hints: Vec<_> = rows
        .iter()
        .map(|row| (row["key"].as_str().unwrap(), row["hint"].as_str()))
        .collect();
    assert!(
        hints.contains(&("inputMonitoring", Some(STALE_HINT))),
        "{hints:?}"
    );
    assert!(hints.contains(&("accessibility", None)), "{hints:?}");
}

#[gpui_kit::test]
fn a_granted_then_lost_permission_is_requested_again_by_allow(cx: &mut TestAppContext) {
    let fixture = mac_with(cx, FakeSystem::all(Access::Granted));
    fixture.act(cx, |me, cx| me.refresh(cx));
    fixture
        .system
        .set(Permission::Accessibility, Access::Denied);
    fixture.act(cx, |me, cx| me.refresh(cx));
    assert_eq!(row(cx, &fixture, "accessibility"), RowState::Missing);

    fixture.act(cx, |me, cx| me.allow(Permission::Accessibility, cx));

    assert_eq!(
        *fixture.guide.requested.borrow(),
        [Permission::Accessibility]
    );
}

#[gpui_kit::test]
fn each_mac_row_offers_allow_then_open_settings_on_one_line(cx: &mut TestAppContext) {
    let fixture = mac_with(cx, FakeSystem::all(Access::Denied));

    let rows = fixture.read(cx, |me, cx| me.rows(cx));

    let opens: Vec<_> = rows.iter().map(|row| row.opens).collect();
    assert_eq!(opens, Permission::ALL.map(Some));
    assert!(rows.iter().all(|row| !row.relaunch));
}

#[gpui_kit::test]
fn a_refused_settings_opener_shows_a_notice_and_nothing_else(cx: &mut TestAppContext) {
    let fixture = mac_with(cx, FakeSystem::all(Access::Denied));
    fixture.guide.refuse.set(true);

    fixture.act(cx, |me, cx| me.open_settings(Permission::Accessibility, cx));

    assert!(fixture.read(cx, |me, _| me.notice().is_some()));
    assert_eq!(fixture.step(cx), Step::Permissions);
}

#[gpui_kit::test]
fn the_page_reads_again_every_second_and_shows_a_grant_within_one_tick(cx: &mut TestAppContext) {
    for flip in Permission::ALL {
        let fixture = mac_with(cx, FakeSystem::all(Access::Denied));
        let key = match flip {
            Permission::Microphone => "microphone",
            Permission::Accessibility => "accessibility",
            Permission::InputMonitoring => "inputMonitoring",
        };
        assert_eq!(row(cx, &fixture, key), RowState::Missing);

        fixture.system.set(flip, Access::Granted);
        cx.executor().advance_clock(Duration::from_millis(1_100));
        cx.run_until_parked();

        assert_eq!(row(cx, &fixture, key), RowState::Ready, "{flip:?}");
    }
}

#[gpui_kit::test]
fn the_repair_page_reads_again_every_second_too(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    rig.storage
        .settings
        .set_internal(COMPLETED, json!(true))
        .unwrap();
    rig.storage
        .settings
        .set_internal(
            "permissions.lastGranted",
            json!({"microphone": true, "accessibility": true, "inputMonitoring": true}),
        )
        .unwrap();
    let system = FakeSystem::all(Access::Granted);
    system.set(Permission::Accessibility, Access::Denied);
    let fixture = start(cx, rig, Platform::MacOs, system);
    list_microphones(cx, &fixture, vec![device()]);
    assert_eq!(fixture.mode(cx), Mode::Repair);
    assert_eq!(row(cx, &fixture, "accessibility"), RowState::Missing);

    fixture
        .system
        .set(Permission::Accessibility, Access::Granted);
    cx.executor().advance_clock(Duration::from_millis(1_100));
    cx.run_until_parked();

    assert_eq!(row(cx, &fixture, "accessibility"), RowState::Ready);
}

#[gpui_kit::test]
fn coming_back_to_the_app_reads_the_permissions_at_once(cx: &mut TestAppContext) {
    let fixture = mac_with(cx, FakeSystem::all(Access::Denied));
    fixture
        .system
        .set(Permission::InputMonitoring, Access::Granted);
    assert_eq!(row(cx, &fixture, "inputMonitoring"), RowState::Missing);

    fixture.act(cx, |me, cx| me.app_activated(cx));

    assert_eq!(row(cx, &fixture, "inputMonitoring"), RowState::Ready);
}

#[gpui_kit::test]
fn the_reads_stop_when_the_permissions_page_is_not_shown(cx: &mut TestAppContext) {
    let fixture = mac(cx, &["base"], "base");
    cx.executor().advance_clock(Duration::from_millis(1_100));
    cx.run_until_parked();
    assert!(fixture.system.reads() > 0, "the page reads while it shows");
    go_to(cx, &fixture, Step::MicTest);
    let reads = fixture.system.reads();

    cx.executor().advance_clock(Duration::from_millis(3_000));
    cx.run_until_parked();
    fixture.act(cx, |me, cx| me.app_activated(cx));

    assert!(!fixture.read(cx, |me, _| me.permissions_page_shown()));
    assert_eq!(
        fixture.system.reads(),
        reads,
        "no tick and no activation reads on another step"
    );
}

#[gpui_kit::test]
fn quit_and_reopen_shows_for_granted_keys_that_cannot_start_and_relaunches(
    cx: &mut TestAppContext,
) {
    let fixture = mac_with(cx, FakeSystem::all(Access::Granted));
    fixture.rig.controller.update(cx, |controller, _| {
        controller.attach_keys(
            crate::controller::KeysStatus::Unavailable(hushpen_platform::keys::Unavailable::new(
                hushpen_platform::keys::Reason::Permission,
                "The tap did not start.",
            )),
            Rc::new(|_| {}),
        );
        controller.attach_keys_restart(Rc::new(|_| {
            Err(hushpen_platform::keys::Unavailable::new(
                hushpen_platform::keys::Reason::Permission,
                "The tap did not start.",
            ))
        }));
    });
    assert!(fixture.read(cx, |me, cx| me.rows(cx).iter().all(|row| !row.relaunch)));

    for _ in 0..crate::controller::KEYS_RETRIES {
        fixture.act(cx, |me, cx| me.refresh(cx));
    }

    let rows = fixture.read(cx, |me, cx| me.rows(cx));
    let reopen = rows.iter().find(|row| row.relaunch).unwrap();
    assert_eq!(reopen.key.key(), "inputMonitoring");
    assert_eq!(reopen.state, RowState::Missing);
    assert!(!fixture.read(cx, |me, cx| me.can_continue(cx)));

    fixture.act(cx, |me, cx| me.relaunch(cx));

    assert_eq!(fixture.guide.relaunched.get(), 1);
}

#[gpui_kit::test]
fn a_relaunch_that_cannot_start_shows_a_notice_and_the_app_stays(cx: &mut TestAppContext) {
    let fixture = mac_with(cx, FakeSystem::all(Access::Granted));
    fixture.guide.refuse_relaunch.set(true);

    fixture.act(cx, |me, cx| me.relaunch(cx));

    assert_eq!(fixture.guide.relaunched.get(), 1);
    assert!(fixture.read(cx, |me, _| me.notice().is_some()));
}

#[gpui_kit::test]
fn the_mac_rows_follow_the_system_without_a_restart(cx: &mut TestAppContext) {
    let fixture = mac_with(cx, FakeSystem::all(Access::Denied));
    for key in ["microphone", "accessibility", "inputMonitoring"] {
        assert_eq!(row(cx, &fixture, key), RowState::Missing, "{key}");
    }
    assert!(!fixture.read(cx, |me, cx| me.can_continue(cx)));

    fixture
        .system
        .set(Permission::Accessibility, Access::Granted);
    fixture.act(cx, |me, cx| me.refresh(cx));
    assert_eq!(row(cx, &fixture, "accessibility"), RowState::Ready);
    assert_eq!(row(cx, &fixture, "microphone"), RowState::Missing);

    for permission in Permission::ALL {
        fixture.system.set(permission, Access::Granted);
    }
    fixture.act(cx, |me, cx| me.refresh(cx));
    assert!(fixture.read(cx, |me, cx| me.can_continue(cx)));

    fixture
        .system
        .set(Permission::InputMonitoring, Access::Denied);
    fixture.act(cx, |me, cx| me.refresh(cx));
    assert_eq!(row(cx, &fixture, "inputMonitoring"), RowState::Missing);
    assert!(!fixture.read(cx, |me, cx| me.can_continue(cx)));
}

#[gpui_kit::test]
fn the_page_has_five_steps_in_order_and_no_step_can_be_skipped(cx: &mut TestAppContext) {
    let fixture = mac_with(cx, FakeSystem::all(Access::Denied));

    let refused = fixture.act(cx, |me, cx| me.advance(cx));
    assert!(refused.is_err());
    assert_eq!(
        fixture.step(cx),
        Step::Permissions,
        "missing grants hold the step"
    );

    for permission in Permission::ALL {
        fixture.system.set(permission, Access::Granted);
    }
    fixture.act(cx, |me, cx| me.refresh(cx));
    fixture.act(cx, |me, cx| me.advance(cx)).unwrap();
    assert_eq!(fixture.step(cx), Step::MicTest);
    assert_eq!(fixture.settings().get(STEP), Some(json!("mic")));
    assert!(
        fixture.act(cx, |me, cx| me.advance(cx)).is_err(),
        "the mic test has to pass first"
    );
}

#[gpui_kit::test]
fn a_start_resumes_at_the_stored_step(cx: &mut TestAppContext) {
    for step in Step::ALL {
        let rig = rig(cx, &["base"], "base");
        rig.storage
            .settings
            .set_internal(STEP, json!(step.key()))
            .unwrap();
        let fixture = start(cx, rig, Platform::MacOs, FakeSystem::all(Access::Granted));

        assert_eq!(fixture.mode(cx), Mode::Setup);
        assert_eq!(fixture.step(cx), step);
        let expected = match step {
            Step::Permissions | Step::MicTest | Step::Model => KeyGate::Closed,
            Step::Practice => KeyGate::Practice,
            Step::Updates => KeyGate::Open,
        };
        assert_eq!(fixture.gate(cx), expected, "{step:?}");
    }
}

#[gpui_kit::test]
fn the_mic_test_passes_with_a_level_and_words(cx: &mut TestAppContext) {
    let fixture = mac(cx, &["base"], "base");
    go_to(cx, &fixture, Step::MicTest);
    say(&fixture.rig, outcome_text("the quick brown fox", "en"));

    fixture.act(cx, |me, cx| me.toggle_mic_test(cx)).unwrap();
    assert_eq!(
        fixture.read(cx, |me, _| me.mic_test().phase),
        MicPhase::Listening
    );
    level(cx, &fixture, 0.4);
    fixture.act(cx, |me, cx| me.toggle_mic_test(cx)).unwrap();
    assert_eq!(
        fixture.read(cx, |me, _| me.mic_test().phase),
        MicPhase::Checking
    );
    assert!(
        !fixture.read(cx, |me, cx| me.can_continue(cx)),
        "not before the words arrive"
    );
    settle(cx, &fixture.rig);

    let test = fixture.read(cx, |me, _| me.mic_test().clone());
    assert!(test.passed);
    assert!(test.peak >= 0.4);
    assert_eq!(
        test.verdict,
        Some(MicVerdict::Passed {
            transcript: Some("the quick brown fox".into())
        })
    );
    assert!(fixture.read(cx, |me, cx| me.can_continue(cx)));
    assert!(
        crate::controller::testkit::session_wavs(&fixture.rig).is_empty(),
        "the test recording is deleted"
    );
    fixture.act(cx, |me, cx| me.advance(cx)).unwrap();
    assert_eq!(fixture.step(cx), Step::Model);
}

#[gpui_kit::test]
fn a_silent_mic_test_says_so_and_does_not_pass(cx: &mut TestAppContext) {
    let fixture = mac(cx, &["base"], "base");
    go_to(cx, &fixture, Step::MicTest);

    fixture.act(cx, |me, cx| me.toggle_mic_test(cx)).unwrap();
    level(cx, &fixture, 0.0);
    fixture.act(cx, |me, cx| me.toggle_mic_test(cx)).unwrap();
    settle(cx, &fixture.rig);

    let test = fixture.read(cx, |me, _| me.mic_test().clone());
    assert_eq!(test.phase, MicPhase::Idle);
    assert_eq!(test.verdict, Some(MicVerdict::NoSound));
    assert!(!test.passed);
    assert!(
        fixture.rig.specs.lock().unwrap().is_empty(),
        "no engine call for silence"
    );
    assert!(fixture.act(cx, |me, cx| me.advance(cx)).is_err());
}

#[gpui_kit::test]
fn sound_without_words_is_not_a_pass(cx: &mut TestAppContext) {
    let fixture = mac(cx, &["base"], "base");
    go_to(cx, &fixture, Step::MicTest);
    say(&fixture.rig, outcome_text("[BLANK_AUDIO]", "en"));

    fixture.act(cx, |me, cx| me.toggle_mic_test(cx)).unwrap();
    level(cx, &fixture, 0.3);
    fixture.act(cx, |me, cx| me.toggle_mic_test(cx)).unwrap();
    settle(cx, &fixture.rig);

    let test = fixture.read(cx, |me, _| me.mic_test().clone());
    assert_eq!(test.verdict, Some(MicVerdict::NoSpeech));
    assert!(!test.passed);
}

#[gpui_kit::test]
fn a_loud_test_without_a_model_still_passes_the_level_check(cx: &mut TestAppContext) {
    let fixture = mac(cx, &[], "base");
    go_to(cx, &fixture, Step::MicTest);

    fixture.act(cx, |me, cx| me.toggle_mic_test(cx)).unwrap();
    level(cx, &fixture, 0.5);
    fixture.act(cx, |me, cx| me.toggle_mic_test(cx)).unwrap();

    let test = fixture.read(cx, |me, _| me.mic_test().clone());
    assert_eq!(test.verdict, Some(MicVerdict::Passed { transcript: None }));
    assert!(test.passed);
}

#[gpui_kit::test]
fn the_model_step_is_ready_when_the_file_is_there_and_names_the_default(cx: &mut TestAppContext) {
    let missing = mac(cx, &[], "");
    go_to_unchecked(cx, &missing, Step::Model);
    assert!(!missing.read(cx, |me, cx| me.can_continue(cx)));
    assert!(missing.act(cx, |me, cx| me.advance(cx)).is_err());
    assert_eq!(
        missing.read(cx, |me, cx| me.model_id(cx)),
        "base",
        "the catalog default"
    );

    let present = mac(cx, &["base"], "");
    go_to_unchecked(cx, &present, Step::Model);
    assert!(present.read(cx, |me, cx| me.can_continue(cx)));
    present.act(cx, |me, cx| me.advance(cx)).unwrap();
    assert_eq!(present.step(cx), Step::Practice);
    assert_eq!(
        present.settings().get("dictation.modelId"),
        Some(json!("base")),
        "the downloaded default becomes the chosen model"
    );
}

/// Opens a step the way a restart at that step does, without playing the earlier ones.
fn go_to_unchecked(cx: &mut TestAppContext, fixture: &Fixture, step: Step) {
    fixture.act(cx, |me, cx| me.go(step, cx));
}

#[gpui_kit::test]
fn the_practice_step_passes_only_through_a_dictation_and_then_opens_the_keys(
    cx: &mut TestAppContext,
) {
    let fixture = mac(cx, &["base"], "base");
    go_to_unchecked(cx, &fixture, Step::Practice);
    assert_eq!(fixture.gate(cx), KeyGate::Practice);
    assert!(!fixture.read(cx, |me, cx| me.can_continue(cx)));
    assert!(fixture.act(cx, |me, cx| me.advance(cx)).is_err());
    say(&fixture.rig, outcome_text("the quick brown fox", "en"));

    hold(cx, &fixture, 1_000).unwrap();

    assert_eq!(
        fixture
            .read(cx, |me, _| me.practice_text().map(str::to_owned))
            .as_deref(),
        Some("the quick brown fox")
    );
    assert!(fixture.read(cx, |me, cx| me.can_continue(cx)));
    assert_eq!(fixture.gate(cx), KeyGate::Open);
    fixture.act(cx, |me, cx| me.advance(cx)).unwrap();
    assert_eq!(fixture.step(cx), Step::Updates);
}

#[gpui_kit::test]
fn before_the_practice_step_a_hold_key_starts_nothing(cx: &mut TestAppContext) {
    for step in [Step::Permissions, Step::MicTest, Step::Model] {
        let fixture = mac(cx, &["base"], "base");
        go_to_unchecked(cx, &fixture, step);

        let result = send_at(cx, &fixture.rig, 1_000, AppEvent::HoldDown);

        assert!(result.is_err(), "{step:?}");
        assert!(fixture.rig.specs.lock().unwrap().is_empty());
    }
}

#[gpui_kit::test]
fn finishing_with_the_default_keeps_update_checks_off_and_never_opens_again(
    cx: &mut TestAppContext,
) {
    let fixture = mac(cx, &["base"], "base");
    go_to_unchecked(cx, &fixture, Step::Updates);
    assert!(!fixture.read(cx, |me, _| me.updates_on()), "off by default");

    fixture.act(cx, |me, cx| me.advance(cx)).unwrap();

    assert_eq!(fixture.mode(cx), Mode::Hidden);
    assert_eq!(fixture.settings().get(UPDATES), Some(json!(false)));
    assert_eq!(fixture.settings().get(COMPLETED), Some(json!(true)));
    assert_eq!(fixture.settings().get(STEP), Some(json!("")));
    assert_eq!(fixture.gate(cx), KeyGate::Open);

    let again = start(
        cx,
        fixture.rig,
        Platform::MacOs,
        FakeSystem::all(Access::Granted),
    );
    assert_eq!(
        again.mode(cx),
        Mode::Hidden,
        "a restart opens the main window"
    );
}

#[gpui_kit::test]
fn choosing_updates_on_is_saved_at_the_end(cx: &mut TestAppContext) {
    let fixture = mac(cx, &["base"], "base");
    go_to_unchecked(cx, &fixture, Step::Updates);

    fixture.act(cx, |me, cx| me.set_updates(true, cx));
    assert_eq!(
        fixture.settings().get(UPDATES),
        Some(json!(false)),
        "nothing is saved before Finish"
    );
    fixture.act(cx, |me, cx| me.advance(cx)).unwrap();

    assert_eq!(fixture.settings().get(UPDATES), Some(json!(true)));
}

#[gpui_kit::test]
fn a_lost_mac_grant_reopens_the_permissions_step_and_nothing_else_changes(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let settings = &rig.storage.settings;
    settings.set_internal(COMPLETED, json!(true)).unwrap();
    settings
        .set_internal(
            "permissions.lastGranted",
            json!({"microphone": true, "accessibility": true, "inputMonitoring": true}),
        )
        .unwrap();
    let system = FakeSystem::all(Access::Granted);
    system.set(Permission::InputMonitoring, Access::Denied);
    let before = settings.values();
    let fixture = start(cx, rig, Platform::MacOs, system);
    list_microphones(cx, &fixture, vec![device()]);

    assert_eq!(fixture.mode(cx), Mode::Repair);
    assert_eq!(fixture.step(cx), Step::Permissions);
    assert_eq!(
        fixture.gate(cx),
        KeyGate::Open,
        "dictation keeps its normal gate"
    );
    assert_eq!(fixture.settings().values(), before, "no setting changed");
    assert!(
        fixture.act(cx, |me, cx| me.advance(cx)).is_err(),
        "the grant is still gone"
    );

    fixture
        .system
        .set(Permission::InputMonitoring, Access::Granted);
    fixture.act(cx, |me, cx| me.refresh(cx));
    fixture.act(cx, |me, cx| me.advance(cx)).unwrap();

    assert_eq!(
        fixture.mode(cx),
        Mode::Hidden,
        "back to the main window, no other step"
    );
    assert_eq!(fixture.settings().get(COMPLETED), Some(json!(true)));
    assert_eq!(fixture.settings().values(), before);
    assert_eq!(
        fixture.rig.models.read_with(cx, |m, _| m.active()),
        "base",
        "the model choice is untouched"
    );
}

#[gpui_kit::test]
fn a_linux_start_with_no_microphone_reopens_the_permissions_step(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    rig.storage
        .settings
        .set_internal(COMPLETED, json!(true))
        .unwrap();
    let before = rig.storage.settings.values();
    let fixture = start(
        cx,
        rig,
        Platform::Linux,
        FakeSystem::all(Access::NotApplicable),
    );
    assert_eq!(
        fixture.mode(cx),
        Mode::Hidden,
        "until the microphones are listed"
    );

    list_microphones(cx, &fixture, Vec::new());

    assert_eq!(fixture.mode(cx), Mode::Repair);
    assert_eq!(fixture.step(cx), Step::Permissions);
    assert_eq!(row(cx, &fixture, "microphone"), RowState::Missing);
    assert_eq!(fixture.settings().values(), before);

    list_microphones(cx, &fixture, vec![device()]);
    assert_eq!(row(cx, &fixture, "microphone"), RowState::Ready);
    fixture.act(cx, |me, cx| me.advance(cx)).unwrap();
    assert_eq!(fixture.mode(cx), Mode::Hidden);
    assert_eq!(fixture.settings().values(), before);
}

#[gpui_kit::test]
fn a_linux_start_with_a_microphone_stays_on_the_main_window(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    rig.storage
        .settings
        .set_internal(COMPLETED, json!(true))
        .unwrap();
    let fixture = start(
        cx,
        rig,
        Platform::Linux,
        FakeSystem::all(Access::NotApplicable),
    );

    list_microphones(cx, &fixture, vec![device()]);

    assert_eq!(fixture.mode(cx), Mode::Hidden);
}

#[gpui_kit::test]
fn the_state_section_reports_step_gate_and_rows(cx: &mut TestAppContext) {
    let fixture = mac(cx, &["base"], "base");

    let state = fixture.read(cx, |me, cx| me.state_json(cx));

    assert_eq!(state["active"], true);
    assert_eq!(state["mode"], "setup");
    assert_eq!(state["step"], "permissions");
    assert_eq!(state["gate"], "closed");
    assert_eq!(state["updates"]["check"], false);
    assert_eq!(state["permissions"]["rows"].as_array().unwrap().len(), 3);
}

mod window {
    use super::*;
    use crate::shell::Shell;
    use crate::theme;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AnyWindowHandle, Bounds, Point, WindowBounds, WindowOptions, px, size};

    fn open(cx: &mut TestAppContext, fixture: &Fixture) -> AnyWindowHandle {
        cx.update(gpui_kit::init);
        let onboarding = fixture.onboarding.clone();
        let (handle, _shell) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Point::default(),
                        size: size(px(940.0), px(600.0)),
                    })),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| {
                        let mut shell = Shell::new(window, cx);
                        shell.attach_onboarding(onboarding, cx);
                        shell
                    })
                },
            )
            .expect("open test window")
        });
        frame(cx, handle);
        handle
    }

    fn frame(cx: &mut TestAppContext, handle: AnyWindowHandle) {
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
    }

    fn present(
        cx: &mut TestAppContext,
        handle: AnyWindowHandle,
        id: impl Into<gpui_kit::SharedString>,
    ) -> bool {
        let id = id.into();
        cx.update_window(handle, |_, window, _| window.try_find(id).is_some())
            .unwrap()
    }

    fn focused(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> bool {
        cx.update_window(handle, |_, window, _| {
            window.try_find(id).and_then(|found| found.focused()) == Some(true)
        })
        .unwrap()
    }

    fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, key: &'static str) {
        cx.update_window(handle, |_, window, cx| window.press(key, cx))
            .unwrap();
        frame(cx, handle);
    }

    /// Tab until `id` has focus. A step has a handful of controls, so a few presses are enough.
    fn tab_to(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) {
        for _ in 0..8 {
            if focused(cx, handle, id) {
                return;
            }
            press(cx, handle, "tab");
        }
        assert!(focused(cx, handle, id), "Tab never reached {id}");
    }

    #[gpui_kit::test]
    fn onboarding_takes_the_window_and_has_no_skip_control(cx: &mut TestAppContext) {
        let fixture = mac(cx, &["base"], "base");
        let handle = open(cx, &fixture);

        assert!(present(cx, handle, "onboarding.page"));
        assert!(present(cx, handle, "onboarding.continue"));
        assert!(
            !present(cx, handle, "sidebar.home"),
            "no sidebar to leave by"
        );
        for id in ["onboarding.skip", "onboarding.back", "onboarding.later"] {
            assert!(!present(cx, handle, id), "{id}");
        }
        assert_eq!(
            present(cx, handle, "window.close"),
            !cfg!(target_os = "macos"),
            "the drawn lights stay on Linux; AppKit draws them on macOS"
        );
    }

    /// `(x, y, width, height)` of an element in the window.
    fn rect(cx: &mut TestAppContext, handle: AnyWindowHandle, id: String) -> (f32, f32, f32, f32) {
        let id = gpui_kit::SharedString::from(id);
        let bounds = cx
            .update_window(handle, |_, window, _| window.find(id).bounds())
            .unwrap();
        (
            f32::from(bounds.origin.x),
            f32::from(bounds.origin.y),
            f32::from(bounds.size.width),
            f32::from(bounds.size.height),
        )
    }

    /// Every permission row is icon, name, status word, and buttons on one line, plus one
    /// secondary line that is cut instead of wrapped.
    fn assert_permission_rows_are_one_line(
        cx: &mut TestAppContext,
        fixture: &Fixture,
        handle: AnyWindowHandle,
    ) {
        let title = rect(cx, handle, "onboarding.title".into());
        assert!(
            (title.3 - theme::TITLE.size * theme::TITLE.line_height).abs() < 0.6,
            "the title is one 22 px line, not {}",
            title.3
        );
        let rows = fixture.read(cx, |me, cx| me.rows(cx));
        assert!(rows.len() >= 3);
        let secondary = theme::SECONDARY.size * theme::SECONDARY.line_height;
        for row in rows {
            let key = row.key.key();
            let whole = rect(cx, handle, format!("onboarding.permissions.row.{key}"));
            let state = rect(cx, handle, format!("onboarding.permissions.state.{key}"));
            let detail = rect(cx, handle, format!("onboarding.permissions.detail.{key}"));
            assert!(
                whole.3 >= theme::size::ROW_MIN_HEIGHT,
                "{key} row height {}",
                whole.3
            );
            assert!(
                whole.3 <= theme::size::ROW_MIN_HEIGHT + secondary,
                "{key} row is one line plus one secondary line, not {}",
                whole.3
            );
            assert!(
                detail.3 <= secondary + 0.5,
                "{key} description is one line: {}",
                detail.3
            );
            let middle = |r: (f32, f32, f32, f32)| r.1 + r.3 / 2.0;
            let mut previous_right = state.0 + state.2;
            assert!(
                detail.0 + detail.2 <= state.0,
                "{key}: the description is cut before the status"
            );
            assert!(
                (middle(state) - middle(whole)).abs() < 1.0,
                "{key}: status on the row's line"
            );
            if row.opens.is_some() {
                // Allow has nothing left to do once the row is Ready.
                let names: &[&str] = if row.state == RowState::Ready {
                    assert!(
                        !present(cx, handle, format!("onboarding.permissions.allow.{key}")),
                        "{key}: Allow is hidden when Ready"
                    );
                    &["open"]
                } else {
                    &["allow", "open"]
                };
                for name in names {
                    let button = rect(cx, handle, format!("onboarding.permissions.{name}.{key}"));
                    assert!(
                        (middle(button) - middle(whole)).abs() < 1.0,
                        "{key} {name} is on the row's line"
                    );
                    assert!(
                        button.0 >= previous_right,
                        "{key} {name} comes after the previous control"
                    );
                    assert_eq!(button.3, theme::size::BUTTON_HEIGHT, "{key} {name}");
                    previous_right = button.0 + button.2;
                }
                assert!(
                    previous_right <= whole.0 + whole.2,
                    "{key}: controls stay inside the row"
                );
            }
        }
    }

    #[gpui_kit::test]
    fn permission_rows_stay_on_one_line_in_setup(cx: &mut TestAppContext) {
        let fixture = mac(cx, &["base"], "base");
        let handle = open(cx, &fixture);
        assert_permission_rows_are_one_line(cx, &fixture, handle);
    }

    #[gpui_kit::test]
    fn permission_rows_stay_on_one_line_on_the_permissions_changed_page(cx: &mut TestAppContext) {
        let rig = rig(cx, &["base"], "base");
        rig.storage
            .settings
            .set_internal(COMPLETED, json!(true))
            .unwrap();
        rig.storage
            .settings
            .set_internal(
                "permissions.lastGranted",
                json!({"microphone": true, "accessibility": true, "inputMonitoring": true}),
            )
            .unwrap();
        let system = FakeSystem::all(Access::Granted);
        system.set(Permission::Accessibility, Access::Denied);
        system.set(Permission::InputMonitoring, Access::Denied);
        let fixture = start(cx, rig, Platform::MacOs, system);
        list_microphones(cx, &fixture, vec![device()]);
        assert_eq!(fixture.mode(cx), Mode::Repair);
        let handle = open(cx, &fixture);
        assert_permission_rows_are_one_line(cx, &fixture, handle);
        let state = rect(
            cx,
            handle,
            "onboarding.permissions.state.accessibility".into(),
        );
        assert!(state.2 > 0.0);
    }

    #[gpui_kit::test]
    fn the_keyboard_alone_goes_from_permissions_to_the_main_window(cx: &mut TestAppContext) {
        let fixture = mac(cx, &["base"], "base");
        let handle = open(cx, &fixture);
        say(&fixture.rig, outcome_text("the quick brown fox", "en"));

        tab_to(cx, handle, "onboarding.continue");
        press(cx, handle, "enter");
        assert_eq!(fixture.step(cx), Step::MicTest);

        tab_to(cx, handle, "onboarding.mic.test");
        press(cx, handle, "space");
        level(cx, &fixture, 0.4);
        press(cx, handle, "space");
        settle(cx, &fixture.rig);
        frame(cx, handle);
        tab_to(cx, handle, "onboarding.continue");
        press(cx, handle, "enter");
        assert_eq!(fixture.step(cx), Step::Model);

        tab_to(cx, handle, "onboarding.continue");
        press(cx, handle, "enter");
        assert_eq!(fixture.step(cx), Step::Practice);
        assert!(present(cx, handle, "onboarding.practice.field"));

        say(&fixture.rig, outcome_text("the quick brown fox", "en"));
        hold(cx, &fixture, 1_000).unwrap();
        frame(cx, handle);
        tab_to(cx, handle, "onboarding.continue");
        press(cx, handle, "enter");
        assert_eq!(fixture.step(cx), Step::Updates);

        tab_to(cx, handle, "onboarding.updates.toggle");
        press(cx, handle, "space");
        assert!(fixture.read(cx, |me, _| me.updates_on()));
        press(cx, handle, "space");
        assert!(!fixture.read(cx, |me, _| me.updates_on()));
        tab_to(cx, handle, "onboarding.continue");
        press(cx, handle, "enter");

        assert_eq!(fixture.mode(cx), Mode::Hidden);
        assert!(
            present(cx, handle, "sidebar.home"),
            "the main window is back"
        );
        assert!(!present(cx, handle, "onboarding.page"));
    }
}
