//! Test hook for the Manual view in Read aloud mode: the `read` state section and the actions
//! that set the text and the voice, and play, stop, or export like the view's controls do.
//! Compiled only with the `test-automation` feature.

use super::{register_action, set_state_section};
use crate::read_aloud::{EngineKind, ReadAloud};
use gpui_kit::{App, Entity};
use hushpen_speech::TextFormat;
use serde_json::{Value, json};

fn text_arg<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("needs args like {{\"{key}\": \"...\"}}"))
}

pub fn attach(cx: &mut App, read: Entity<ReadAloud>) {
    set_state_section(cx, "read", {
        let read = read.clone();
        move |cx| read.read(cx).state_json(cx)
    });
    let _ = register_action(
        cx,
        "read-text",
        "Put text in the text area like typing. Args: {\"text\": \"Hello\"}.",
        {
            let read = read.clone();
            move |cx, args| {
                let text = text_arg(&args, "text")?.to_owned();
                read.update(cx, |read, cx| read.set_text(&text, TextFormat::Plain, cx));
                Ok(json!({"chars": text.chars().count()}))
            }
        },
    );
    let _ = register_action(
        cx,
        "read-engine",
        "Pick the engine. Args: {\"engine\": \"system\" | \"kokoro\"}.",
        {
            let read = read.clone();
            move |cx, args| {
                let engine = EngineKind::from_key(text_arg(&args, "engine")?)
                    .ok_or("needs args like {\"engine\": \"kokoro\"}")?;
                read.update(cx, |read, cx| read.set_engine(engine, cx))?;
                Ok(json!({"engine": engine.key()}))
            }
        },
    );
    let _ = register_action(
        cx,
        "read-voice",
        "Pick a voice by id, or the default with {\"id\": null}. Args: {\"id\": \"af_heart\"}. \
         The ids are in `hookctl state` section `read`.",
        {
            let read = read.clone();
            move |cx, args| {
                let id = args.get("id").and_then(Value::as_str).map(str::to_owned);
                read.update(cx, |read, cx| read.set_voice(id.as_deref(), cx))?;
                Ok(json!({"id": id}))
            }
        },
    );
    let _ = register_action(
        cx,
        "read-speed",
        "Set the speed like the slider. Args: {\"speed\": 1.25}, from 0.5 to 2.",
        {
            let read = read.clone();
            move |cx, args| {
                let speed = args
                    .get("speed")
                    .and_then(Value::as_f64)
                    .ok_or("needs args like {\"speed\": 1.25}")?;
                read.update(cx, |read, cx| read.set_speed(speed as f32, cx))?;
                Ok(json!({"speed": read.read(cx).speed()}))
            }
        },
    );
    let _ = register_action(cx, "read-play", "Press Play.", {
        let read = read.clone();
        move |cx, _| {
            read.update(cx, |read, cx| read.play(cx))?;
            Ok(Value::Null)
        }
    });
    let _ = register_action(cx, "read-stop", "Press Stop.", {
        let read = read.clone();
        move |cx, _| {
            read.update(cx, |read, cx| read.stop(cx));
            Ok(Value::Null)
        }
    });
    let _ = register_action(
        cx,
        "read-export",
        "Press Export Audio. The save dialog answer comes from the dialog answer of the test \
         hook.",
        move |cx, _| {
            read.update(cx, |read, cx| read.export(cx))?;
            Ok(Value::Null)
        },
    );
}
