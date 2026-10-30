//! The "Manage Voices" sheet, drawn over the window like the audio enhance sheet. The list page
//! has the voice that comes with the model, then each cloned voice with play, rename, delete,
//! and Set as Default; the add page has the lines to read, the take with its level meter, the
//! file to clone from instead, the preview, and the name. Tab stays inside the sheet, Esc and
//! Done close it, and focus goes back to the control that opened it.

use super::{BUILT_IN_NAME, Page, Playing, READING, Take, VoiceLibrary, detail, take_detail};
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
use hushpen_core::chatterbox::BUILT_IN_VOICE;

/// The width of the last place in a voice row: Set as Default, or the word Default.
const DEFAULT_COLUMN_WIDTH: f32 = 108.0;

fn id(name: &str) -> SharedString {
    hook::id("voices", name)
}

/// The sheet, or nothing while it is closed. It comes in like every sheet: the scrim fades in,
/// and the sheet fades in and rises a few pixels into place.
pub fn render(
    library: &Entity<VoiceLibrary>,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let page = library.read(cx).page()?;
    let arrival = motion::enter(id("sheet.arrival"), motion::BASE, window, cx);
    let palette = theme::palette(cx);
    let close = {
        let library = library.clone();
        move |window: &mut Window, cx: &mut App| {
            library.update(cx, |library, cx| library.close(window, cx));
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
    let me = library.read(cx);
    let (title, body) = match page {
        Page::List => ("Voices", list_page(palette, library, me)),
        Page::Add => ("Add Voice", add_page(palette, library, me, cx)),
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
        .child(footer(palette, library, page, me, close));
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

/// Runs `act` on the library when a control is pressed.
fn act(
    library: &Entity<VoiceLibrary>,
    act: impl Fn(&mut VoiceLibrary, &mut Window, &mut Context<VoiceLibrary>) + 'static,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let library = library.clone();
    move |window, cx| library.update(cx, |me, cx| act(me, window, cx))
}

fn notice_row(palette: Palette, text: String) -> AnyElement {
    ui::notice_row(palette, id("notice"), text, Notice::Alert, None)
}

fn list_page(
    palette: Palette,
    library: &Entity<VoiceLibrary>,
    me: &VoiceLibrary,
) -> Vec<AnyElement> {
    let default = me.default_id();
    let mut rows = vec![voice_row(
        palette,
        library,
        me,
        RowView {
            index: 0,
            id: BUILT_IN_VOICE.to_owned(),
            name: BUILT_IN_NAME.to_owned(),
            detail: "The voice that comes with the model".to_owned(),
            cloned: false,
            default: default.is_none(),
            playable: me.built_in_clip().is_some(),
        },
    )];
    rows.extend(me.voices().iter().enumerate().map(|(index, voice)| {
        voice_row(
            palette,
            library,
            me,
            RowView {
                index: index + 1,
                id: voice.id.clone(),
                name: voice.name.clone(),
                detail: detail(voice),
                cloned: true,
                default: Some(&voice.id) == default.as_ref(),
                playable: true,
            },
        )
    }));
    if let Some(notice) = me.notice() {
        rows.push(notice_row(palette, notice.to_owned()));
    }
    vec![
        header(
            palette,
            "Voices",
            "Chatterbox reads with the default voice unless you pick another.",
        ),
        ui::group(id("list"), palette, rows).into_any_element(),
    ]
}

struct RowView {
    /// 0 for the built-in voice, then 1 and up for the cloned ones.
    index: usize,
    id: String,
    name: String,
    detail: String,
    /// A cloned voice can be renamed and deleted; the built-in one cannot.
    cloned: bool,
    default: bool,
    playable: bool,
}

/// Lightest to heaviest: play, rename, delete, then Set as Default or the word Default. The
/// built-in voice keeps empty places for rename and delete, so the columns line up.
fn voice_row(
    palette: Palette,
    library: &Entity<VoiceLibrary>,
    me: &VoiceLibrary,
    view: RowView,
) -> AnyElement {
    let RowView {
        index,
        id: voice,
        name,
        detail,
        cloned,
        default,
        playable,
    } = view;
    if me.renaming() == Some(voice.as_str()) {
        return rename_row(palette, library, me, index);
    }
    if me.deleting() == Some(voice.as_str()) {
        return delete_row(palette, library, me, index, &voice, &name);
    }
    let focus = &me.focus.rows[index];
    let what = Playing::Voice(voice.clone());
    let playing = me.playing() == Some(&what);
    let play = ui::icon_button_enabled(
        palette,
        hook::indexed("voices", "play", index),
        if playing {
            IconName::Square
        } else {
            IconName::Play
        },
        if playing {
            format!("Stop {name}")
        } else {
            format!("Play {name}")
        },
        if playing { "Stop" } else { "Play" },
        &focus.play,
        playable,
        act(library, move |me, _, cx| me.toggle_play(what.clone(), cx)),
    )
    .into_any_element();
    let edits: Vec<AnyElement> = if cloned {
        let (rename_id, delete_id) = (voice.clone(), voice.clone());
        vec![
            ui::icon_button(
                palette,
                hook::indexed("voices", "rename", index),
                IconName::Pencil,
                format!("Rename {name}"),
                "Rename",
                &focus.rename,
                {
                    let library = library.clone();
                    move |window: &mut Window, cx: &mut App| {
                        let started = library.update(cx, |me, cx| me.start_rename(&rename_id, cx));
                        if started.is_ok() {
                            let field = library.read(cx).rename.clone();
                            field.update(cx, |input, cx| input.focus(window, cx));
                        }
                    }
                },
            )
            .into_any_element(),
            ui::icon_button(
                palette,
                hook::indexed("voices", "delete", index),
                IconName::Trash,
                format!("Delete {name}"),
                "Delete",
                &focus.delete,
                act(library, move |me, window, cx| {
                    me.ask_delete(&delete_id, cx);
                    window.focus(&me.focus.cancel, cx);
                }),
            )
            .into_any_element(),
        ]
    } else {
        (0..2).map(|_| empty_icon_place()).collect()
    };
    let last = if default {
        ui::value(palette)
            .id(hook::indexed("voices", "default", index))
            .test_support()
            .aria_label("Default")
            .child("Default")
            .into_any_element()
    } else {
        let chosen = cloned.then(|| voice.clone());
        ui::button(
            palette,
            hook::indexed("voices", "setDefault", index),
            "Set as Default",
            Kind::Plain,
            Some(&focus.default),
            true,
            act(library, move |me, _, cx| {
                // A refusal shows in the notice row.
                let _ = me.set_default(chosen.as_deref(), cx);
            }),
        )
        .into_any_element()
    };
    ui::row(palette)
        .id(hook::indexed("voices", "row", index))
        .test_support()
        .child(ui::named_lines(
            palette,
            hook::indexed("voices", "name", index),
            name,
            Some((hook::indexed("voices", "detail", index), detail)),
        ))
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(space::SM))
                .child(play)
                .children(edits)
                .child(
                    div()
                        .min_w(px(DEFAULT_COLUMN_WIDTH))
                        .flex()
                        .justify_end()
                        .child(last),
                ),
        )
        .into_any_element()
}

/// The place of an icon button a row does not have.
fn empty_icon_place() -> AnyElement {
    div()
        .flex_none()
        .size(px(size::BUTTON_HEIGHT))
        .into_any_element()
}

/// The row of a voice being renamed: the name field, Cancel, and Save.
fn rename_row(
    palette: Palette,
    library: &Entity<VoiceLibrary>,
    me: &VoiceLibrary,
    index: usize,
) -> AnyElement {
    let cancel = {
        let library = library.clone();
        move |window: &mut Window, cx: &mut App| {
            library.update(cx, |me, cx| {
                me.cancel_rename(cx);
                window.focus(&me.focus.rows[index].rename, cx);
            });
        }
    };
    let save = act(library, |me, window, cx| {
        // A refusal shows in the notice row.
        let _ = me.save_rename(Some(window), cx);
    });
    ui::row(palette)
        .id(hook::indexed("voices", "row", index))
        .test_support()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(ui::field(palette, id("rename"), "Name", &me.rename)),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(space::SM))
                .child(ui::button(
                    palette,
                    id("rename.cancel"),
                    "Cancel",
                    Kind::Plain,
                    Some(&me.focus.cancel),
                    true,
                    cancel,
                ))
                .child(ui::button(
                    palette,
                    id("rename.save"),
                    "Save",
                    Kind::Default,
                    Some(&me.focus.confirm),
                    true,
                    save,
                )),
        )
        .into_any_element()
}

