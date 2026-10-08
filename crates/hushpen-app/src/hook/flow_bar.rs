//! Test hook for the flow bar: the `overlay` state section, the bar's elements in the tree
//! (so a test can click them), and an action that picks a language without scrolling the
//! list. Compiled only with the `test-automation` feature.

use super::{ExtraWindow, record_event, register_action, register_window, set_state_section};
use crate::flow_bar::{FlowBar, SharedHost};
use gpui_kit::{App, Entity};
use serde_json::{Value, json};

pub fn attach(cx: &mut App, bar: Entity<FlowBar>, host: SharedHost) {
    set_state_section(cx, "overlay", {
        let bar = bar.clone();
        let host = host.clone();
        move |cx| {
            let mut state = bar.read(cx).state_json();
            state["window_open"] = json!(host.borrow().window().is_some());
            state
        }
    });
    // Every change of the bar's state is an event, so a suite reads how long the result flash
    // lasted from the hook's own timestamps.
    let mut last = bar.read(cx).state_json()["state"].clone();
    cx.observe(&bar, move |bar, cx| {
        let state = bar.read(cx).state_json()["state"].clone();
        if state != last {
            record_event("overlay", state.as_str().unwrap_or_default());
            last = state;
        }
    })
    .detach();
    register_window(cx, {
        let bar = bar.clone();
        let host = host.clone();
        move |cx| {
            let window = host.borrow().window()?;
            let ((x, y), _) = bar.read(cx).frame();
            Some(ExtraWindow {
                window,
                origin: (f64::from(x), f64::from(y)),
            })
        }
    });
    let _ = register_action(
        cx,
        "overlay-language",
        "Pick a language in the flow bar list, like clicking its option. Args: \
         {\"code\": \"es\"} or {\"code\": \"auto\"}. The list has about a hundred options, so \
         this saves scrolling it.",
        move |cx, args| {
            let code = args
                .get("code")
                .and_then(Value::as_str)
                .ok_or("needs args like {\"code\": \"es\"}")?
                .to_owned();
            bar.update(cx, |bar, cx| bar.choose_language(&code, cx))?;
            Ok(Value::Null)
        },
    );
}
