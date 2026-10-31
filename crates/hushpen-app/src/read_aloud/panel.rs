//! The Read aloud side of the Manual view: the text, the voice options, and Play and Export.

use super::{DEFAULT_VOICE, EngineKind, Menu, ReadAloud, language_name, mix_position};
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
        .gap(px(space::MD))
        .child(ui::section(
            palette,
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
        // Chatterbox reads English only; the system voice reads the rest.
        let other_engine = view.offers_other_engine().then(|| {
            let read = read.clone();
            ui::button(
                palette,
                hook::id("read", "otherEngine"),
                "Use System Voice",
                Kind::Plain,
                Some(&view.focus.other_engine),
                true,
                move |_: &mut Window, cx: &mut App| {
                    // The system voice is always a choice of the setting.
                    let _ = read.update(cx, |read, cx| read.set_engine(EngineKind::System, cx));
                },
            )
            .into_any_element()
        });
        let mut notices = Vec::new();
        if let Some(text) = view.error() {
            notices.push(ui::notice_row(
                palette,
                hook::id("read", "error"),
                text.to_owned(),
                Notice::Failure,
                other_engine,
            ));
        }
        if let Some(text) = view.info() {
            notices.push(ui::notice_row(
                palette,
                hook::id("read", "info"),
                text.to_owned(),
                Notice::Info,
                None,
            ));
        }
        notices
    };
    if !notices.is_empty() {
        page = page.child(ui::group(hook::id("read", "notices"), palette, notices));
    }
    page
}

/// Playback controls, the text area, then the size of the text and Open Text File.
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
        .p(px(size::ROW_PADDING))
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
    ui::group(
        hook::id("read", "text.group"),
        palette,
        vec![actions_row(palette, read, cx), area, footer],
    )
}

/// Engine, then Language and Voice from the engine's voices (and the voice library for
/// Chatterbox), then Speed and Format.
fn voice_rows(palette: Palette, read: &Entity<ReadAloud>, cx: &App) -> Vec<AnyElement> {
    let view = read.read(cx);
    let engine = view.engine();
    let toggle = |menu: Menu| {
        let read = read.clone();
        move |_: &mut Window, cx: &mut App| read.update(cx, |read, cx| read.toggle_menu(menu, cx))
    };
    let options = &view.focus.options;
    let open = |menu: Menu, choices: &dyn Fn() -> Vec<Choice>| {
        (view.menu() == Some(menu)).then(|| options_menu(read, menu, choices(), options))
    };
    let engine_menu = open(Menu::Engine, &|| {
        EngineKind::ALL
            .into_iter()
            .map(|kind| {
                (
                    kind.key().to_owned(),
                    kind.label().to_owned(),
                    view.missing(kind).then(|| "Not downloaded".to_owned()),
                    kind == engine,
                )
            })
            .collect()
    });
    let mut rows = vec![ui::labeled_row(
        palette,
        hook::id("read", "engine.row"),
        "Engine",
        Some(
            match engine {
                EngineKind::System => "The voices of this computer",
                EngineKind::Kokoro => "Natural voices from a downloaded model",
                EngineKind::ChatterboxTurbo => "Natural English voices, and voices you clone",
                EngineKind::ChatterboxNano => "Quick English voices, and voices you clone",
            }
            .to_owned(),
        ),
        ui::pop_up(
            palette,
            ui::PopUp {
                id: hook::id("read", "engine"),
                label: "Engine",
                current: engine.label().to_owned(),
                enabled: true,
                focus: &view.focus.engine,
                menu: engine_menu,
            },
            toggle(Menu::Engine),
        ),
    )];
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
    let language_menu = open(Menu::Language, &|| {
        view.languages()
            .into_iter()
            .map(|code| {
                let chosen = Some(&code) == language.as_ref();
                (code.clone(), language_name(&code), None, chosen)
            })
            .collect()
    });
    rows.push(ui::labeled_row(
        palette,
        hook::id("read", "language.row"),
        "Language",
        Some("Lists the voices that speak it".to_owned()),
        ui::pop_up(
            palette,
            ui::PopUp {
                id: hook::id("read", "language"),
                label: "Language",
                current: language
                    .as_deref()
                    .map_or_else(|| "None".to_owned(), language_name),
                enabled: ready && !view.languages().is_empty(),
                focus: &view.focus.language,
                menu: language_menu,
            },
            toggle(Menu::Language),
        ),
    ));
    let voices = view.language_voices();
    let voice = view.voice();
    let current = voice
        .as_ref()
        .and_then(|id| voices.iter().find(|found| &found.id == id))
        .map_or_else(|| DEFAULT_VOICE.to_owned(), |found| found.name.clone());
    // Chatterbox's default is the default voice of the library.
    let default_name = engine.is_chatterbox().then(|| view.default_voice_name(cx));
    let voice_menu = open(Menu::Voice, &|| {
        let mut choices = vec![(
            String::new(),
            DEFAULT_VOICE.to_owned(),
            default_name.clone(),
            voice.is_none(),
        )];
        choices.extend(voices.iter().map(|found| {
            let chosen = Some(&found.id) == voice.as_ref();
            let detail = match found.quality {
                VoiceQuality::Default => found.language.clone(),
                VoiceQuality::Enhanced => format!("{}, Enhanced", found.language),
                VoiceQuality::Premium => format!("{}, Premium", found.language),
            };
            (found.id.clone(), found.name.clone(), Some(detail), chosen)
        }));
        choices
    });
    rows.push(ui::labeled_row(
        palette,
        hook::id("read", "voice.row"),
        "Voice",
        Some(match (view.voices(), &default_name) {
            (None, _) => "Looking for voices\u{2026}".to_owned(),
            (Some(_), Some(name)) => format!("Default is {name}"),
            (Some(_), None) => "Default is the engine's own choice".to_owned(),
        }),
        ui::pop_up(
            palette,
            ui::PopUp {
                id: hook::id("read", "voice"),
                label: "Voice",
                current,
                enabled: ready,
                focus: &view.focus.voice,
                menu: voice_menu,
            },
            toggle(Menu::Voice),
        ),
    ));
    if engine == EngineKind::Kokoro {
        rows.extend(mix_rows(palette, read, cx));
    }
    if engine.is_chatterbox()
        && let Some(library) = view.library()
    {
        let cloned = library.read(cx).voices().len();
        let manage = {
            let read = read.clone();
            move |window: &mut Window, cx: &mut App| {
                read.update(cx, |read, cx| read.manage_voices(window, cx))
            }
        };
        rows.push(ui::labeled_row(
            palette,
            hook::id("read", "library.row"),
            "Voice library",
            Some(match cloned {
                0 => "Clone a voice from a recording or a file".to_owned(),
                1 => "1 cloned voice".to_owned(),
                count => format!("{count} cloned voices"),
            }),
            ui::button(
                palette,
                hook::id("read", "manage"),
                "Manage Voices\u{2026}",
                Kind::Plain,
                Some(&view.focus.manage),
                true,
                manage,
            ),
        ));
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
        "How fast the voice reads",
        ui::Slider {
            id: hook::id("read", "speed"),
            label: "Speed",
            position: (speed - MIN_SPEED) / (MAX_SPEED - MIN_SPEED),
            // The setting keeps a twentieth, so two decimals show it whole.
            reading: format!("{speed:.2}\u{d7}"),
            focus: &view.focus.speed,
        },
        slider,
    ));
    let format_menu = open(Menu::Format, &|| {
        vec![(
            "wav".to_owned(),
            "WAV".to_owned(),
            Some("16-bit, 24 kHz, mono".to_owned()),
            true,
        )]
    });
    rows.push(ui::labeled_row(
        palette,
        hook::id("read", "format.row"),
        "Format",
        Some("For Export Audio".to_owned()),
        ui::pop_up(
            palette,
            ui::PopUp {
                id: hook::id("read", "format"),
                label: "Format",
                current: view.format().to_uppercase(),
                enabled: true,
                focus: &view.focus.format,
                menu: format_menu,
            },
            toggle(Menu::Format),
        ),
    ));
    rows
}

