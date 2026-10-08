//! Test hook for dictation: the `dictation`, `pipeline`, `global_keys`, and `last_insert` state
//! sections, and the actions that record, feed the pipeline, copy, and choose the language.
//! Compiled only with the `test-automation` feature.

use super::{register_action, set_state_section};
use crate::controller::Controller;
use crate::dictation::Dictation;
use gpui_kit::{App, Entity};
use hushpen_core::dictation::AppEvent;
use serde_json::{Value, json};

/// The pipeline events a test can send. The hold key itself is driven with `xdotool`; these
/// are for the surfaces that have no key, such as the tray and the flow bar.
fn event_named(name: &str) -> Option<AppEvent> {
    Some(match name {
        "hold-down" => AppEvent::HoldDown,
        "hold-up" => AppEvent::HoldUp,
        "hands-free" => AppEvent::HandsFreeToggle,
        "home" => AppEvent::HomeToggle,
        "flow-bar" => AppEvent::FlowBarClick,
        "esc" => AppEvent::Esc,
        _ => return None,
    })
}

pub fn attach(cx: &mut App, dictation: Entity<Dictation>) {
    let controller = dictation.read(cx).controller().clone();
    set_state_section(cx, "dictation", {
        let dictation = dictation.clone();
        move |cx| dictation.read(cx).state_json(cx)
    });
    set_state_section(cx, "pipeline", {
        let controller = controller.clone();
        move |cx| controller.read(cx).pipeline_json()
    });
    set_state_section(cx, "global_keys", {
        let controller = controller.clone();
        move |cx| controller.read(cx).keys_json()
    });
    set_state_section(cx, "last_insert", {
        let controller = controller.clone();
        move |cx| controller.read(cx).last_insert_json()
    });
    let _ = register_action(
        cx,
        "dictation-record",
        "Press the Home record button: start a dictation, or stop the one that is listening.",
        {
            let dictation = dictation.clone();
            move |cx, _| {
                dictation.update(cx, |dictation, cx| dictation.toggle(cx))?;
                Ok(Value::Null)
            }
        },
    );
    let _ = register_action(
        cx,
        "pipeline-event",
        "Send one event to the dictation pipeline, the same one the hold key feeds. \
         Args: {\"event\": \"hold-down\" | \"hold-up\" | \"hands-free\" | \"home\" | \"flow-bar\" | \"esc\"}.",
        move |cx, args| {
            let name = args
                .get("event")
                .and_then(Value::as_str)
                .ok_or_else(|| "needs args like {\"event\": \"hands-free\"}".to_string())?;
            let event =
                event_named(name).ok_or_else(|| format!("'{name}' is not a pipeline event"))?;
            controller.update(cx, |controller: &mut Controller, cx| {
                controller.dispatch(event, cx)
            })?;
            Ok(json!({"event": name}))
        },
    );
    let _ = register_action(
        cx,
        "dictation-language",
        "Choose the dictation language like the picker does. Args: {\"code\": \"auto\"} or {\"code\": \"es\"}.",
        move |cx, args| {
            let code = args
                .get("code")
                .and_then(Value::as_str)
                .ok_or_else(|| "needs args like {\"code\": \"es\"}".to_string())?;
            dictation.update(cx, |dictation, cx| dictation.set_language(code, cx))?;
            Ok(json!({"code": code}))
        },
    );
}
