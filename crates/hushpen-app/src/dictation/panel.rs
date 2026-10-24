//! The dictation group on the Home view: record button, status, transcript, and language.

use super::{Dictation, Phase};
use crate::hook;
use crate::shell::Shell;
use crate::theme::{self, BODY, Palette, StyledType, size, space};
use crate::ui::{self, Kind, Notice};
use crate::views::View;
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement, Rgba,
    Role, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use hushpen_core::language;

struct Snapshot {
    phase: Phase,
    /// The cleanup model is loading, so the wait is longer than usual.
    preparing: bool,
    status: String,
    transcript: String,
    /// A failed Command Mode edit has no transcript to wait for: the error says it all.
    show_transcript: bool,
    notice: Option<(String, bool)>,
    /// Why the global keys are off, shown as plain information rather than an error.
    keys_notice: Option<String>,
    can_record: bool,
    language: String,
    detail: String,
    picker_open: bool,
    picker_off: bool,
    codes: Vec<&'static str>,
}

pub fn render(
    dictation: &Entity<Dictation>,
    shell: &Entity<Shell>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let view = {
        let me = dictation.read(cx);
        let blocker = me.blocker(cx);
        let off = me.picker_off_reason(cx);
        let language = me.language(cx);
        let phase = me.phase(cx);
        let detected = me
            .detected(cx)
            .map(|code| language::label(code).to_owned())
            .filter(|_| phase == Phase::Done);
        let detail = match (off, detected) {
            (Some(reason), _) => reason.to_owned(),
            (None, Some(name)) => format!("Detected: {name}"),
            (None, None) if language == language::AUTO => "Detects the language you speak".into(),
            (None, None) => String::new(),
        };
        let notice = match (me.notice(cx), blocker) {
            (Some(notice), _) if phase != Phase::Listening => Some((notice.message.clone(), false)),
            (_, Some(blocker)) if !phase.busy() => Some((blocker.message, blocker.models_link)),
            _ => None,
        };
        let preparing = phase == Phase::Transcribing && me.preparing_ai(cx);
        Snapshot {
            phase,
            preparing,
            status: if preparing {
                "Preparing the AI model".into()
            } else {
                status_text(phase, me.can_record(cx))
            },
            transcript: me.transcript(cx).to_owned(),
            show_transcript: !(phase == Phase::Failed
                && me.ran_command(cx)
                && me.transcript(cx).is_empty()),
            notice,
            keys_notice: me.keys_notice(cx).map(str::to_owned),
            can_record: me.can_record(cx),
            language,
            detail,
            picker_open: me.picker_open(),
            picker_off: off.is_some(),
            codes: me.picker_codes(),
        }
    };
    let (record_focus, copy_focus, models_focus, language_focus, option_focus) = {
        let me = dictation.read(cx);
        (
            me.record_focus.clone(),
            me.copy_focus.clone(),
            me.models_focus.clone(),
            me.language_focus.clone(),
            me.option_focus.clone(),
        )
    };

    let palette = theme::palette(cx);
    let mut rows: Vec<AnyElement> =
        vec![header(palette, dictation, &view, &record_focus, &copy_focus).into_any_element()];
    if let Some((message, link)) = view.notice.clone() {
        rows.push(notice_row(palette, shell, message, link, &models_focus));
    }
    if let Some(message) = view.keys_notice.clone() {
        // Why the global keys are off is plain information, not an error.
        rows.push(ui::notice_row(
            palette,
            hook::id("home", "keys-notice"),
            message,
            Notice::Info,
            None,
        ));
    }
    if view.show_transcript {
        rows.push(transcript_row(palette, &view).into_any_element());
    }
    rows.push(language_row(palette, dictation, &view, &language_focus).into_any_element());
    if view.picker_open {
        rows.push(option_list(palette, dictation, &view, &option_focus).into_any_element());
    }
    ui::group(hook::id("home", "dictation"), palette, rows)
}

fn status_text(phase: Phase, can_record: bool) -> String {
    match phase {
        Phase::Listening => "Listening",
        Phase::Transcribing => "Transcribing",
        Phase::Done => "Done",
        Phase::NoSpeech => "No speech heard",
        Phase::Failed => "Failed",
        Phase::Idle if can_record => "Ready",
        Phase::Idle => "Not ready",
    }
    .into()
}

/// The color of the status word: a status color for ready and failed. Not ready stays neutral;
/// the notice row under it says why.
fn status_tone(palette: Palette, view: &Snapshot) -> Rgba {
    match view.phase {
        Phase::Idle if view.can_record => palette.success,
        Phase::Failed => palette.error,
        _ => palette.label_secondary,
    }
}

