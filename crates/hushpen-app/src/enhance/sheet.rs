//! The audio enhance sheet, drawn over the window like the model details sheet. The tuning page
//! has the kept sample with its record and play buttons, the Custom values, and the way to the
//! search; the search page has the reading, its progress, and the five best settings. Tab stays
//! inside the sheet, Esc and Done close it, and focus goes back to the control that opened it.

use super::{Find, Page, Playing, SAMPLE_SECONDS, Take, Tuning, signed_db};
use crate::theme::{
    self, BODY, Palette, SECONDARY, StyledType, TOOLBAR_TITLE, radius, size, space,
};
use crate::ui::{self, Kind, Notice};
use crate::{hook, motion};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::base::FocusTrapElement as _;
use gpui_kit::{
    AnyElement, App, Context, Entity, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    Role, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

fn id(name: &str) -> SharedString {
    hook::id("enhance", name)
}

/// The sheet, or nothing while it is closed. It comes in like every sheet: the scrim fades in,
/// and the sheet fades in and rises a few pixels into place.
pub fn render(tuning: &Entity<Tuning>, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let page = tuning.read(cx).page()?;
    let arrival = motion::enter(id("sheet.arrival"), motion::BASE, window, cx);
    let palette = theme::palette(cx);
    let close = {
        let tuning = tuning.clone();
        move |window: &mut Window, cx: &mut App| {
            tuning.update(cx, |tuning, cx| tuning.close(window, cx));
        }
    };
    let on_key = {
        let close = close.clone();
        move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            if event.keystroke.key.as_str() == "escape" {
                close(window, cx);
                cx.stop_propagation();
            }
        }
    };
    let me = tuning.read(cx);
    let (title, body) = match page {
        Page::Tune => ("Audio enhance", tune_page(palette, tuning, me, cx)),
        Page::Find => ("Find best settings", find_page(palette, tuning, me, cx)),
    };
    let sheet = div()
        .id(id("sheet"))
        .test_support()
        .role(Role::Dialog)
        .aria_label(title)
        .on_key_down(on_key)
        .flex()
        .flex_col()
        .gap(px(space::LG))
        .w(px(space::SHEET_WIDTH))
        .p(px(space::XL))
        .bg(palette.content)
        .rounded(px(radius::WINDOW))
        .border(theme::hairline())
        .border_color(palette.separator)
        .children(body)
        .child(footer(palette, tuning, page, me, close));
    let trap = me.focus.trap.clone();
    Some(
        div()
            .focus_trap(id("sheet.scrim"), &trap)
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::faded(palette.scrim, arrival))
            .child(motion::arrive(sheet, arrival, motion::TRAVEL))
            .into_any_element(),
    )
}

/// The title of the page and one line under it.
fn header(palette: Palette, title: &'static str, line: &'static str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(space::XS))
        .child(
            div()
                .id(id("sheet.title"))
                .test_support()
                .role(Role::Heading)
                .aria_label(title)
                .text_token(TOOLBAR_TITLE)
                .text_color(palette.label)
                .truncate()
                .child(title),
        )
        .child(
            div()
                .id(id("sheet.line"))
                .test_support()
                .aria_label(line)
                .text_token(SECONDARY)
                .text_color(palette.label_secondary)
                .truncate()
                .child(line),
        )
        .into_any_element()
}

/// Runs `act` on the tuning when a control is pressed.
fn act(
    tuning: &Entity<Tuning>,
    act: impl Fn(&mut Tuning, &mut Context<Tuning>) + 'static,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let tuning = tuning.clone();
    move |_, cx| tuning.update(cx, |me, cx| act(me, cx))
}

