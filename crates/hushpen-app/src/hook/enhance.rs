//! Test hook for audio enhance: the `enhance` state section and the actions that set the mode
//! and the values, open the sheet, record or load the kept sample, play it, and run "Find best
//! settings" on a WAV with a known text. Compiled only with the `test-automation` feature.

use super::{register_action, set_state_section};
use crate::enhance::{Mode, Page, Playing, Tuning};
use gpui_kit::{AnyWindowHandle, App, Entity};
use serde_json::{Value, json};
use std::path::Path;

pub fn attach(cx: &mut App, tuning: Entity<Tuning>, window: AnyWindowHandle) {
    set_state_section(cx, "enhance", {
        let tuning = tuning.clone();
        move |cx| tuning.read(cx).hook_state_json(cx)
    });
    let _ = register_action(
        cx,
        "enhance-set",
        "Set audio enhance like the Settings row and the sheet do. Args (each optional): \
         {\"mode\": \"off\"|\"auto\"|\"custom\", \"lowCutHz\": 80, \"bassDb\": -6, \"trebleDb\": 0, \
         \"noiseReduction\": false}. A value makes the mode Custom.",
        {
            let tuning = tuning.clone();
            move |cx, args| {
                let mode = match args.get("mode").and_then(Value::as_str) {
                    Some(key) => Some(Mode::from_key(key).ok_or("mode is off, auto, or custom")?),
                    None => None,
                };
                let number = |key| args.get(key).and_then(Value::as_f64).map(|v| v as f32);
                tuning.update(cx, |me, cx| {
                    if let Some(hz) = number("lowCutHz") {
                        me.set_low_cut(hz, cx);
                    }
                    if let Some(db) = number("bassDb") {
                        me.set_bass(db, cx);
                    }
                    if let Some(db) = number("trebleDb") {
                        me.set_treble(db, cx);
                    }
                    if let Some(on) = args.get("noiseReduction").and_then(Value::as_bool) {
                        me.set_noise_reduction(on, cx);
                    }
                    if let Some(mode) = mode {
                        me.set_mode(mode, cx);
                    }
                });
                Ok(tuning.read(cx).hook_state_json(cx))
            }
        },
    );
    let _ = register_action(
        cx,
        "enhance-sheet",
        "Open the audio enhance sheet like the Adjust button, or close it. Args: \
         {\"page\": \"tune\"|\"find\"} to open, {\"page\": null} to close.",
        {
            let tuning = tuning.clone();
            move |cx, args| {
                let page = match args.get("page").and_then(Value::as_str) {
                    Some("tune") => Some(Page::Tune),
                    Some("find") => Some(Page::Find),
                    Some(_) => return Err("page is tune, find, or null".into()),
                    None => None,
                };
                window
                    .update(cx, |_, window, cx| {
                        tuning.update(cx, |me, cx| match page {
                            Some(page) => me.open(page, None, window, cx),
                            None => me.close(window, cx),
                        })
                    })
                    .map_err(|error| error.to_string())?;
                Ok(Value::Null)
            }
        },
    );
    let _ = register_action(
        cx,
        "enhance-take",
        "Start or stop a take like the record button. Args: {\"find\": false} for the 8 s sample \
         (default), {\"find\": true} for the reading of the search, which starts the search when \
         it stops.",
        {
            let tuning = tuning.clone();
            move |cx, args| {
                let find = args.get("find").and_then(Value::as_bool).unwrap_or(false);
                tuning.update(cx, |me, cx| me.toggle_take(find, cx));
                Ok(json!({"take": tuning.read(cx).hook_state_json(cx)["take"]}))
            }
        },
    );
    let _ = register_action(
        cx,
        "enhance-load-sample",
        "Make an audio file the kept sample, as if the microphone recorded it. Args: \
         {\"path\": \"/abs/take.wav\"}.",
        {
            let tuning = tuning.clone();
            move |cx, args| {
                let path = args
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or("needs args like {\"path\": \"/abs/take.wav\"}")?;
                tuning.update(cx, |me, cx| me.load_sample(Path::new(path), cx))?;
                Ok(tuning.read(cx).hook_state_json(cx)["sample"].clone())
            }
        },
    );
    let _ = register_action(
        cx,
        "enhance-play",
        "Play the kept sample through the current setting, or through result n of the search, \
         like its play button; a second call stops it. Args: {} or {\"result\": 0}.",
        {
            let tuning = tuning.clone();
            move |cx, args| {
                let what = match args.get("result").and_then(Value::as_u64) {
                    Some(index) => Playing::Result(index as usize),
                    None => Playing::Sample,
                };
                tuning.update(cx, |me, cx| me.toggle_play(what, cx));
                Ok(json!({"playing": tuning.read(cx).hook_state_json(cx)["playing"]}))
            }
        },
    );
    let _ = register_action(
        cx,
        "enhance-find",
        "Run \"Find best settings\": transcribe the kept sample (or {\"path\"} loaded first) with \
         many settings and score each against {\"text\"} (default: the reading the sheet shows). \
         Read the progress and the results in the enhance state section. Args: \
         {\"path\": \"/abs/take.wav\", \"text\": \"...\"}, both optional; {\"cancel\": true} stops it.",
        {
            let tuning = tuning.clone();
            move |cx, args| {
                if args.get("cancel").and_then(Value::as_bool) == Some(true) {
                    tuning.update(cx, |me, cx| me.cancel_search(cx));
                    return Ok(Value::Null);
                }
                if let Some(path) = args.get("path").and_then(Value::as_str) {
                    tuning.update(cx, |me, cx| me.load_sample(Path::new(path), cx))?;
                }
                let text = args.get("text").and_then(Value::as_str).map(str::to_owned);
                tuning.update(cx, |me, cx| {
                    me.show(Page::Find, cx);
                    me.start_search(text, cx);
                });
                Ok(tuning.read(cx).hook_state_json(cx)["find"].clone())
            }
        },
    );
    let _ = register_action(
        cx,
        "enhance-use",
        "Use result n of the search like its Use button: Custom with its values and its boost. \
         Args: {\"result\": 0}.",
        move |cx, args| {
            let index = args
                .get("result")
                .and_then(Value::as_u64)
                .ok_or("needs args like {\"result\": 0}")?;
            tuning.update(cx, |me, cx| me.use_result(index as usize, cx))?;
            Ok(tuning.read(cx).hook_state_json(cx))
        },
    );
}
