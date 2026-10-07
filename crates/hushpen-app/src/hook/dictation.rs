//! Test hook for Home dictation: the `dictation` state section and the actions that record,
//! copy, and choose the language. Compiled only with the `test-automation` feature.

use super::{register_action, set_state_section};
use crate::dictation::Dictation;
use gpui_kit::{App, Entity};
use serde_json::{Value, json};

pub fn attach(cx: &mut App, dictation: Entity<Dictation>) {
    set_state_section(cx, "dictation", {
        let dictation = dictation.clone();
        move |cx| dictation.read(cx).state_json(cx)
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
