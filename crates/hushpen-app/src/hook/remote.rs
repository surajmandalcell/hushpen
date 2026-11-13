//! Debug pairing drives the same service and settings as device setup.

use super::{register_action, set_state_section};
use crate::{remote_jobs, storage::Storage};
use gpui_kit::App;
use hushpen_core::os::Os;
use hushpen_core::remote::{Device, DeviceKind, Serves, UnloadRule};
use hushpen_store::settings::{RemoteDevice, Transport};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{self, TryRecvError};
use std::time::{Duration, Instant};

pub fn attach(cx: &mut App, storage: Rc<Storage>) {
    let state = Rc::new(RefCell::new(json!({"state":"idle"})));
    set_state_section(cx, "remote", {
        let state = Rc::clone(&state);
        move |_| state.borrow().clone()
    });
    let _ = register_action(
        cx,
        "remote-pair",
        "Pair through an SSH bootstrap offer and save the authenticated host. Args: {\"offer\":\"JSON bootstrap\",\"id\":\"test-host\",\"name\":\"Test host\",\"noLocalModels\":true}. Results appear in remote.state; the code is never returned.",
        move |cx, args| {
            if state.borrow()["state"] == "pairing" {
                return Err("pairing is already running".into());
            }
            let bootstrap = args
                .get("offer")
                .and_then(Value::as_str)
                .ok_or_else(|| "needs a pairing offer".to_owned())?
                .to_owned();
            hushpen_remote::PairOffer::from_bootstrap(&bootstrap)
                .map_err(|_| "the pairing offer is invalid".to_owned())?;
            let id = text(&args, "id", "test-host")?;
            let name = text(&args, "name", "Test host")?;
            let no_local = args
                .get("noLocalModels")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let identity = remote_jobs::identity(&storage.secrets)
                .map_err(|_| "the device key could not be saved".to_owned())?;
            let resolver = storage.remote.resolver.clone();
            let (sender, receiver) = mpsc::sync_channel(1);
            std::thread::spawn(move || {
                let _ = sender.send(remote_jobs::authenticate_pair(
                    &identity, &bootstrap, &resolver,
                ));
            });
            *state.borrow_mut() = json!({"state":"pairing","id":id});
            let state = Rc::clone(&state);
            let storage = Rc::clone(&storage);
            cx.spawn(async move |cx| {
                let deadline = Instant::now() + Duration::from_secs(15);
                let paired = loop {
                    match receiver.try_recv() {
                        Ok(result) => break result.ok(),
                        Err(TryRecvError::Disconnected) => break None,
                        Err(TryRecvError::Empty) if Instant::now() >= deadline => break None,
                        Err(TryRecvError::Empty) => {
                            cx.background_executor()
                                .timer(Duration::from_millis(25))
                                .await;
                        }
                    }
                };
                let saved = paired.is_some_and(|(address, public_key)| {
                    let mut settings = storage.settings.remote();
                    settings.enabled = true;
                    settings.no_local_models = no_local;
                    settings.devices.retain(|device| device.device.id != id);
                    settings.devices.push(RemoteDevice {
                        device: Device {
                            id: id.clone(),
                            name,
                            kind: DeviceKind::Server,
                            os: Os::Linux,
                            serves: Serves {
                                speech: true,
                                cleanup: true,
                            },
                        },
                        address,
                        static_public_key: public_key,
                        transport: Transport::Lan,
                        unload_rule: UnloadRule::KeepLoaded,
                    });
                    for route in [&mut settings.routes.speech, &mut settings.routes.cleanup] {
                        route.retain(|device| device != &id);
                        route.insert(0, id.clone());
                    }
                    storage.settings.set_remote(&settings).is_ok()
                });
                *state.borrow_mut() = json!({
                    "state":if saved {"paired"} else {"failed"},
                    "id":id,
                    "code":if saved {"OK"} else {"PAIR_FAILED"},
                });
            })
            .detach();
            Ok(json!({"state":"pairing"}))
        },
    );
}

fn text(args: &Value, field: &str, default: &str) -> Result<String, String> {
    let value = args.get(field).and_then(Value::as_str).unwrap_or(default);
    if value.trim().is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(format!("invalid {field}"));
    }
    Ok(value.to_owned())
}
