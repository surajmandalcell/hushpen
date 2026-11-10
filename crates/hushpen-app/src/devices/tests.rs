use super::*;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{TestAppContext, WindowOptions};
use hushpen_core::remote::THIS_COMPUTER;

fn rig(cx: &mut TestAppContext) -> (tempfile::TempDir, Rc<Storage>, Entity<Devices>) {
    let dir = tempfile::tempdir().unwrap();
    let storage = Rc::new(
        crate::storage::open(
            hushpen_store::data_dir::DataDir::open(dir.path().join("data")).unwrap(),
        )
        .unwrap(),
    );
    let mut devices = None;
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::theme::install(cx);
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            let state = cx.new(|cx| Devices::new(Rc::clone(&storage), window, cx));
            devices = Some(state.clone());
            cx.new(|cx| {
                let mut shell = crate::shell::Shell::new(window, cx);
                shell.attach_devices(state, cx);
                shell
            })
        })
        .unwrap();
    });
    (dir, storage, devices.unwrap())
}

fn phone(id: &str, os: Os) -> RemoteDevice {
    RemoteDevice {
        device: Device {
            id: id.into(),
            name: id.into(),
            kind: DeviceKind::Phone,
            os,
            serves: Serves {
                speech: true,
                cleanup: true,
            },
        },
        address: "phone.local:7342".into(),
        static_public_key: [1; 32],
        transport: Transport::Lan,
        unload_rule: if os == Os::Ios {
            UnloadRule::WhileServerModeOpen
        } else {
            UnloadRule::KeepLoaded
        },
    }
}

#[gpui_kit::test]
fn switches_routes_and_forgetting_persist_without_changing_off_defaults(cx: &mut TestAppContext) {
    let (_dir, storage, devices) = rig(cx);
    assert!(!storage.settings.remote().enabled);
    devices.update(cx, |state, cx| {
        state.add_paired(phone("android", Os::Android), cx).unwrap();
        state.add_paired(phone("ios", Os::Ios), cx).unwrap();
        assert!(!state.settings.enabled);
        state
            .move_device(ModelKind::Speech, "android", true, cx)
            .unwrap();
        assert_eq!(
            state.settings.routes.speech,
            ["android", "ios", THIS_COMPUTER]
        );
        state
            .move_device(ModelKind::Speech, THIS_COMPUTER, true, cx)
            .unwrap();
        assert_eq!(
            state.settings.routes.speech.last().map(String::as_str),
            Some(THIS_COMPUTER)
        );
        state.set_enabled(true, cx).unwrap();
        state.set_no_local(true, cx).unwrap();
        assert!(
            !state
                .settings
                .routes
                .speech
                .iter()
                .any(|id| id == THIS_COMPUTER)
        );
        state.set_no_local(false, cx).unwrap();
        state.forget("android", cx).unwrap();
    });
    let saved = storage.settings.remote();
    assert_eq!(saved.routes.speech, ["ios", THIS_COMPUTER]);
    assert_eq!(saved.routes.cleanup, ["ios", THIS_COMPUTER]);
}

#[gpui_kit::test]
fn device_edits_reject_invalid_names_rules_and_replaced_pins(cx: &mut TestAppContext) {
    let (_dir, storage, devices) = rig(cx);
    devices.update(cx, |state, cx| {
        state.add_paired(phone("ios", Os::Ios), cx).unwrap();
        assert!(state.rename("ios", " ", cx).is_err());
        assert!(state.unload("ios", UnloadRule::KeepLoaded, cx).is_err());
        let mut replacement = phone("ios", Os::Ios);
        replacement.static_public_key = [2; 32];
        assert!(state.add_paired(replacement, cx).is_err());
        state.rename("ios", "Phone", cx).unwrap();
    });
    let saved = storage.settings.remote();
    assert_eq!(saved.devices[0].device.name, "Phone");
    assert_eq!(saved.devices[0].static_public_key, [1; 32]);
    assert_eq!(
        saved.devices[0].unload_rule,
        UnloadRule::WhileServerModeOpen
    );
}

#[gpui_kit::test]
fn closing_clears_pairing_and_snapshot_never_contains_the_offer(cx: &mut TestAppContext) {
    let (_dir, _storage, devices) = rig(cx);
    devices.update(cx, |state, cx| {
        state.set_offer(
            "secret-pairing-payload".into(),
            vec![vec![true]],
            now() + 60,
            cx,
        );
        assert!(
            !state
                .snapshot()
                .to_string()
                .contains("secret-pairing-payload")
        );
        state.request(Request::Discover, cx);
        assert_eq!(state.pairing, Pairing::Idle);
        assert!(state.qr.is_empty());
        state.set_offer("new-payload".into(), vec![vec![true]], now() + 60, cx);
        state.busy = true;
        state.progress.push("working".into());
        state.close(cx);
        assert!(!state.busy);
        assert!(state.progress.is_empty());
        assert_eq!(state.pairing, Pairing::Idle);
        assert!(state.qr.is_empty());
    });
}

/// Devices open in a window of the app's size, with the shell around them and remote use on.
struct Screen {
    handle: gpui_kit::AnyWindowHandle,
    devices: Entity<Devices>,
    storage: Rc<Storage>,
    _dir: tempfile::TempDir,
}

