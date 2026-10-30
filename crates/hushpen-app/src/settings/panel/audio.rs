//! Audio: the microphone and its boost, the cue sounds and their volume, and the longest a
//! dictation runs.

use super::Page;
use crate::settings::{Settings, slots};
use crate::ui;
use gpui_kit::{AnyElement, App};

/// The lengths offered for a dictation, in minutes, inside the range the store accepts.
const MINUTE_CHOICES: [i64; 7] = [2, 4, 6, 10, 15, 30, 60];

pub fn microphone(page: &Page, cx: &mut App) -> AnyElement {
    let (current, choices, focus) = {
        let mic = page.settings.read(cx).mic.read(cx);
        (mic.saved_label(), mic.choices(), mic.row_focus.clone())
    };
    let menu = page
        .picker_open("audio.mic", cx)
        .then(|| page.options("audio.mic", choices, &focus, select_mic));
    page.menu(
        "audio.mic",
        "Microphone",
        "The microphone dictation listens on.",
        current,
        slots::MIC,
        menu,
    )
}

fn select_mic(me: &mut Settings, id: &str, cx: &mut gpui_kit::Context<Settings>) {
    let _ = me.mic.update(cx, |mic, cx| mic.select(id, cx));
    me.picker = None;
    cx.notify();
}

/// The boost slider runs from 100% at the left end to 200% at the right.
pub fn input_gain(page: &Page, cx: &mut App) -> AnyElement {
    let gain = page
        .settings
        .read(cx)
        .stored_f64(crate::mic::GAIN_SETTING)
        .unwrap_or(1.0) as f32;
    let settings = page.settings.clone();
    ui::slider_row(
        page.palette,
        "Makes a quiet microphone louder.",
        ui::Slider {
            id: super::settings_id(crate::mic::GAIN_SETTING),
            label: "Input boost",
            position: gain - 1.0,
            reading: percent(gain),
            focus: &page.focus[slots::INPUT_GAIN],
        },
        move |position, cx| settings.update(cx, |me, cx| me.set_input_gain(1.0 + position, cx)),
    )
}

pub fn sounds(page: &Page, cx: &mut App) -> AnyElement {
    let on = page.settings.read(cx).stored_bool("audio.cueSounds");
    page.toggle(
        "audio.sounds",
        "Sounds",
        "A short cue marks the start and end of a dictation.",
        on,
        slots::SOUNDS,
        Settings::toggle_sounds,
    )
}

pub fn cue_volume(page: &Page, cx: &mut App) -> AnyElement {
    let volume = page
        .settings
        .read(cx)
        .stored_f64("audio.cueVolume")
        .unwrap_or_default();
    let settings = page.settings.clone();
    let volume = volume as f32;
    ui::slider_row(
        page.palette,
        "How loud the cues play.",
        ui::Slider {
            id: super::settings_id("audio.cueVolume"),
            label: "Sound volume",
            position: volume,
            reading: percent(volume),
            focus: &page.focus[slots::CUE_VOLUME],
        },
        move |volume, cx| settings.update(cx, |me, cx| me.set_cue_volume(volume, cx)),
    )
}

/// `1.5` reads `150%`.
fn percent(fraction: f32) -> String {
    format!("{:.0}%", fraction * 100.0)
}

pub fn max_length(page: &Page, cx: &mut App) -> AnyElement {
    let stored = page
        .settings
        .read(cx)
        .stored_f64("dictation.maxMinutes")
        .map_or(0, |minutes| minutes as i64);
    let range = hushpen_store::settings::range("dictation.maxMinutes");
    let mut minutes: Vec<i64> = MINUTE_CHOICES
        .into_iter()
        .chain([stored])
        .filter(|choice| {
            range
                .as_ref()
                .is_some_and(|range| range.contains(&(*choice as f64)))
        })
        .collect();
    minutes.sort_unstable();
    minutes.dedup();
    let menu = page.picker_open("audio.maxMinutes", cx).then(|| {
        let choices = minutes
            .into_iter()
            .map(|choice| (choice.to_string(), minutes_label(choice), choice == stored))
            .collect();
        page.options(
            "audio.maxMinutes",
            choices,
            &page.options,
            |me, value, cx| {
                if let Ok(minutes) = value.parse() {
                    me.set_max_minutes(minutes, cx);
                }
            },
        )
    });
    page.menu(
        "audio.maxMinutes",
        "Maximum length",
        "A dictation stops by itself after this time.",
        minutes_label(stored),
        slots::MAX_MINUTES,
        menu,
    )
}

fn minutes_label(minutes: i64) -> String {
    format!("{minutes} minutes")
}