/// Mix with, then the Mix slider while a voice is chosen there.
fn mix_rows(palette: Palette, read: &Entity<ReadAloud>, cx: &App) -> Vec<AnyElement> {
    let view = read.read(cx);
    let voices = view.mix_voices();
    let mix = view.mix_voice();
    let mix_name = mix
        .as_ref()
        .and_then(|id| voices.iter().find(|found| &found.id == id))
        .map(|found| found.name.clone());
    let menu = (view.menu() == Some(Menu::Mix)).then(|| {
        let mut choices = vec![(String::new(), "None".to_owned(), None, mix.is_none())];
        choices.extend(voices.iter().map(|found| {
            let chosen = Some(&found.id) == mix.as_ref();
            (
                found.id.clone(),
                found.name.clone(),
                Some(found.language.clone()),
                chosen,
            )
        }));
        options_menu(read, Menu::Mix, choices, &view.focus.options)
    });
    let toggle = {
        let read = read.clone();
        move |_: &mut Window, cx: &mut App| {
            read.update(cx, |read, cx| read.toggle_menu(Menu::Mix, cx))
        }
    };
    let mut rows = vec![ui::labeled_row(
        palette,
        hook::id("read", "mix.row"),
        "Mix with",
        Some("Blends a second voice into this one".to_owned()),
        ui::pop_up(
            palette,
            ui::PopUp {
                id: hook::id("read", "mix"),
                label: "Mix with",
                current: mix_name.clone().unwrap_or_else(|| "None".to_owned()),
                enabled: !voices.is_empty(),
                focus: &view.focus.mix,
                menu,
            },
            toggle,
        ),
    )];
    if let Some(name) = mix_name {
        let percent = view.mix_percent();
        let slider = {
            let read = read.clone();
            move |position: f32, cx: &mut App| {
                read.update(cx, |read, cx| read.set_mix_position(position, cx))
            }
        };
        rows.push(ui::slider_row(
            palette,
            &format!("How much of {name} the voice has"),
            ui::Slider {
                id: hook::id("read", "mixShare"),
                label: "Mix",
                position: mix_position(percent),
                reading: format!("{percent}%"),
                focus: &view.focus.mix_share,
            },
            slider,
        ));
    }
    rows
}

/// One choice of an open menu: its value, label, note, and whether it is the current one.
type Choice = (String, String, Option<String>, bool);

fn options_menu(
    read: &Entity<ReadAloud>,
    menu: Menu,
    choices: Vec<Choice>,
    focus: &[FocusHandle],
) -> ui::Menu {
    let name = match menu {
        Menu::Engine => "engine",
        Menu::Language => "language",
        Menu::Voice => "voice",
        Menu::Mix => "mix",
        Menu::Format => "format",
    };
    let close = read.clone();
    let list = ui::Menu::new(hook::id("read", &format!("{name}.list")), move |cx| {
        close.update(cx, |read, cx| read.close_menu(cx))
    });
    choices.into_iter().zip(focus).enumerate().fold(
        list,
        |list, (index, ((value, label, detail, chosen), focus))| {
            let read = read.clone();
            list.option(
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
                        Menu::Mix => {
                            read.set_mix(Some(value.as_str()).filter(|id| !id.is_empty()), cx)
                        }
                        Menu::Format => read.set_format(&value, cx),
                    });
                },
            )
        },
    )
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