fn tune_page(palette: Palette, tuning: &Entity<Tuning>, me: &Tuning, cx: &App) -> Vec<AnyElement> {
    let focus = &me.focus;
    let recording = matches!(me.take(), Take::Recording { find: false, .. });
    let busy = me.take() != Take::Idle;
    let detail = match (me.take(), me.sample_seconds()) {
        (
            Take::Recording {
                find: false,
                seconds,
            },
            _,
        ) => {
            format!(
                "Speak now. {} s left",
                SAMPLE_SECONDS.saturating_sub(seconds)
            )
        }
        (_, Some(seconds)) => format!("{seconds} s, plays with the values below"),
        (_, None) => format!("Record {SAMPLE_SECONDS} s of speech to hear the values"),
    };
    let record = ui::icon_button(
        palette,
        id("record"),
        if recording {
            IconName::Square
        } else {
            IconName::Mic
        },
        if recording {
            "Stop recording"
        } else {
            "Record a sample"
        }
        .to_owned(),
        if recording { "Stop" } else { "Record" },
        &focus.record,
        act(tuning, |me, cx| me.toggle_take(false, cx)),
    );
    let playing = me.playing() == Some(Playing::Sample);
    let play = (me.sample_seconds().is_some() && !busy).then(|| {
        ui::icon_button(
            palette,
            id("play"),
            if playing {
                IconName::Square
            } else {
                IconName::Play
            },
            if playing {
                "Stop the sample"
            } else {
                "Play the sample"
            }
            .to_owned(),
            if playing { "Stop" } else { "Play" },
            &focus.play,
            act(tuning, |me, cx| me.toggle_play(Playing::Sample, cx)),
        )
    });
    let sample = ui::row(palette)
        .id(id("sample.row"))
        .test_support()
        .child(ui::named_lines(
            palette,
            id("sample.title"),
            "Sample",
            Some((id("sample.detail"), detail)),
        ))
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(space::XS))
                .child(record)
                .children(play),
        )
        .into_any_element();
    let mut sample_rows = vec![sample];
    if recording {
        sample_rows.push(ui::meter_row(palette, "enhance", &me.meter(cx), true));
    }
    if let Some(notice) = me.notice() {
        sample_rows.push(notice_row(palette, notice.to_owned(), Notice::Alert));
    }
    let values = me.custom();
    let slider = |key: &str,
                  label: &'static str,
                  detail: &str,
                  position: f32,
                  reading: String,
                  focus,
                  set: fn(&mut Tuning, f32, &mut Context<Tuning>),
                  value_of: fn(f32) -> f32| {
        let tuning = tuning.clone();
        ui::slider_row(
            palette,
            detail,
            ui::Slider {
                id: id(key),
                label,
                position,
                reading,
                focus,
            },
            move |position, cx| tuning.update(cx, |me, cx| set(me, value_of(position), cx)),
        )
    };
    let low_cut = slider(
        "lowCut",
        "Low cut",
        "Cuts rumble below this pitch.",
        f32::from(values.low_cut_hz) / 200.0,
        hz_text(values.low_cut_hz),
        &focus.low_cut,
        Tuning::set_low_cut,
        |position| position * 200.0,
    );
    let bass = slider(
        "bass",
        "Bass",
        "Lower it to take out boom.",
        (values.bass_db + 12.0) / 18.0,
        db_text(values.bass_db),
        &focus.bass,
        Tuning::set_bass,
        |position| position * 18.0 - 12.0,
    );
    let treble = slider(
        "treble",
        "Treble",
        "Raise it for crisper speech.",
        (values.treble_db + 6.0) / 18.0,
        db_text(values.treble_db),
        &focus.treble,
        Tuning::set_treble,
        |position| position * 18.0 - 6.0,
    );
    let noise = {
        let tuning = tuning.clone();
        let on = values.noise_reduction;
        ui::toggle_row(
            palette,
            id("noiseReduction"),
            "Noise reduction",
            "Can make the text worse. Use it only in a noisy room.",
            on,
            &focus.noise,
            move |cx| tuning.update(cx, |me, cx| me.set_noise_reduction(!on, cx)),
        )
    };
    vec![
        header(
            palette,
            "Audio enhance",
            "Shapes the microphone so the transcript gets more words right.",
        ),
        ui::group(id("sample"), palette, sample_rows).into_any_element(),
        ui::group(id("values"), palette, vec![low_cut, bass, treble, noise]).into_any_element(),
    ]
}

/// `Off`, `80 Hz`.
fn hz_text(hz: u16) -> String {
    if hz == 0 {
        "Off".to_owned()
    } else {
        format!("{hz} Hz")
    }
}

