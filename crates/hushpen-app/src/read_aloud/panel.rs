//! The Read aloud side of the Files view: the text, the voice options, and Play and Export.

use super::{EngineKind, Menu, ReadAloud, language_name};
use crate::hook;
use crate::theme::{self, Palette, radius, size, space};
use crate::ui::{self, Kind, Notice};
use gpui_kit::TestSupportExt as _;
use gpui_kit::component::input::Textarea;
use gpui_kit::{
    AnyElement, App, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use hushpen_speech::{MAX_SPEED, MIN_SPEED, VoiceQuality};

pub fn render(read: &Entity<ReadAloud>, cx: &mut App) -> impl IntoElement + use<> {
    let palette = theme::palette(cx);
    let mut page = div()
        .flex()
        .flex_col()
        .gap(px(space::XL))
        .child(ui::titled_section(
            palette,
            hook::id("read", "text.heading"),
            "Text",
            text_group(palette, read, cx),
            Some((
                hook::id("read", "text.footer"),
                "Plain text or Markdown, up to 1 MB. Speech starts after the first sentence."
                    .to_owned(),
            )),
        ))
        .child(ui::titled_section(
            palette,
            hook::id("read", "voice.heading"),
            "Voice",
            ui::group(
                hook::id("read", "voice"),
                palette,
                voice_rows(palette, read, cx),
            ),
            None,
        ));
    let notices: Vec<AnyElement> = {
        let view = read.read(cx);
        [
            view.error()
                .map(|text| ("error", text.to_owned(), Notice::Failure)),
            view.info()
                .map(|text| ("info", text.to_owned(), Notice::Info)),
        ]
        .into_iter()
        .flatten()
        .map(|(name, text, notice)| {
            ui::notice_row(palette, hook::id("read", name), text, notice, None)
        })
        .collect()
    };
    if !notices.is_empty() {
        page = page.child(ui::group(hook::id("read", "notices"), palette, notices));
    }
    page.child(ui::group(
        hook::id("read", "actions"),
        palette,
        vec![actions_row(palette, read, cx)],
    ))
}

/// The text area, then the size of the text and Open Text File.
fn text_group(palette: Palette, read: &Entity<ReadAloud>, cx: &App) -> impl IntoElement {
    let view = read.read(cx);
    let chars = view.text.read(cx).value().chars().count();
    let open = {
        let read = read.clone();
        move |_: &mut Window, cx: &mut App| {
            // A refusal is the picker being open already.
            let _ = read.update(cx, |read, cx| read.choose_text_file(cx));
        }
    };
    let area = div()
        .id(hook::id("read", "text"))
        .test_support()
        .p(px(space::XS))
        .child(
            Textarea::new(&view.text)
                .aria_label("Text to read aloud")
                .h(px(size::TEXT_AREA_HEIGHT))
                .bg(palette.control)
                .rounded(px(radius::BUTTON))
                .border_color(gpui_kit::transparent_black()),
        )
        .into_any_element();
    let count = if chars == 1 {
        "1 character".to_owned()
    } else {
        format!("{chars} characters")
    };
    let footer = ui::labeled_row(
        palette,
        hook::id("read", "file"),
        count,
        None,
        ui::button(
            palette,
            hook::id("read", "open"),
            "Open Text File\u{2026}",
            Kind::Plain,
            Some(&view.focus.open),
            !view.choosing(),
            open,
        ),
    );
    ui::group(hook::id("read", "text.group"), palette, vec![area, footer])
}

/// Engine, then Language and Voice from the engine's voices, then Speed and Format.
fn voice_rows(palette: Palette, read: &Entity<ReadAloud>, cx: &App) -> Vec<AnyElement> {
    let view = read.read(cx);
    let engine = view.engine();
    let toggle = |menu: Menu| {
        let read = read.clone();
        move |_: &mut Window, cx: &mut App| read.update(cx, |read, cx| read.toggle_menu(menu, cx))
    };
    let mut rows = vec![ui::labeled_row(
        palette,
        hook::id("read", "engine.row"),
        "Engine",
        Some(match engine {
            EngineKind::System => "The voices of this computer".to_owned(),
            EngineKind::Kokoro => "Natural voices from a downloaded model".to_owned(),
        }),
        ui::menu_button(
            palette,
            hook::id("read", "engine"),
            "Engine",
            engine.label().to_owned(),
            true,
            &view.focus.engine,
            toggle(Menu::Engine),
        ),
    )];
    let options = &view.focus.options;
    if view.menu() == Some(Menu::Engine) {
        let choices = EngineKind::ALL
            .into_iter()
            .map(|kind| {
                (
                    kind.key().to_owned(),
                    kind.label().to_owned(),
                    None,
                    kind == engine,
                )
            })
            .collect();
        rows.push(options_list(palette, read, Menu::Engine, choices, options));
    }
    if let Some(why) = view.unavailable() {
        let button = view.needs_model().then(|| {
            let read = read.clone();
            ui::button(
                palette,
                hook::id("read", "models"),
                "Download in Models",
                Kind::Plain,
                Some(&view.focus.models),
                true,
                move |_: &mut Window, cx: &mut App| {
                    read.update(cx, |read, cx| read.open_models(cx))
                },
            )
            .into_any_element()
        });
        rows.push(ui::notice_row(
            palette,
            hook::id("read", "unavailable"),
            why,
            Notice::Alert,
            button,
        ));
    }
    let ready = view.voices().is_some_and(Result::is_ok);
    let language = view.language();
    rows.push(ui::labeled_row(
        palette,
        hook::id("read", "language.row"),
        "Language",
        Some("Lists the voices that speak it".to_owned()),
        ui::menu_button(
            palette,
            hook::id("read", "language"),
            "Language",
            language
                .as_deref()
                .map_or_else(|| "None".to_owned(), language_name),
            ready && !view.languages().is_empty(),
            &view.focus.language,
            toggle(Menu::Language),
        ),
    ));
    if view.menu() == Some(Menu::Language) {
        let choices = view
            .languages()
            .into_iter()
            .map(|code| {
                let chosen = Some(&code) == language.as_ref();
                (code.clone(), language_name(&code), None, chosen)
            })
            .collect();
        rows.push(options_list(
            palette,
            read,
            Menu::Language,
            choices,
            options,
        ));
    }
    let voices = view.language_voices();
    let voice = view.voice();
    let current = voice
        .as_ref()
        .and_then(|id| voices.iter().find(|found| &found.id == id))
        .map_or_else(|| "Default".to_owned(), |found| found.name.clone());
    rows.push(ui::labeled_row(
        palette,
        hook::id("read", "voice.row"),
        "Voice",
        Some(match view.voices() {
            None => "Looking for voices\u{2026}".to_owned(),
            Some(_) => "Default is the engine's own choice".to_owned(),
        }),
        ui::menu_button(
            palette,
            hook::id("read", "voice"),
            "Voice",
            current,
            ready,
            &view.focus.voice,
            toggle(Menu::Voice),
        ),
    ));
    if view.menu() == Some(Menu::Voice) {
        let mut choices = vec![(String::new(), "Default".to_owned(), None, voice.is_none())];
        choices.extend(voices.into_iter().map(|found| {
            let chosen = Some(&found.id) == voice.as_ref();
            let detail = match found.quality {
                VoiceQuality::Default => found.language.clone(),
                VoiceQuality::Enhanced => format!("{}, Enhanced", found.language),
                VoiceQuality::Premium => format!("{}, Premium", found.language),
            };
            (found.id, found.name, Some(detail), chosen)
        }));
        rows.push(options_list(palette, read, Menu::Voice, choices, options));
    }
    let speed = view.speed();
    let slider = {
        let read = read.clone();
        move |fraction: f32, cx: &mut App| {
            let speed = MIN_SPEED + fraction * (MAX_SPEED - MIN_SPEED);
            // Every value of the slider is a speed the setting takes.
            let _ = read.update(cx, |read, cx| read.set_speed(speed, cx));
        }
    };
    rows.push(ui::slider_row(
        palette,
        hook::id("read", "speed"),
        "Speed",
        &format!("{speed:.2}\u{d7}"),
        (speed - MIN_SPEED) / (MAX_SPEED - MIN_SPEED),
        &view.focus.speed,
        slider,
    ));
    rows.push(ui::labeled_row(
        palette,
        hook::id("read", "format.row"),
        "Format",
        Some("For Export Audio".to_owned()),
        ui::menu_button(
            palette,
            hook::id("read", "format"),
            "Format",
            view.format().to_uppercase(),
            true,
            &view.focus.format,
            toggle(Menu::Format),
        ),
    ));
    if view.menu() == Some(Menu::Format) {
        let choices = vec![(
            "wav".to_owned(),
            "WAV".to_owned(),
            Some("16-bit, 24 kHz, mono".to_owned()),
            true,
        )];
        rows.push(options_list(palette, read, Menu::Format, choices, options));
    }
    rows
}

/// One choice of an open menu: its value, label, note, and whether it is the current one.
type Choice = (String, String, Option<String>, bool);

fn options_list(
    palette: Palette,
    read: &Entity<ReadAloud>,
    menu: Menu,
    choices: Vec<Choice>,
    focus: &[FocusHandle],
) -> AnyElement {
    let name = match menu {
        Menu::Engine => "engine",
        Menu::Language => "language",
        Menu::Voice => "voice",
        Menu::Format => "format",
    };
    let rows = choices
        .into_iter()
        .zip(focus)
        .enumerate()
        .map(|(index, ((value, label, detail, chosen), focus))| {
            let read = read.clone();
            ui::option_row(
                palette,
                hook::indexed("read", &format!("{name}.option"), index),
                label,
                detail,
                chosen,
                focus,
                move |_, cx| {
                    // Each value comes from the list the setter checks against.
                    let _ = read.update(cx, |read, cx| match menu {
                        Menu::Engine => EngineKind::from_key(&value)
                            .map_or(Ok(()), |kind| read.set_engine(kind, cx)),
                        Menu::Language => read.set_language(&value, cx),
                        Menu::Voice => {
                            read.set_voice(Some(value.as_str()).filter(|id| !id.is_empty()), cx)
                        }
                        Menu::Format => read.set_format(&value, cx),
                    });
                },
            )
        })
        .collect();
    let close = read.clone();
    ui::option_list(hook::id("read", &format!("{name}.list")), rows, move |cx| {
        close.update(cx, |read, cx| read.close_menu(cx))
    })
}

/// What is happening, the progress of long text, then Export Audio and Play or Stop.
fn actions_row(palette: Palette, read: &Entity<ReadAloud>, cx: &App) -> AnyElement {
    let view = read.read(cx);
    let playing = view.playing();
    let progress = view.progress();
    let status = match (progress, playing) {
        (Some((done, total)), _) if total > 0 => format!("Reading part {done} of {total}"),
        (Some(_), _) => "Starting\u{2026}".to_owned(),
        (None, true) => "Playing".to_owned(),
        (None, false) => "Ready".to_owned(),
    };
    let enabled = view.unavailable().is_none();
    let export = {
        let read = read.clone();
        move |_: &mut Window, cx: &mut App| {
            // A refusal shows in the error line.
            let _ = read.update(cx, |read, cx| read.export(cx));
        }
    };
    let play = {
        let read = read.clone();
        move |_: &mut Window, cx: &mut App| {
            // A refusal shows in the error line.
            let _ = read.update(cx, |read, cx| {
                if playing {
                    read.stop(cx);
                    Ok(())
                } else {
                    read.play(cx)
                }
            });
        }
    };
    let fraction = progress
        .filter(|(_, total)| *total > 0)
        .map(|(done, total)| done as f32 / total as f32);
    ui::row(palette)
        .id(hook::id("read", "status.row"))
        .test_support()
        .child(
            ui::title(palette)
                .id(hook::id("read", "status"))
                .test_support()
                .aria_label(status.clone())
                .flex_1()
                .min_w_0()
                .child(status),
        )
        .children(fraction.map(|fraction| {
            div()
                .id(hook::id("read", "progress"))
                .test_support()
                .flex_none()
                .w(px(size::QUEUE_PROGRESS_WIDTH))
                .child(ui::track(palette, fraction))
        }))
        .child(ui::button(
            palette,
            hook::id("read", "export"),
            "Export Audio\u{2026}",
            Kind::Plain,
            Some(&view.focus.export),
            enabled && !view.exporting() && !view.choosing(),
            export,
        ))
        .child(ui::button(
            palette,
            hook::id("read", "play"),
            if playing { "Stop" } else { "Play" },
            Kind::Default,
            Some(&view.focus.play),
            enabled,
            play,
        ))
        .into_any_element()
}