fn header(
    palette: Palette,
    dictation: &Entity<Dictation>,
    view: &Snapshot,
    record_focus: &FocusHandle,
    copy_focus: &FocusHandle,
) -> impl IntoElement + use<> {
    let listening = view.phase == Phase::Listening;
    let label = if listening { "Stop" } else { "Record" };
    let record = {
        let dictation = dictation.clone();
        move |_: &mut Window, cx: &mut App| {
            dictation.update(cx, |me, cx| {
                // A refused start shows its reason in the notice row.
                let _ = me.toggle(cx);
            });
        }
    };
    let copy = {
        let dictation = dictation.clone();
        move |_: &mut Window, cx: &mut App| {
            dictation.update(cx, |me, cx| {
                let _ = me.copy(cx);
            });
        }
    };
    let enabled = view.can_record && view.phase != Phase::Transcribing;
    ui::icon_row(
        palette,
        IconName::Mic,
        ui::title(palette).child("Dictation"),
        None,
    )
    .child(
        ui::value(palette)
            .id(hook::id("home", "status"))
            .test_support()
            .role(Role::Status)
            .aria_label(view.status.clone())
            .text_color(status_tone(palette, view))
            .child(view.status.clone()),
    )
    .children((!view.transcript.is_empty()).then(|| {
        ui::button(
            palette,
            hook::id("home", "copy"),
            "Copy",
            Kind::Plain,
            Some(copy_focus),
            true,
            copy,
        )
    }))
    .child(ui::button(
        palette,
        hook::id("home", "record"),
        label,
        Kind::Default,
        enabled.then_some(record_focus),
        enabled,
        record,
    ))
}

fn notice_row(
    palette: Palette,
    shell: &Entity<Shell>,
    message: String,
    models_link: bool,
    focus: &FocusHandle,
) -> AnyElement {
    let open = {
        let shell = shell.clone();
        move |_: &mut Window, cx: &mut App| {
            shell.update(cx, |shell, cx| shell.select(View::Models, cx));
        }
    };
    let link = models_link.then(|| {
        ui::button(
            palette,
            hook::id("home", "notice.models"),
            "Open Models",
            Kind::Plain,
            Some(focus),
            true,
            open,
        )
        .into_any_element()
    });
    ui::notice_row(
        palette,
        hook::id("home", "notice"),
        message,
        Notice::Alert,
        link,
    )
}

/// The transcript, or the placeholder that says what happens now. The block is centered in the
/// group, and a long transcript keeps every line centered.
fn transcript_row(palette: Palette, view: &Snapshot) -> impl IntoElement + use<> {
    let empty = view.transcript.is_empty();
    let placeholder = match view.phase {
        Phase::Listening => "Speak now. Stop when you are done.",
        Phase::Transcribing if view.preparing => "Preparing the AI model.",
        Phase::Transcribing => "Transcribing your recording.",
        Phase::NoSpeech => "No speech was heard.",
        Phase::Failed => "Nothing was transcribed.",
        Phase::Idle | Phase::Done => "Your words appear here and are copied to the clipboard.",
    };
    let text: SharedString = if empty {
        placeholder.into()
    } else {
        view.transcript.clone().into()
    };
    div()
        .id(hook::id("home", "transcript"))
        .test_support()
        .role(Role::Paragraph)
        .aria_value(if empty { "" } else { &view.transcript }.to_owned())
        .min_h(px(size::TRANSCRIPT_MIN_HEIGHT))
        .w_full()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .px(px(space::XXL))
        .py(px(space::LG))
        .text_center()
        .text_token(BODY)
        .text_color(if empty {
            palette.label_secondary
        } else {
            palette.label
        })
        .child(text)
}

fn language_row(
    palette: Palette,
    dictation: &Entity<Dictation>,
    view: &Snapshot,
    focus: &FocusHandle,
) -> impl IntoElement + use<> {
    let toggle = {
        let dictation = dictation.clone();
        move |_: &mut Window, cx: &mut App| {
            dictation.update(cx, |me, cx| {
                let _ = me.toggle_picker(cx);
            });
        }
    };
    let detail = ui::secondary(palette)
        .id(hook::id("home", "language.detail"))
        .test_support()
        .aria_label(view.detail.clone())
        .child(view.detail.clone());
    ui::icon_row(
        palette,
        IconName::Globe,
        ui::title(palette).child("Language"),
        Some(detail.into_any_element()),
    )
    .child(ui::menu_button(
        palette,
        hook::id("home", "language"),
        "Language",
        language::label(&view.language).to_owned(),
        !view.picker_off,
        focus,
        toggle,
    ))
}

fn option_list(
    palette: Palette,
    dictation: &Entity<Dictation>,
    view: &Snapshot,
    focus: &[FocusHandle],
) -> impl IntoElement + use<> {
    let rows = view.codes.iter().zip(focus).map(|(&code, focus)| {
        let choose = {
            let dictation = dictation.clone();
            move |_: &mut Window, cx: &mut App| {
                dictation.update(cx, |me, cx| {
                    let _ = me.set_language(code, cx);
                });
            }
        };
        ui::option_row(
            palette,
            hook::id("home", &format!("language.option.{code}")),
            language::label(code).to_owned(),
            None,
            code == view.language,
            focus,
            choose,
        )
    });
    let close = dictation.clone();
    ui::option_list(
        hook::id("home", "language.list"),
        rows.collect(),
        move |cx| close.update(cx, |me, cx| me.close_picker(cx)),
    )
}
