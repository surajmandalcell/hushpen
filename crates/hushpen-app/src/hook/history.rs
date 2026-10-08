//! Test hook for the History view: the `history` state section, and the actions that do what
//! the view's controls do. Also the `frames` section, which reports how long the UI thread
//! stalled. Compiled only with the `test-automation` feature.

use super::{register_action, set_state_section};
use crate::history::History;
use gpui_kit::{AnyWindowHandle, App, Entity};
use hushpen_core::export::Format;
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
    type Run =
        fn(&mut History, Option<String>, &mut gpui_kit::Context<History>) -> Result<(), String>;
    let action = |cx: &mut App, name: &str, description: &str, run: Run| {
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
        "history-play",
        "Play the audio of a transcript, like Play. Args: {\"id\": \"<row id>\"}; with no id it \
         uses the open detail view. The position is in `hookctl state` section `history`, key \
         `playback`.",
        |history, id, cx| history.play(id.as_deref(), cx),
    );
    action(
        cx,
        "history-pause",
        "Pause the audio of a transcript, like Pause. Args as history-play.",
        |history, id, cx| history.pause(id.as_deref(), cx),
    );
    let _ = register_action(
        cx,
        "history-seek",
        "Move the audio of a transcript like a click on the seek bar. Args: {\"fraction\": 0.5} \
         or {\"ms\": 1500}, and optionally {\"id\": \"<row id>\"}.",
        {
            let history = history.clone();
            move |cx, args| {
                let id = id_of(&args);
                history.update(cx, |history, cx| {
                    if let Some(ms) = args.get("ms").and_then(Value::as_u64) {
                        history.seek_ms(id.as_deref(), ms, cx)
                    } else if let Some(fraction) = args.get("fraction").and_then(Value::as_f64) {
                        history.seek_fraction(id.as_deref(), fraction as f32, cx)
                    } else {
                        Err("needs args like {\"fraction\": 0.5} or {\"ms\": 1500}".to_owned())
                    }
                })?;
                Ok(Value::Null)
            }
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
    let _ = register_action(
        cx,
        "history-select",
        "Choose transcripts for an export, like clicking rows in Select mode. Args: \
         {\"ids\": [\"<row id>\", ...]} makes them the whole selection; {\"id\": \"<row id>\"} \
         toggles one; {\"all\": true} toggles every loaded row; {\"off\": true} leaves Select \
         mode. The selection is in `hookctl state` section `history`, key `selection`.",
        {
            let history = history.clone();
            move |cx, args| {
                history.update(cx, |history, cx| {
                    if args.get("off").and_then(Value::as_bool) == Some(true) {
                        history.set_selecting(false, cx);
                    } else if args.get("all").and_then(Value::as_bool) == Some(true) {
                        history.toggle_select_all(cx);
                    } else if let Some(ids) = args.get("ids").and_then(Value::as_array) {
                        let ids = ids.iter().filter_map(Value::as_str).map(str::to_owned);
                        history.select_only(ids.collect(), cx);
                    } else if let Some(id) = id_of(&args) {
                        history.toggle_selected(&id, cx);
                    } else {
                        history.set_selecting(true, cx);
                    }
                });
                Ok(Value::Null)
            }
        },
    );
    let _ = register_action(
        cx,
        "history-export",
        "Export transcripts to one file, like a format button. Args: {\"format\": \"txt\" | \
         \"srt\" | \"vtt\" | \"json\"} and optionally {\"ids\": [...]}; with no ids it uses the \
         selection, then the open detail view. The save dialog asks for the path unless \
         `hookctl paths <file>` queued one. The result is in `hookctl state` section \
         `history`, keys `export`, `notice`, and `message`.",
        {
            let history = history.clone();
            move |cx, args| {
                let format = args
                    .get("format")
                    .and_then(Value::as_str)
                    .and_then(Format::from_key)
                    .ok_or("needs args like {\"format\": \"srt\"} (txt, srt, vtt, or json)")?;
                let ids = args.get("ids").and_then(Value::as_array).map(|ids| {
                    ids.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                });
                history.update(cx, |history, cx| history.export(format, ids, cx))?;
                Ok(Value::Null)
            }
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
