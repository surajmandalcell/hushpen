use super::*;
use crate::secrets::Secrets;

fn phone() -> RemoteDevice {
    RemoteDevice {
        device: Device {
            id: "pixel".into(),
            name: "Pixel 10".into(),
            kind: DeviceKind::Phone,
            os: Os::Android,
            serves: Serves {
                speech: true,
                cleanup: true,
            },
        },
        address: "pixel.local:6745".into(),
        static_public_key: [7; 32],
        transport: Transport::Lan,
        unload_rule: UnloadRule::AfterIdle(30),
    }
}

#[test]
fn migration_adds_disabled_routes_without_changing_existing_settings_or_unknown_keys() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("settings.json");
    std::fs::write(&path, json!({"schemaVersion":4,"values":{"audio.inputGain":1.25,"shortcut.handsFree":"Ctrl+Alt+F9","future.setting":7,"_unknown":{"older":true}}}).to_string()).unwrap();
    let settings = SettingsStore::open(root.path()).unwrap();
    assert_eq!(settings.remote(), RemoteSettings::default());
    assert_eq!(settings.get("audio.inputGain"), Some(json!(1.25)));
    assert_eq!(
        settings.get("shortcut.handsFree"),
        Some(json!("Ctrl+Alt+F9"))
    );
    assert_eq!(settings.unknown().get("future.setting"), Some(&json!(7)));
    assert_eq!(settings.unknown().get("older"), Some(&json!(true)));
    let document: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(document["schemaVersion"], 5);
}