/// The row of a voice that asks whether to delete it.
fn delete_row(
    palette: Palette,
    library: &Entity<VoiceLibrary>,
    me: &VoiceLibrary,
    index: usize,
    voice: &str,
    name: &str,
) -> AnyElement {
    let cancel = {
        let library = library.clone();
        move |window: &mut Window, cx: &mut App| {
            library.update(cx, |me, cx| {
                me.cancel_delete(cx);
                window.focus(&me.focus.rows[index].delete, cx);
            });
        }
    };
    let delete = {
        let voice = voice.to_owned();
        act(library, move |me, window, cx| {
            if me.delete(&voice, cx).is_ok() {
                window.focus(&me.focus.add, cx);
            }
        })
    };
    ui::row(palette)
        .id(hook::indexed("voices", "row", index))
        .test_support()
        .child(ui::named_lines(
            palette,
            id("delete.title"),
            format!("Delete \u{201c}{name}\u{201d}?"),
            Some((id("delete.detail"), "Its clip is deleted too.".to_owned())),
        ))
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(space::SM))
                .child(ui::button(
                    palette,
                    id("delete.cancel"),
                    "Cancel",
                    Kind::Plain,
                    Some(&me.focus.cancel),
                    true,
                    cancel,
                ))
                .child(ui::button(
                    palette,
                    id("delete.confirm"),
                    "Delete",
                    Kind::Default,
                    Some(&me.focus.confirm),
                    true,
                    delete,
                )),
        )
        .into_any_element()
}

