//! Cleanup on an endpoint: the preset, the address, the model, the API key, and the connection
//! test. The rows show only while AI cleanup runs on an endpoint.

use super::Page;
use crate::settings::endpoint::{EndpointForm, Test};
use crate::settings::{Settings, slots};
use crate::ui::{self, Kind, Notice};
use gpui_kit::{AnyElement, App, Entity, IntoElement, Window};
use hushpen_core::endpoint::{Preset, UrlError};

/// The form, while the AI source is the endpoint.
fn form(page: &Page, cx: &App) -> Option<Entity<EndpointForm>> {
    let me = page.settings.read(cx);
    me.endpoint
        .clone()
        .filter(|_| me.ai_provider() == "endpoint")
}

pub fn preset(page: &Page, cx: &mut App) -> Vec<AnyElement> {
    let Some(form) = form(page, cx) else {
        return Vec::new();
    };
    let preset = form.read(cx).preset();
    let menu = page.picker_open("cleanup.endpointPreset", cx).then(|| {
        let choices = Preset::ALL
            .into_iter()
            .map(|choice| {
                (
                    choice.key().to_owned(),
                    choice.label().to_owned(),
                    choice == preset,
                )
            })
            .collect();
        page.options(
            "cleanup.endpointPreset",
            choices,
            &page.options,
            |me: &mut Settings, key, cx| {
                if let Some(choice) = Preset::ALL.into_iter().find(|each| each.key() == key) {
                    me.set_endpoint_preset(choice, cx);
                }
            },
        )
    });
    vec![page.menu(
        "cleanup.endpointPreset",
        "Endpoint",
        "Ollama and LM Studio fill in the address for you.",
        preset.label().to_owned(),
        slots::ENDPOINT_PRESET,
        menu,
    )]
}

pub fn url(page: &Page, cx: &mut App) -> Vec<AnyElement> {
    form(page, cx)
        .map(|form| {
            ui::field_row(
                page.palette,
                super::settings_id("cleanup.endpointUrl"),
                "Address",
                UrlError::NeedsHttps.reason(),
                &form.read(cx).url,
                None,
            )
        })
        .into_iter()
        .collect()
}

pub fn model(page: &Page, cx: &mut App) -> Vec<AnyElement> {
    form(page, cx)
        .map(|form| {
            ui::field_row(
                page.palette,
                super::settings_id("cleanup.endpointModel"),
                "Model",
                "The model name the endpoint knows.",
                &form.read(cx).model,
                None,
            )
        })
        .into_iter()
        .collect()
}

pub fn key(page: &Page, cx: &mut App) -> Vec<AnyElement> {
    let Some(form) = form(page, cx) else {
        return Vec::new();
    };
    let (key_set, key) = {
        let me = form.read(cx);
        (me.key_is_set(), me.key.clone())
    };
    let clear = key_set.then(|| {
        let form = form.clone();
        ui::button(
            page.palette,
            super::settings_id("cleanup.endpointKeyClear"),
            "Remove",
            Kind::Plain,
            Some(&page.focus[slots::ENDPOINT_KEY_CLEAR]),
            true,
            move |_, cx| form.update(cx, |me, cx| me.clear_key(cx)),
        )
        .into_any_element()
    });
    vec![ui::field_row(
        page.palette,
        super::settings_id("cleanup.endpointKey"),
        "API key",
        if key_set {
            "Saved on this computer. Press Enter to replace it."
        } else {
            "Optional. Press Enter to save it on this computer."
        },
        &key,
        clear,
    )]
}

pub fn test(page: &Page, cx: &mut App) -> Vec<AnyElement> {
    let Some(form) = form(page, cx) else {
        return Vec::new();
    };
    let (test, text) = {
        let me = form.read(cx);
        (me.test.clone(), me.test_text())
    };
    let run = move |_: &mut Window, cx: &mut App| form.update(cx, |me, cx| me.test_connection(cx));
    vec![ui::notice_row(
        page.palette,
        super::settings_id("cleanup.endpointStatus"),
        text,
        Notice::status(matches!(test, Test::Failed(_))),
        Some(
            ui::button(
                page.palette,
                super::settings_id("cleanup.endpointTest"),
                "Test connection",
                Kind::Plain,
                Some(&page.focus[slots::ENDPOINT_TEST]),
                test != Test::Running,
                run,
            )
            .into_any_element(),
        ),
    )]
}