fn screen(cx: &mut TestAppContext, paired: &[(&str, Os)]) -> Screen {
    let dir = tempfile::tempdir().unwrap();
    let storage = Rc::new(
        crate::storage::open(
            hushpen_store::data_dir::DataDir::open(dir.path().join("data")).unwrap(),
        )
        .unwrap(),
    );
    let mut devices = None;
    let handle: gpui_kit::AnyWindowHandle = cx.update(|cx| {
        gpui_kit::init(cx);
        crate::theme::install(cx);
        let options = WindowOptions {
            window_bounds: Some(gpui_kit::WindowBounds::Windowed(gpui_kit::Bounds {
                origin: gpui_kit::Point::default(),
                size: gpui_kit::size(
                    gpui_kit::px(crate::theme::space::WINDOW_WIDTH),
                    gpui_kit::px(crate::theme::space::WINDOW_HEIGHT),
                ),
            })),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| {
            let state = cx.new(|cx| Devices::new(Rc::clone(&storage), window, cx));
            devices = Some(state.clone());
            cx.new(|cx| {
                let mut shell = crate::shell::Shell::new(window, cx);
                shell.attach_devices(state, cx);
                shell.select(crate::views::View::Devices, cx);
                shell
            })
        })
        .unwrap()
        .0
    });
    let devices = devices.unwrap();
    devices.update(cx, |state, cx| {
        state.set_enabled(true, cx).unwrap();
        for (id, os) in paired {
            state.add_paired(phone(id, *os), cx).unwrap();
        }
    });
    let view = Screen {
        handle,
        devices,
        storage,
        _dir: dir,
    };
    frame(cx, &view);
    view
}

fn frame(cx: &mut TestAppContext, view: &Screen) {
    cx.run_until_parked();
    cx.update_window(view.handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn click(cx: &mut TestAppContext, view: &Screen, id: &'static str) {
    cx.update_window(view.handle, |_, window, cx| window.click(id, cx))
        .unwrap();
    frame(cx, view);
}

fn present(cx: &mut TestAppContext, view: &Screen, id: &'static str) -> bool {
    cx.update_window(view.handle, |_, window, _| window.try_find(id).is_some())
        .unwrap()
}

fn details(cx: &mut TestAppContext, view: &Screen, id: &str) {
    let devices = view.devices.clone();
    cx.update_window(view.handle, |_, window, cx| {
        devices.update(cx, |state, cx| state.open_detail(id, window, cx))
    })
    .unwrap();
    frame(cx, view);
}

#[gpui_kit::test]
fn the_arrows_move_a_device_in_one_route_and_stop_at_the_ends(cx: &mut TestAppContext) {
    let view = screen(cx, &[("android", Os::Android), ("ios", Os::Ios)]);
    let routes = || {
        let saved = view.storage.settings.remote().routes;
        (saved.speech.join(","), saved.cleanup.join(","))
    };
    let start = routes();
    assert_eq!(start.0, "ios,android,this-computer");
    click(cx, &view, "devices.speech.ios.up");
    click(cx, &view, "devices.speech.this-computer.up");
    assert_eq!(
        routes(),
        start,
        "the first device and This computer stay put"
    );
    click(cx, &view, "devices.speech.android.up");
    assert_eq!(
        routes(),
        ("android,ios,this-computer".into(), start.1.clone()),
        "only the speech route changes"
    );
    click(cx, &view, "devices.speech.ios.down");
    assert_eq!(routes().0, "android,ios,this-computer");
}

#[gpui_kit::test]
fn the_page_takes_no_input_while_remote_use_is_off(cx: &mut TestAppContext) {
    let view = screen(cx, &[("android", Os::Android)]);
    view.devices
        .update(cx, |state, cx| state.set_enabled(false, cx).unwrap());
    frame(cx, &view);
    click(cx, &view, "devices.noLocal");
    assert!(!view.storage.settings.remote().no_local_models);
    click(cx, &view, "devices.add");
    assert!(view.devices.read_with(cx, |state, _| state.sheet.is_none()));
    click(cx, &view, "devices.enabled");
    assert!(view.storage.settings.remote().enabled);
    click(cx, &view, "devices.noLocal");
    assert!(view.storage.settings.remote().no_local_models);
}

#[gpui_kit::test]
fn the_details_offer_only_the_unload_rules_the_system_allows(cx: &mut TestAppContext) {
    let view = screen(cx, &[("android", Os::Android), ("ios", Os::Ios)]);
    details(cx, &view, "ios");
    click(cx, &view, "devices.rule");
    assert!(
        !present(cx, &view, "devices.rule.list"),
        "an iPhone has one rule"
    );
    details(cx, &view, "android");
    click(cx, &view, "devices.rule");
    for option in [
        "devices.rule.KeepLoaded",
        "devices.rule.AfterIdle",
        "devices.rule.WhenMemoryLow",
        "devices.rule.WhileServerModeOpen",
    ] {
        assert!(present(cx, &view, option), "{option}");
    }
    assert!(!present(cx, &view, "devices.idleTime"));
    click(cx, &view, "devices.rule.AfterIdle");
    let saved = view.storage.settings.remote();
    assert_eq!(saved.devices[0].unload_rule, UnloadRule::AfterIdle(15));
    assert!(present(cx, &view, "devices.idleTime"));
}

#[gpui_kit::test]
fn remove_device_asks_in_a_row_before_it_forgets(cx: &mut TestAppContext) {
    let view = screen(cx, &[("android", Os::Android)]);
    details(cx, &view, "android");
    click(cx, &view, "devices.remove");
    assert!(present(cx, &view, "devices.forget.ask"));
    click(cx, &view, "devices.forget.cancel");
    assert!(!present(cx, &view, "devices.forget.ask"));
    assert_eq!(view.storage.settings.remote().devices.len(), 1);
    click(cx, &view, "devices.remove");
    click(cx, &view, "devices.forget");
    assert!(view.storage.settings.remote().devices.is_empty());
    assert!(!present(cx, &view, "devices.sheet"));
}