/// `0 dB`, `-6 dB`, `+3 dB`.
fn db_text(db: f32) -> String {
    if db == 0.0 {
        "0 dB".to_owned()
    } else {
        signed_db(db)
    }
}

fn notice_row(palette: Palette, text: String, notice: Notice) -> AnyElement {
    ui::notice_row(palette, id("notice"), text, notice, None)
}

fn find_page(palette: Palette, tuning: &Entity<Tuning>, me: &Tuning, cx: &App) -> Vec<AnyElement> {
    let focus = &me.focus;
    let mut page = vec![header(
        palette,
        "Find best settings",
        "Read the lines aloud. The test then tries many settings, which takes a few minutes.",
    )];
    let reading = matches!(me.take(), Take::Recording { find: true, .. });
    match me.find() {
        Find::Reading | Find::Failed(_) => {
            let lines = me.reading().into_iter().enumerate().map(|(index, line)| {
                div()
                    .id(hook::indexed("enhance", "reading.line", index))
                    .test_support()
                    .aria_label(line.clone())
                    .text_token(BODY)
                    .text_color(palette.label)
                    .child(line)
            });
            let text = div()
                .id(id("reading"))
                .test_support()
                .flex()
                .flex_col()
                .gap(px(space::XS))
                .px(px(space::XL))
                .py(px(space::LG))
                .children(lines)
                .into_any_element();
            let detail = match me.take() {
                Take::Recording {
                    find: true,
                    seconds,
                } => {
                    format!("Recording, {seconds} s. Press Stop when you are done.")
                }
                _ => "Press Record, then read the lines in your normal voice.".to_owned(),
            };
            let button = ui::button(
                palette,
                id("reading.record"),
                if reading { "Stop" } else { "Record" },
                Kind::Default,
                Some(&focus.record),
                me.take() == Take::Idle || reading,
                act(tuning, |me, cx| me.toggle_take(true, cx)),
            );
            let mut rows = vec![
                text,
                ui::labeled_row(
                    palette,
                    id("reading.row"),
                    "Your reading",
                    Some(detail),
                    button,
                ),
            ];
            if reading {
                rows.push(ui::meter_row(palette, "enhance", &me.meter(cx), true));
            }
            if let Find::Failed(message) = me.find() {
                rows.push(notice_row(palette, message.clone(), Notice::Failure));
            } else if let Some(notice) = me.notice() {
                rows.push(notice_row(palette, notice.to_owned(), Notice::Alert));
            }
            page.push(ui::group(id("reading.group"), palette, rows).into_any_element());
        }
        Find::Testing { done, about } => {
            let progress = if *about == 0 {
                0.0
            } else {
                *done as f32 / *about as f32
            };
            let trailing = div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(space::MD))
                .child(
                    div()
                        .id(id("testing.progress"))
                        .test_support()
                        .w(px(size::QUEUE_PROGRESS_WIDTH))
                        .child(ui::track(palette, progress)),
                )
                .child(ui::button(
                    palette,
                    id("testing.cancel"),
                    "Cancel",
                    Kind::Plain,
                    Some(&focus.cancel),
                    true,
                    act(tuning, |me, cx| me.cancel_search(cx)),
                ));
            let detail = if *done == 0 {
                "Starting".to_owned()
            } else {
                format!("Testing {done} of about {about}")
            };
            page.push(
                ui::group(
                    id("testing.group"),
                    palette,
                    vec![ui::labeled_row(
                        palette,
                        id("testing.row"),
                        "Testing settings",
                        Some(detail),
                        trailing,
                    )],
                )
                .into_any_element(),
            );
        }
        Find::Results(outcome) => {
            let rows = if outcome.all_same() {
                vec![notice_row(
                    palette,
                    "Every setting got the same words right, so Auto stays.".to_owned(),
                    Notice::Info,
                )]
            } else {
                let gain = me.mic.read(cx).gain();
                outcome
                    .top()
                    .iter()
                    .enumerate()
                    .map(|(index, scored)| result_row(palette, tuning, me, index, scored, gain))
                    .collect()
            };
            page.push(ui::group(id("results"), palette, rows).into_any_element());
        }
    }
    page
}

