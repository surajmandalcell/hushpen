//! Test hook for the Manual view in Read aloud mode: the `read` state section and the actions
//! that set the text and the voice, and play, stop, or export like the view's controls do; and
//! the `voices` state section with the actions of the Manage Voices sheet.
//! Compiled only with the `test-automation` feature.

use super::{register_action, set_state_section};
use crate::read_aloud::{EngineKind, ReadAloud};
use crate::voices::{Page, Playing, VoiceLibrary};
use gpui_kit::{App, Entity};
use hushpen_speech::TextFormat;
use serde_json::{Value, json};
use std::path::PathBuf;

fn text_arg<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("needs args like {{\"{key}\": \"...\"}}"))
}

pub fn attach(cx: &mut App, read: Entity<ReadAloud>) {
    if let Some(library) = read.read(cx).library().cloned() {
        attach_voices(cx, library);
    }
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
        "Pick the engine. Args: {\"engine\": \"system\" | \"kokoro\" | \"chatterbox-turbo\" | \
         \"chatterbox-nano\"}. An engine whose model is not downloaded shows \
         `read.needs_model`.",
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
         The ids are in `hookctl state` section `read`; Chatterbox lists \"built-in\" and the \
         cloned voices of section `voices`.",
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

fn voice_id(args: &Value) -> Result<String, String> {
    text_arg(args, "id").map(str::to_owned)
}

/// The `voices` section and the actions of the Manage Voices sheet. Each acts on the library
/// as the sheet's controls do, without moving focus.
fn attach_voices(cx: &mut App, library: Entity<VoiceLibrary>) {
    set_state_section(cx, "voices", {
        let library = library.clone();
        move |cx| library.read(cx).state_json(cx)
    });
    let _ = register_action(
        cx,
        "voices-open",
        "Open the Manage Voices sheet like Manage Voices or Add Voice. Args: {\"page\": \"list\" \
         | \"add\"}.",
        {
            let library = library.clone();
            move |cx, args| {
                let page = match text_arg(&args, "page")? {
                    "list" => Page::List,
                    "add" => Page::Add,
                    _ => return Err("needs args like {\"page\": \"list\"}".into()),
                };
                library.update(cx, |me, cx| me.show(page, cx));
                Ok(json!({"sheet": library.read(cx).state_json(cx)["sheet"]}))
            }
        },
    );
    let _ = register_action(cx, "voices-close", "Close the sheet like Done or Esc.", {
        let library = library.clone();
        move |cx, _| {
            library.update(cx, |me, cx| me.dismiss(cx));
            Ok(Value::Null)
        }
    });
    let _ = register_action(
        cx,
        "voices-take",
        "Press Record on the add page, or Stop while it records. The clip is ready when \
         `voices.take.state` is idle and `voices.clip` is set.",
        {
            let library = library.clone();
            move |cx, _| {
                library.update(cx, |me, cx| me.toggle_take(cx));
                Ok(library.read(cx).state_json(cx)["take"].clone())
            }
        },
    );
    let _ = register_action(
        cx,
        "voices-file",
        "Clone from an audio file like Choose File, without the picker. Args: {\"path\": \
         \"/abs/voice.m4a\"}. The clip is made on a thread; read `voices.clip` or \
         `voices.notice`.",
        {
            let library = library.clone();
            move |cx, args| {
                let path = PathBuf::from(text_arg(&args, "path")?);
                library.update(cx, |me, cx| me.load_file(path, cx));
                Ok(library.read(cx).state_json(cx)["take"].clone())
            }
        },
    );
    let _ = register_action(
        cx,
        "voices-name",
        "Type the name of the new voice. Args: {\"name\": \"Anna\"}.",
        {
            let library = library.clone();
            move |cx, args| {
                let name = text_arg(&args, "name")?.to_owned();
                library.update(cx, |me, cx| me.set_name(name, cx));
                Ok(Value::Null)
            }
        },
    );
    let _ = register_action(
        cx,
        "voices-save",
        "Press Save on the add page. Returns the new voice.",
        {
            let library = library.clone();
            move |cx, _| {
                let entry = library.update(cx, |me, cx| me.save(cx))?;
                Ok(json!({"id": entry.id, "name": entry.name, "duration_ms": entry.duration_ms}))
            }
        },
    );
    let _ = register_action(
        cx,
        "voices-rename",
        "Rename a cloned voice. Args: {\"id\": \"v1a2b3c\", \"name\": \"Anna\"}.",
        {
            let library = library.clone();
            move |cx, args| {
                let (id, name) = (voice_id(&args)?, text_arg(&args, "name")?.to_owned());
                library.update(cx, |me, cx| me.rename_voice(&id, &name, cx))?;
                Ok(json!({"id": id, "name": name}))
            }
        },
    );
    let _ = register_action(
        cx,
        "voices-delete",
        "Delete a cloned voice and its clip, as the confirmed Delete does. A setting that \
         chose it goes back to the default. Args: {\"id\": \"v1a2b3c\"}.",
        {
            let library = library.clone();
            move |cx, args| {
                let id = voice_id(&args)?;
                library.update(cx, |me, cx| me.delete(&id, cx))?;
                Ok(json!({"id": id}))
            }
        },
    );
    let _ = register_action(
        cx,
        "voices-default",
        "Set as Default: a cloned voice by id, or the voice of the model with {\"id\": null}.",
        {
            let library = library.clone();
            move |cx, args| {
                let id = args.get("id").and_then(Value::as_str).map(str::to_owned);
                library.update(cx, |me, cx| me.set_default(id.as_deref(), cx))?;
                Ok(json!({"default": library.read(cx).default_id()}))
            }
        },
    );
    let _ = register_action(
        cx,
        "voices-play",
        "Press the play button of a voice, or of the new clip with {\"id\": \"new\"}. A second \
         press stops it. Args: {\"id\": \"built-in\"}.",
        move |cx, args| {
            let what = match voice_id(&args)?.as_str() {
                "new" => Playing::NewClip,
                id => Playing::Voice(id.to_owned()),
            };
            library.update(cx, |me, cx| me.toggle_play(what, cx));
            Ok(json!({"playing": library.read(cx).state_json(cx)["playing"]}))
        },
    );
}
