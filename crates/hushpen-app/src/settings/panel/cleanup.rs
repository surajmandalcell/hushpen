//! Cleanup: the rules, spoken punctuation, AI cleanup, where it runs, the local model with its
//! download status, and the profile with its prompt. The endpoint rows are in
//! [`super::endpoint`].

use super::Page;
use crate::ai_cleanup;
use crate::settings::{AiStatus, Settings, slots};
use crate::ui::{self, Kind, Notice};
use gpui_kit::{AnyElement, App, IntoElement};
use hushpen_core::llm_prompt::{Profile, PromptFamily, Target};

pub fn rules(page: &Page, cx: &mut App) -> AnyElement {
    let on = page.settings.read(cx).stored_bool("cleanup.rules");
    page.toggle(
        "cleanup.rules",
        "Clean up text",
        "Fillers and stutters are removed before the text is pasted.",
        on,
        slots::RULES,
        Settings::toggle_rules,
    )
}

pub fn spoken_punctuation(page: &Page, cx: &mut App) -> AnyElement {
    let on = page
        .settings
        .read(cx)
        .stored_bool("cleanup.spokenPunctuation");
    page.toggle(
        "cleanup.spokenPunctuation",
        "Spoken punctuation",
        "Saying \"comma\" or \"period\" writes the mark.",
        on,
        slots::SPOKEN_PUNCTUATION,
        Settings::toggle_spoken_punctuation,
    )
}

pub fn ai_cleanup(page: &Page, cx: &mut App) -> AnyElement {
    let on = page.settings.read(cx).stored_bool("cleanup.llm.enabled");
    page.toggle(
        "cleanup.ai",
        "AI cleanup",
        "A language model fixes self-corrections.",
        on,
        slots::AI_CLEANUP,
        Settings::toggle_ai_cleanup,
    )
}

pub fn ai_source(page: &Page, cx: &mut App) -> AnyElement {
    let provider = page.settings.read(cx).ai_provider();
    let menu = page.picker_open("cleanup.provider", cx).then(|| {
        let choices = ["local", "endpoint"]
            .into_iter()
            .map(|value| {
                (
                    value.to_owned(),
                    provider_label(value).to_owned(),
                    value == provider,
                )
            })
            .collect();
        page.options(
            "cleanup.provider",
            choices,
            &page.options,
            Settings::set_ai_provider,
        )
    });
    page.menu(
        "cleanup.provider",
        "AI source",
        "Where the model runs.",
        provider_label(provider).to_owned(),
        slots::AI_PROVIDER,
        menu,
    )
}

fn provider_label(provider: &str) -> &'static str {
    match provider {
        "endpoint" => "OpenAI-compatible endpoint",
        _ => "This computer",
    }
}

/// The local model, its picker, and its status. Nothing while AI cleanup uses an endpoint.
pub fn ai_model(page: &Page, cx: &mut App) -> Vec<AnyElement> {
    let (on, provider, model, offered, status) = {
        let me = page.settings.read(cx);
        (
            me.stored_bool("cleanup.llm.enabled"),
            me.ai_provider(),
            me.ai_model(),
            me.ai_offered(),
            me.ai_status(cx),
        )
    };
    let Some(model) = model.filter(|_| provider == "local") else {
        return Vec::new();
    };
    let size = crate::models::panel::format_size(model.bytes());
    let menu = page.picker_open("cleanup.aiModel", cx).then(|| {
        let choices = offered
            .into_iter()
            .map(|entry| {
                let selected = entry.id == model.id;
                (entry.id, entry.name, selected)
            })
            .collect();
        page.options(
            "cleanup.aiModel",
            choices,
            &page.options,
            Settings::set_ai_model,
        )
    });
    let mut rows = vec![page.menu(
        "cleanup.aiModel",
        "AI model",
        &format!("{size} download. Chosen for this computer."),
        model.name.clone(),
        slots::AI_MODEL,
        menu,
    )];
    let can_download = on && matches!(status, Some(AiStatus::NotDownloaded | AiStatus::Failed));
    let download = can_download.then(|| {
        let settings = page.settings.clone();
        ui::button(
            page.palette,
            super::settings_id("cleanup.aiDownload"),
            "Download",
            Kind::Plain,
            Some(&page.focus[slots::AI_DOWNLOAD]),
            true,
            move |_, cx| settings.update(cx, |me, cx| me.download_ai_model(cx)),
        )
        .into_any_element()
    });
    rows.push(ui::notice_row(
        page.palette,
        super::settings_id("cleanup.aiStatus"),
        status_text(status, on),
        Notice::status(matches!(status, Some(AiStatus::Failed))),
        download,
    ));
    rows
}

