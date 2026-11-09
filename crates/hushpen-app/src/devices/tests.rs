use super::*;
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