fn result_row(
    palette: Palette,
    tuning: &Entity<Tuning>,
    me: &Tuning,
    index: usize,
    scored: &super::Scored,
    gain: f32,
) -> AnyElement {
    let words = super::score::words(&me.reading().join(" ")).len();
    let errors = match scored.errors {
        0 => format!("All {words} words right"),
        wrong => format!("{wrong} of {words} words wrong"),
    };
    let playing = me.playing() == Some(Playing::Result(index));
    let play = ui::icon_button(
        palette,
        hook::indexed("enhance", "result.play", index),
        if playing {
            IconName::Square
        } else {
            IconName::Play
        },
        if playing {
            "Stop"
        } else {
            "Play the reading with this setting"
        }
        .to_owned(),
        if playing { "Stop" } else { "Play" },
        &me.focus.result_play[index],
        act(tuning, move |me, cx| {
            me.toggle_play(Playing::Result(index), cx)
        }),
    );
    let use_it = ui::button(
        palette,
        hook::indexed("enhance", "result.use", index),
        "Use",
        Kind::Plain,
        Some(&me.focus.result_use[index]),
        true,
        {
            let tuning = tuning.clone();
            move |window: &mut Window, cx: &mut App| {
                tuning.update(cx, |me, cx| {
                    if me.use_result(index, cx).is_ok() {
                        window.focus(&me.focus.done, cx);
                    }
                })
            }
        },
    );
    ui::row(palette)
        .id(hook::indexed("enhance", "result", index))
        .test_support()
        .child(ui::named_lines(
            palette,
            hook::indexed("enhance", "result.title", index),
            scored.candidate.describe(gain),
            Some((hook::indexed("enhance", "result.detail", index), errors)),
        ))
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(space::SM))
                .child(play)
                .child(use_it),
        )
        .into_any_element()
}

/// The buttons under the page, lightest to heaviest, with Done last.
fn footer(
    palette: Palette,
    tuning: &Entity<Tuning>,
    page: Page,
    me: &Tuning,
    close: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let focus = &me.focus;
    let idle = me.take() == Take::Idle;
    let mut buttons: Vec<AnyElement> = Vec::new();
    match page {
        Page::Tune => {
            buttons.push(
                ui::button(
                    palette,
                    id("reset"),
                    "Reset to Auto",
                    Kind::Plain,
                    Some(&focus.reset),
                    true,
                    act(tuning, |me, cx| me.reset_to_auto(cx)),
                )
                .into_any_element(),
            );
            let open_find = {
                let tuning = tuning.clone();
                move |window: &mut Window, cx: &mut App| {
                    tuning.update(cx, |me, cx| me.open(Page::Find, None, window, cx))
                }
            };
            buttons.push(
                ui::button(
                    palette,
                    id("find"),
                    "Find best settings…",
                    Kind::Plain,
                    Some(&focus.find),
                    idle,
                    open_find,
                )
                .into_any_element(),
            );
        }
        Page::Find => {
            let back = {
                let tuning = tuning.clone();
                move |window: &mut Window, cx: &mut App| {
                    tuning.update(cx, |me, cx| {
                        me.cancel_search(cx);
                        me.open(Page::Tune, None, window, cx);
                    })
                }
            };
            buttons.push(
                ui::button(
                    palette,
                    id("back"),
                    "Back",
                    Kind::Plain,
                    Some(&focus.back),
                    idle,
                    back,
                )
                .into_any_element(),
            );
            if matches!(me.find(), Find::Results(_)) {
                buttons.push(
                    ui::button(
                        palette,
                        id("again"),
                        "Test again",
                        Kind::Plain,
                        Some(&focus.find),
                        true,
                        act(tuning, |me, cx| me.show(Page::Find, cx)),
                    )
                    .into_any_element(),
                );
            }
        }
    }
    div()
        .flex()
        .items_center()
        .justify_end()
        .gap(px(space::SM))
        .children(buttons)
        .child(ui::button(
            palette,
            id("done"),
            "Done",
            Kind::Default,
            Some(&focus.done),
            true,
            close,
        ))
        .into_any_element()
}