/// The profile pop-up: how the cleaned text reads. A profile the user changed says so.
pub fn profile(page: &Page, cx: &mut App) -> AnyElement {
    let (current, edited) = {
        let me = page.settings.read(cx);
        let storage = &me.storage;
        let current = me.cleanup_profile();
        let edited: Vec<bool> = Profile::ALL
            .iter()
            .map(|profile| ai_cleanup::profile_of(storage, *profile).edited())
            .collect();
        (current, edited)
    };
    let menu = page.picker_open("cleanup.profile", cx).then(|| {
        let close = page.settings.clone();
        let menu = ui::Menu::new(super::settings_id("cleanup.profile.list"), move |cx| {
            close.update(cx, |me, cx| me.close_picker(cx))
        });
        Profile::ALL
            .into_iter()
            .zip(&edited)
            .zip(&page.options)
            .fold(menu, |menu, ((profile, edited), focus)| {
                let settings = page.settings.clone();
                menu.option(
                    super::settings_id(&format!("cleanup.profile.option.{}", profile.key())),
                    profile.label(),
                    edited.then(|| "Edited".to_owned()),
                    profile == current,
                    focus,
                    move |_, cx| {
                        settings.update(cx, |me, cx| me.set_cleanup_profile(profile.key(), cx))
                    },
                )
            })
    });
    let index = Profile::ALL.iter().position(|p| *p == current).unwrap_or(0);
    let detail = if edited[index] {
        "How the cleaned text reads. Edited."
    } else {
        "How the cleaned text reads."
    };
    page.menu(
        "cleanup.profile",
        "Profile",
        detail,
        current.label().to_owned(),
        slots::PROFILE,
        menu,
    )
}

/// The prompt the cleanup model reads, with the button that opens it: View for S1-mini, whose
/// prompt is fixed, and Edit for the others.
pub fn prompt(page: &Page, cx: &mut App) -> AnyElement {
    let (target, tuned) = {
        let me = page.settings.read(cx);
        (me.cleanup_target(), ai_cleanup::profile(&me.storage))
    };
    let fixed = target == Target::Local(PromptFamily::S1Mini);
    let detail = if fixed {
        let controls = tuned.controls;
        format!(
            "S1-mini: {}, {}, {}.",
            controls.styling.label(),
            controls.structure.key(),
            controls.context.key()
        )
    } else {
        "What the model reads before your text.".to_owned()
    };
    let settings = page.settings.clone();
    let button = ui::button(
        page.palette,
        super::settings_id("cleanup.prompt"),
        if fixed {
            "View\u{2026}"
        } else {
            "Edit\u{2026}"
        },
        Kind::Plain,
        Some(&page.focus[slots::PROMPT]),
        true,
        move |window, cx| settings.update(cx, |me, cx| me.open_prompt(window, cx)),
    );
    ui::labeled_row(
        page.palette,
        super::settings_id("cleanup.prompt.row"),
        "Prompt",
        Some(detail),
        button,
    )
}
fn status_text(status: Option<AiStatus>, on: bool) -> String {
    match status {
        None => "No AI model is available.".into(),
        Some(AiStatus::NotDownloaded) if on => {
            "Not downloaded. Dictations use the rule text until it is ready.".into()
        }
        Some(AiStatus::NotDownloaded) => "Downloads when you turn AI cleanup on.".into(),
        Some(AiStatus::Downloading(Some(percent))) => format!("Downloading {percent}%"),
        Some(AiStatus::Downloading(None)) => "Downloading".into(),
        Some(AiStatus::Verifying) => "Checking the download".into(),
        Some(AiStatus::Failed) => "The file failed its check. Download it again.".into(),
        Some(AiStatus::Preparing) => "Preparing the AI model".into(),
        Some(AiStatus::Ready) if on => "Ready".into(),
        Some(AiStatus::Ready) => "Downloaded".into(),
    }
}
