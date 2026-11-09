//! Device automation uses the same state intents as the placeholder controls.

use super::{register_action, set_state_section};
use crate::devices::{AddMethod, Devices, Request};
use gpui_kit::{App, Entity};
use hushpen_core::os::Os;
use hushpen_core::remote::{Device, DeviceKind, ModelKind, Serves, UnloadRule};
use hushpen_store::settings::Transport;
use serde_json::{Value, json};

pub fn attach(cx: &mut App, devices: Entity<Devices>) {
    set_state_section(cx, "devices", {
        let devices = devices.clone();
        move |cx| devices.read(cx).snapshot()
    });
    let _ = register_action(
        cx,
        "devices-action",
        "Run a Devices intent. Args: {op:enabled|noLocal|move|add|method|close|rename|forget|unload|discover|offer|ssh|pair|probe, ...}.",
        move |cx, args| {
            let text = |key| {
                args.get(key)
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("Missing {key}."))
            };
            devices.update(cx, |state, cx| -> Result<(), String> {
                match text("op")? {
                    "enabled" => {
                        state.set_enabled(args["value"].as_bool().ok_or("Missing value.")?, cx)?
                    }
                    "noLocal" => {
                        state.set_no_local(args["value"].as_bool().ok_or("Missing value.")?, cx)?
                    }
                    "move" => state.move_device(
                        match text("kind")? {
                            "speech" => ModelKind::Speech,
                            "cleanup" => ModelKind::Cleanup,
                            _ => return Err("Unknown model kind.".into()),
                        },
                        text("id")?,
                        args["up"].as_bool().ok_or("Missing up.")?,
                        cx,
                    )?,
                    "add" => state.open_add(cx),
                    "method" => state.select_method(
                        match text("method")? {
                            "phone" => AddMethod::Phone,
                            "ssh" => AddMethod::Ssh,
                            "tailscale" => AddMethod::Tailscale,
                            _ => return Err("Unknown method.".into()),
                        },
                        cx,
                    ),
                    "close" => state.close(cx),
                    "rename" => state.rename(text("id")?, text("name")?, cx)?,
                    "forget" => state.forget(text("id")?, cx)?,
                    "unload" => state.unload(
                        text("id")?,
                        match text("rule")? {
                            "keepLoaded" => UnloadRule::KeepLoaded,
                            "afterIdle" => UnloadRule::AfterIdle(
                                args["minutes"]
                                    .as_u64()
                                    .and_then(|minutes| u32::try_from(minutes).ok())
                                    .ok_or("Missing minutes.")?,
                            ),
                            "whenMemoryLow" => UnloadRule::WhenMemoryLow,
                            "whileServerModeOpen" => UnloadRule::WhileServerModeOpen,
                            _ => return Err("Unknown unload rule.".into()),
                        },
                        cx,
                    )?,
                    "discover" => state.request(Request::Discover, cx),
                    "offer" => state.request(
                        Request::Offer {
                            host: text("host")?.into(),
                        },
                        cx,
                    ),
                    "ssh" => state.request(
                        Request::SetupSsh {
                            host: text("host")?.into(),
                        },
                        cx,
                    ),
                    "probe" => state.request(Request::Probe, cx),
                    "pair" => {
                        let os = match text("os")? {
                            "linux" => Os::Linux,
                            "macos" => Os::Mac,
                            "windows" => Os::Windows,
                            "android" => Os::Android,
                            "ios" => Os::Ios,
                            _ => return Err("Unknown OS.".into()),
                        };
                        let device = Device {
                            id: text("id")?.into(),
                            name: text("name")?.into(),
                            os,
                            kind: if matches!(os, Os::Ios | Os::Android) {
                                DeviceKind::Phone
                            } else {
                                DeviceKind::Server
                            },
                            serves: Serves {
                                speech: args["speech"].as_bool().unwrap_or(true),
                                cleanup: args["cleanup"].as_bool().unwrap_or(true),
                            },
                        };
                        let transport = match args
                            .get("transport")
                            .and_then(Value::as_str)
                            .unwrap_or("lan")
                        {
                            "lan" => Transport::Lan,
                            "tailscale" => Transport::Tailscale,
                            _ => return Err("Unknown transport.".into()),
                        };
                        state.request(
                            Request::Pair {
                                uri: text("uri")?.into(),
                                device,
                                transport,
                            },
                            cx,
                        );
                    }
                    _ => return Err("Unknown Devices intent.".into()),
                }
                Ok(())
            })?;
            Ok(json!({"ok":true}))
        },
    );
}