#[test]
fn devices_routes_and_private_identity_survive_a_restart() {
    let root = tempfile::tempdir().unwrap();
    let settings = SettingsStore::open(root.path()).unwrap();
    let mut remote = RemoteSettings {
        enabled: true,
        devices: vec![phone()],
        ..RemoteSettings::default()
    };
    remote.routes.speech = vec![THIS_COMPUTER.into(), "pixel".into(), "pixel".into()];
    remote.routes.cleanup = vec!["pixel".into()];
    settings.set_remote(&remote).unwrap();
    let secrets = Secrets::open(root.path()).unwrap();
    secrets.set_endpoint_api_key(Some("endpoint-test")).unwrap();
    secrets.set_noise_private_key(&[23; 32]).unwrap();
    secrets
        .set_pair_offer(Some("hushpen-pair://v1?otp=123456"))
        .unwrap();
    drop(settings);
    drop(secrets);
    let settings = SettingsStore::open(root.path()).unwrap();
    let remote = settings.remote();
    assert!(remote.enabled);
    assert_eq!(remote.devices, [phone()]);
    assert_eq!(remote.routes.speech, ["pixel", THIS_COMPUTER]);
    assert_eq!(remote.routes.cleanup, ["pixel", THIS_COMPUTER]);
    let secrets = Secrets::open(root.path()).unwrap();
    assert_eq!(secrets.noise_private_key(), Some([23; 32]));
    assert_eq!(secrets.endpoint_api_key().as_deref(), Some("endpoint-test"));
    assert_eq!(
        secrets.pair_offer().as_deref(),
        Some("hushpen-pair://v1?otp=123456")
    );
    secrets.set_pair_offer(None).unwrap();
    assert_eq!(Secrets::open(root.path()).unwrap().pair_offer(), None);
    let values = settings.values();
    assert!(
        values
            .keys()
            .all(|key| !key.to_ascii_lowercase().contains("private"))
    );
    let text = std::fs::read_to_string(settings.path()).unwrap();
    assert!(!text.contains("noiseStaticPrivateKey"));
    assert!(!text.contains("pairOffer"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(secrets.path())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}

#[test]
fn each_transport_and_platform_rule_round_trips_and_unsupported_rules_are_refused() {
    let mut device = phone();
    for transport in [
        Transport::Lan,
        Transport::Tailscale,
        Transport::Ssh {
            host: "lin1".into(),
        },
    ] {
        device.transport = transport;
        assert_eq!(
            RemoteDevice::from_json(&device.to_json()),
            Some(device.clone())
        );
    }
    for (os, rule) in [
        (Os::Mac, UnloadRule::KeepLoaded),
        (Os::Windows, UnloadRule::AfterIdle(5)),
        (Os::Linux, UnloadRule::WhenMemoryLow),
        (Os::Ios, UnloadRule::WhileServerModeOpen),
    ] {
        device.device.os = os;
        device.unload_rule = rule;
        assert_eq!(
            RemoteDevice::from_json(&device.to_json()),
            Some(device.clone())
        );
    }
    device.unload_rule = UnloadRule::KeepLoaded;
    assert_eq!(RemoteDevice::from_json(&device.to_json()), None);
}

#[test]
fn an_invalid_device_change_keeps_existing_pins_and_routes() {
    let root = tempfile::tempdir().unwrap();
    let settings = SettingsStore::open(root.path()).unwrap();
    let valid = RemoteSettings {
        enabled: true,
        devices: vec![phone()],
        ..RemoteSettings::default()
    };
    settings.set_remote(&valid).unwrap();
    let mut bad = valid.clone();
    bad.devices[0].static_public_key = [0; 32];
    assert!(settings.set_remote(&bad).is_err());
    assert_eq!(settings.remote(), valid);
    for address in [
        "",
        "pixel",
        "pixel:0",
        "pixel:65536",
        "pixel:abc",
        "user@pixel:6745",
        "pixel:6745/",
        "two words:6745",
    ] {
        bad = valid.clone();
        bad.devices[0].address = address.into();
        assert!(settings.set_remote(&bad).is_err(), "{address}");
        assert_eq!(settings.remote(), valid);
    }
    for address in ["100.64.0.1:6745", "pixel.tailnet.ts.net:6745", "[::1]:6745"] {
        assert!(valid_address(address), "{address}");
    }
    let duplicate = json!([phone().to_json(), phone().to_json()]);
    assert!(settings.set("remote.devices", duplicate).is_err());
    let mut json = phone().to_json();
    json["os"] = json!("unknown");
    assert!(
        settings
            .set("remote.devices", Value::Array(vec![json]))
            .is_err()
    );
}

#[test]
fn local_routes_follow_the_switch_and_disabling_remote_restores_the_desktop_path() {
    let root = tempfile::tempdir().unwrap();
    let settings = SettingsStore::open(root.path()).unwrap();
    settings.set("remote.noLocalModels", json!(true)).unwrap();
    assert_eq!(settings.remote().routes.speech, Vec::<String>::new());
    assert!(settings.remote().local_models_allowed());
    assert_eq!(
        settings.remote().route(ModelKind::Speech).ids(),
        [THIS_COMPUTER]
    );
    settings.set("remote.enabled", json!(true)).unwrap();
    assert!(!settings.remote().local_models_allowed());
    assert!(settings.remote().route(ModelKind::Speech).ids().is_empty());
    settings.set("remote.noLocalModels", json!(false)).unwrap();
    assert_eq!(settings.remote().routes.speech, [THIS_COMPUTER]);
}

#[test]
fn public_device_json_drops_private_fields_and_routes_drop_unknown_or_unsupported_devices() {
    let mut device = phone();
    device.device.serves.cleanup = false;
    let mut value = device.to_json();
    value["privateKey"] = json!("must not be saved");
    let cleaned = sanitize_devices(&json!([value])).unwrap();
    assert!(cleaned[0].get("privateKey").is_none());
    let mut values = Map::new();
    values.insert("remote.devices".into(), cleaned);
    values.insert(
        "remote.routes".into(),
        json!({"speech":["pixel","missing"],"cleanup":["pixel"]}),
    );
    let remote = RemoteSettings::from_values(&values);
    assert_eq!(remote.routes.speech, ["pixel", THIS_COMPUTER]);
    assert_eq!(remote.routes.cleanup, [THIS_COMPUTER]);
}