fn add_page(
    palette: Palette,
    library: &Entity<VoiceLibrary>,
    me: &VoiceLibrary,
    cx: &App,
) -> Vec<AnyElement> {
    let focus = &me.focus;
    let take = me.take();
    let recording = matches!(take, Take::Recording { .. });
    let lines = READING.iter().enumerate().map(|(index, line)| {
        div()
            .id(hook::indexed("voices", "reading.line", index))
            .test_support()
            .aria_label(*line)
            .text_token(BODY)
            .text_color(palette.label)
            .child(*line)
    });
    let reading = div()
        .id(id("reading"))
        .test_support()
        .flex()
        .flex_col()
        .gap(px(space::XS))
        .px(px(space::XL))
        .py(px(space::LG))
        .children(lines)
        .into_any_element();
    let record = ui::icon_button_enabled(
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
            "Record the voice"
        }
        .to_owned(),
        if recording { "Stop" } else { "Record" },
        &focus.record,
        me.can_record() && take != Take::Preparing,
        act(library, |me, _, cx| me.toggle_take(cx)),
    );
    let playing = me.playing() == Some(&Playing::NewClip);
    // Off until a clip exists, rather than missing, so the record button never moves.
    let play = ui::icon_button_enabled(
        palette,
        id("preview"),
        if playing {
            IconName::Square
        } else {
            IconName::Play
        },
        if playing {
            "Stop the clip"
        } else {
            "Play the clip"
        }
        .to_owned(),
        if playing { "Stop" } else { "Play" },
        &focus.play,
        me.clip().is_some() && take == Take::Idle,
        act(library, |me, _, cx| me.toggle_play(Playing::NewClip, cx)),
    );
    let take_row = ui::row(palette)
        .id(id("take.row"))
        .test_support()
        .child(ui::named_lines(
            palette,
            id("take.title"),
            "Recording",
            Some((id("take.detail"), take_detail(take, me.clip()))),
        ))
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(space::SM))
                .child(record)
                .child(play),
        )
        .into_any_element();
    let mut take_rows = vec![reading, take_row];
    if recording {
        take_rows.push(ui::meter_row(palette, "voices", &me.meter(cx), true));
    }
    take_rows.push(ui::labeled_row(
        palette,
        id("file.row"),
        "Audio file",
        Some("Or clone the voice from a WAV, MP3, or M4A file".to_owned()),
        ui::button(
            palette,
            id("file"),
            "Choose File\u{2026}",
            Kind::Plain,
            Some(&focus.import),
            take == Take::Idle,
            act(library, |me, _, cx| me.choose_file(cx)),
        ),
    ));
    if let Some(notice) = me.notice() {
        take_rows.push(notice_row(palette, notice.to_owned()));
    }
    let name = ui::field_row(
        palette,
        id("name"),
        "Name",
        "How the voice shows in the Voice menu",
        &me.name,
        None,
    );
    vec![
        header(
            palette,
            "Add Voice",
            "Use only a voice you have the right to use.",
        ),
        ui::group(id("take"), palette, take_rows).into_any_element(),
        ui::group(id("name.group"), palette, vec![name]).into_any_element(),
    ]
}

/// The buttons under the page, lightest to heaviest: Add Voice and Done on the list, Back and
/// Save on the add page.
fn footer(
    palette: Palette,
    library: &Entity<VoiceLibrary>,
    page: Page,
    me: &VoiceLibrary,
    close: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let focus = &me.focus;
    let buttons: Vec<AnyElement> = match page {
        Page::List => vec![
            ui::button(
                palette,
                id("add"),
                "Add Voice\u{2026}",
                Kind::Plain,
                Some(&focus.add),
                true,
                open(library, Page::Add),
            )
            .into_any_element(),
            ui::button(
                palette,
                id("done"),
                "Done",
                Kind::Default,
                Some(&focus.done),
                true,
                close,
            )
            .into_any_element(),
        ],
        Page::Add => vec![
            ui::button(
                palette,
                id("back"),
                "Back",
                Kind::Plain,
                Some(&focus.back),
                true,
                open(library, Page::List),
            )
            .into_any_element(),
            ui::button(
                palette,
                id("save"),
                "Save",
                Kind::Default,
                Some(&focus.save),
                me.clip().is_some() && me.take() == Take::Idle,
                act(library, |me, window, cx| {
                    if me.save(cx).is_ok() {
                        window.focus(&me.focus.add, cx);
                    }
                }),
            )
            .into_any_element(),
        ],
    };
    div()
        .flex()
        .items_center()
        .justify_end()
        .gap(px(space::SM))
        .children(buttons)
        .into_any_element()
}

/// Turns the sheet to `page`, with focus on its first control.
fn open(library: &Entity<VoiceLibrary>, page: Page) -> impl Fn(&mut Window, &mut App) + 'static {
    act(library, move |me, window, cx| {
        me.open(page, None, window, cx)
    })
}
