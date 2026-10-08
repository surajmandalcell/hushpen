//! Test hook for the History view: the `history` state section, and the actions that do what
//! the view's controls do. Also the `frames` section, which reports how long the UI thread
//! stalled. Compiled only with the `test-automation` feature.

use super::{register_action, set_state_section};
use crate::history::History;
use gpui_kit::{AnyWindowHandle, App, Entity};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// How often the probe asks the UI thread to wake it.
const PROBE_STEP: Duration = Duration::from_millis(16);

fn id_of(args: &Value) -> Option<String> {
    args.get("id").and_then(Value::as_str).map(str::to_owned)
}

pub fn attach(cx: &mut App, history: Entity<History>, window: AnyWindowHandle) {
    set_state_section(cx, "history", {
        let history = history.clone();
        move |cx| history.read(cx).state_json(cx)
    });
    let action = |cx: &mut App,
                  name: &str,
                  description: &str,
                  run: fn(
        &mut History,
        Option<String>,
        &mut gpui_kit::Context<History>,
    ) -> Result<(), String>| {
        let history = history.clone();
        let _ = register_action(cx, name, description, move |cx, args| {
            history.update(cx, |history, cx| run(history, id_of(&args), cx))?;
            Ok(Value::Null)
        });
    };
    let _ = register_action(
        cx,
        "history-search",
        "Type a query into the History search box. Args: {\"query\": \"cafe\"}. An empty query \
         shows the first page again.",
        {
            let history = history.clone();
            move |cx, args| {
                let query = args.get("query").and_then(Value::as_str).unwrap_or("");
                window
                    .update(cx, |_, window, cx| {
                        history.update(cx, |history, cx| history.search_for(query, window, cx));
                    })
                    .map_err(|error| error.to_string())?;
                Ok(Value::Null)
            }
        },
    );
    action(
        cx,
        "history-open",
        "Open the detail view of a transcript. Args: {\"id\": \"<row id>\"}. The ids are in \
         `hookctl state` section `history`.",
        |history, id, cx| history.open(&id.ok_or("needs args like {\"id\": \"<row id>\"}")?, cx),
    );
    action(
        cx,
        "history-close",
        "Leave the detail view, like Back.",
        |history, _, cx| {
            history.close(cx);
            Ok(())
        },
    );
    action(
        cx,
        "history-more",
        "Load the next page, like Show more.",
        |history, _, cx| {
            history.load_more(cx);
            Ok(())
        },
    );
    action(
        cx,
        "history-copy",
        "Copy the text of a transcript to the clipboard, like Copy. Args: {\"id\": \"<row id>\"}; \
         with no id it uses the open detail view.",
        |history, id, cx| history.copy(id.as_deref(), cx),
    );
    action(
        cx,
        "history-repaste",
        "Paste the text of a transcript into the focused app, like Re-paste. Args as history-copy.",
        |history, id, cx| history.repaste(id.as_deref(), cx),
    );
    action(
        cx,
        "history-reprocess",
        "Run the engine again on the audio of a transcript with the current model and \
         dictionary, like Reprocess. Args as history-copy. Progress is in `hookctl state` \
         section `history`, key `reprocess`.",
        |history, id, cx| history.reprocess(id.as_deref(), cx),
    );
    action(
        cx,
        "history-delete",
        "Delete a transcript and start the undo window, like Delete. Args as history-copy.",
        |history, id, cx| history.delete(id.as_deref(), cx),
    );
    action(
        cx,
        "history-undo",
        "Bring back the last deleted transcript, like Undo.",
        |history, _, cx| history.undo(cx),
    );
    action(
        cx,
        "history-clear",
        "Ask to clear the whole history, like Clear all. Nothing is removed until \
         history-clear-confirm.",
        |history, _, cx| history.ask_clear(cx),
    );
    action(
        cx,
        "history-clear-confirm",
        "Confirm Clear all: removes every transcript and its audio.",
        |history, _, cx| history.confirm_clear(cx),
    );
    action(
        cx,
        "history-clear-cancel",
        "Decline Clear all, like Keep.",
        |history, _, cx| {
            history.cancel_clear(cx);
            Ok(())
        },
    );
    frames(cx);
}

/// The longest wait of a 16 ms timer on the UI thread since the last reset. A stall of the UI
/// thread shows as a long wait.
#[derive(Default)]
struct Stalls {
    samples: u64,
    max_ms: f64,
}

fn frames(cx: &mut App) {
    let stalls = Rc::new(RefCell::new(Stalls::default()));
    cx.spawn({
        let stalls = Rc::clone(&stalls);
        async move |cx| {
            loop {
                let started = Instant::now();
                cx.background_executor().timer(PROBE_STEP).await;
                let waited = started.elapsed().as_secs_f64() * 1000.0;
                let mut stalls = stalls.borrow_mut();
                stalls.samples += 1;
                stalls.max_ms = stalls.max_ms.max(waited);
            }
        }
    })
    .detach();
    set_state_section(cx, "frames", {
        let stalls = Rc::clone(&stalls);
        move |_| {
            let stalls = stalls.borrow();
            json!({
                "samples": stalls.samples,
                "max_gap_ms": (stalls.max_ms * 10.0).round() / 10.0,
            })
        }
    });
    let _ = register_action(
        cx,
        "frames-reset",
        "Start the UI stall measurement again. `hookctl state` section `frames` then reports \
         the longest gap between two ticks of a 16 ms timer on the UI thread.",
        move |_, _| {
            *stalls.borrow_mut() = Stalls::default();
            Ok(Value::Null)
        },
    );
}
