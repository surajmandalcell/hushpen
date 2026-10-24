//! The Settings list in a window: the headings, the search field in the toolbar, the empty
//! result, Esc, Cmd+F or Ctrl+F, and the slider.

use super::tests::{Rig, read, rig, value};
use crate::shell::Shell;
use crate::theme;
use crate::views::View;
use gpui_kit::component::input::InputState;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Bounds, Entity, Focusable as _, Point, ScrollDelta,
    TestAppContext, WindowBounds, WindowOptions, point, px, size,
};
use serde_json::json;

struct Shown {
    rig: Rig,
    handle: AnyWindowHandle,
}

fn shown(cx: &mut TestAppContext) -> Shown {
    let rig = rig(cx);
    cx.update(gpui_kit::init);
    cx.update(theme::install);
    let (handle, shell) = cx.update(|cx| {
        let window = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(940.0), px(600.0)),
                })),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Shell::new(window, cx)),
        );
        window.expect("open test window")
    });
    let settings = rig.settings.clone();
    cx.update_window(handle, |_, window, cx| {
        settings.update(cx, |settings, cx| settings.attach_search(window, cx))
    })
    .unwrap();
    shell.update(cx, |shell, cx| {
        shell.attach_settings(rig.settings.clone(), cx);
        shell.select(View::Settings, cx);
    });
    let shown = Shown { rig, handle };
    frame(cx, &shown);
    shown
}

fn frame(cx: &mut TestAppContext, shown: &Shown) {
    cx.run_until_parked();
    cx.update_window(shown.handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn has(cx: &mut TestAppContext, shown: &Shown, id: &'static str) -> bool {
    cx.update_window(shown.handle, |_, window, _| window.try_find(id).is_some())
        .unwrap()
}

fn label(cx: &mut TestAppContext, shown: &Shown, id: &'static str) -> Option<String> {
    cx.update_window(shown.handle, |_, window, _| {
        window
            .try_find(id)
            .and_then(|found| found.label().map(str::to_owned))
    })
    .unwrap()
}

fn search(cx: &mut TestAppContext, shown: &Shown, query: &str) {
    let settings = shown.rig.settings.clone();
    cx.update_window(shown.handle, |_, window, cx| {
        settings.update(cx, |settings, cx| settings.search_for(query, window, cx))
    })
    .unwrap();
    frame(cx, shown);
}

fn press(cx: &mut TestAppContext, shown: &Shown, key: &str) {
    cx.update_window(shown.handle, |_, window, cx| window.press(key, cx))
        .unwrap();
    frame(cx, shown);
}

fn input(cx: &TestAppContext, shown: &Shown) -> Entity<InputState> {
    read(cx, &shown.rig, |me| me.search.clone()).expect("the search field is attached")
}

fn search_focused(cx: &mut TestAppContext, shown: &Shown) -> bool {
    let input = input(cx, shown);
    cx.update_window(shown.handle, |_, window, cx| {
        input.read(cx).focus_handle(cx).is_focused(window)
    })
    .unwrap()
}

#[gpui_kit::test]
fn every_section_has_a_heading_above_its_group_in_page_order(cx: &mut TestAppContext) {
    let shown = shown(cx);
    let headings: Vec<f32> = [
        "settings.general.heading",
        "settings.audio.heading",
        "settings.cleanup.heading",
        "settings.engine.heading",
        "settings.flowbar.heading",
        "settings.storage.heading",
        "settings.updates.heading",
    ]
    .into_iter()
    .map(|id| {
        cx.update_window(shown.handle, |_, window, _| {
            f32::from(window.find(id).bounds().origin.y)
        })
        .unwrap()
    })
    .collect();
    assert!(
        headings.windows(2).all(|pair| pair[0] < pair[1]),
        "{headings:?}"
    );
    assert_eq!(
        label(cx, &shown, "settings.audio.heading").as_deref(),
        Some("Audio")
    );
    assert!(!has(cx, &shown, "settings.sections"), "no tab bar");
    assert!(
        !has(cx, &shown, "settings.shortcuts.heading"),
        "no Shortcuts rows without the Shortcuts view"
    );
}

#[gpui_kit::test]
fn the_search_field_sits_at_the_right_of_the_toolbar_and_is_300_wide(cx: &mut TestAppContext) {
    let shown = shown(cx);
    let (field, toolbar) = cx
        .update_window(shown.handle, |_, window, _| {
            (
                window.find("settings.search").bounds(),
                window.find("toolbar.drag").bounds(),
            )
        })
        .unwrap();
    assert!(f32::from(field.size.width) >= 300.0, "{field:?}");
    let right_gap =
        f32::from(toolbar.origin.x + toolbar.size.width - field.origin.x - field.size.width);
    assert!(
        (right_gap - 24.0).abs() < 0.5,
        "the field ends at the 24 px margin: {right_gap}"
    );
    let center = f32::from(field.origin.y) + f32::from(field.size.height) / 2.0;
    assert!(
        (center - theme::size::CHROME_CENTER).abs() < 0.5,
        "on the chrome line: {center}"
    );
}

#[gpui_kit::test]
fn typing_filters_to_the_matching_rows_under_their_headings(cx: &mut TestAppContext) {
    let shown = shown(cx);
    search(cx, &shown, "input");
    assert!(has(cx, &shown, "settings.audio.mic"));
    assert!(has(cx, &shown, "settings.audio.heading"));
    assert!(!has(cx, &shown, "settings.audio.sounds"));
    assert!(!has(cx, &shown, "settings.general.heading"));
    assert!(!has(cx, &shown, "settings.general.launchAtLogin"));
    let state = cx.read(|app| shown.rig.settings.read(app).hook_state_json(app));
    assert_eq!(state["query"], "input");
    assert_eq!(state["rows"], json!(["audio.mic"]));

    search(cx, &shown, "autostart");
    assert!(has(cx, &shown, "settings.general.launchAtLogin"));
    assert!(!has(cx, &shown, "settings.audio.mic"));

    search(cx, &shown, "");
    assert!(has(cx, &shown, "settings.audio.mic"));
    assert!(has(cx, &shown, "settings.general.launchAtLogin"));
}

#[gpui_kit::test]
fn a_query_with_no_match_says_so_and_clear_brings_the_list_back(cx: &mut TestAppContext) {
    let shown = shown(cx);
    search(cx, &shown, "zebra");
    assert_eq!(
        label(cx, &shown, "settings.search.empty").as_deref(),
        Some("No settings match \"zebra\"")
    );
    assert!(!has(cx, &shown, "settings.general.heading"));
    cx.update_window(shown.handle, |_, window, cx| {
        window.click("settings.search.clear", cx)
    })
    .unwrap();
    frame(cx, &shown);
    assert_eq!(read(cx, &shown.rig, |me| me.query.clone()), "");
    assert_eq!(
        input(cx, &shown).read_with(cx, |input, _| input.value().to_string()),
        ""
    );
    assert!(!has(cx, &shown, "settings.search.empty"));
    assert!(has(cx, &shown, "settings.general.heading"));
}

#[gpui_kit::test]
fn secondary_f_focuses_the_search_and_esc_clears_then_leaves_it(cx: &mut TestAppContext) {
    let shown = shown(cx);
    assert!(!search_focused(cx, &shown));
    press(
        cx,
        &shown,
        if cfg!(target_os = "macos") {
            "cmd-f"
        } else {
            "ctrl-f"
        },
    );
    assert!(
        search_focused(cx, &shown),
        "Cmd+F or Ctrl+F puts the caret in the field"
    );

    cx.update_window(shown.handle, |_, window, cx| window.input("sounds", cx))
        .unwrap();
    frame(cx, &shown);
    assert_eq!(read(cx, &shown.rig, |me| me.query.clone()), "sounds");

    press(cx, &shown, "escape");
    assert_eq!(
        read(cx, &shown.rig, |me| me.query.clone()),
        "",
        "the first Esc clears"
    );
    assert!(search_focused(cx, &shown), "and keeps the caret");
    press(cx, &shown, "escape");
    assert!(!search_focused(cx, &shown), "the next Esc leaves the field");
}

#[gpui_kit::test]
fn a_new_query_puts_the_list_back_at_the_top(cx: &mut TestAppContext) {
    let shown = shown(cx);
    cx.update_window(shown.handle, |_, window, cx| {
        window.scroll(
            "content.scroll",
            ScrollDelta::Pixels(point(px(0.0), px(-400.0))),
            cx,
        )
    })
    .unwrap();
    frame(cx, &shown);
    let offset = || {
        read(cx, &shown.rig, |me| {
            me.scroll.as_ref().map(|scroll| scroll.offset().y)
        })
    };
    assert!(offset().is_some_and(|y| y < px(0.0)), "the list scrolled");
    search(cx, &shown, "updates");
    assert_eq!(
        read(cx, &shown.rig, |me| me
            .scroll
            .as_ref()
            .map(|s| s.offset().y)),
        Some(px(0.0))
    );
}

#[gpui_kit::test]
fn the_volume_slider_follows_a_press_and_the_arrow_keys(cx: &mut TestAppContext) {
    let shown = shown(cx);
    search(cx, &shown, "volume");
    cx.update_window(shown.handle, |_, window, cx| {
        window.click("settings.audio.cueVolume", cx)
    })
    .unwrap();
    frame(cx, &shown);
    assert_eq!(
        value(&shown.rig, "audio.cueVolume"),
        json!(0.5),
        "a press in the middle"
    );
    press(cx, &shown, "right");
    assert_eq!(value(&shown.rig, "audio.cueVolume"), json!(0.55));
    press(cx, &shown, "home");
    assert_eq!(value(&shown.rig, "audio.cueVolume"), json!(0.0));
}

#[gpui_kit::test]
fn the_length_picker_offers_the_range_of_the_store(cx: &mut TestAppContext) {
    let shown = shown(cx);
    search(cx, &shown, "maximum");
    cx.update_window(shown.handle, |_, window, cx| {
        window.click("settings.audio.maxMinutes", cx)
    })
    .unwrap();
    frame(cx, &shown);
    assert!(has(cx, &shown, "settings.audio.maxMinutes.option.2"));
    assert!(has(cx, &shown, "settings.audio.maxMinutes.option.60"));
    cx.update_window(shown.handle, |_, window, cx| {
        window.click("settings.audio.maxMinutes.option.30", cx)
    })
    .unwrap();
    frame(cx, &shown);
    assert_eq!(value(&shown.rig, "dictation.maxMinutes"), json!(30));
    assert_eq!(
        label(cx, &shown, "settings.audio.maxMinutes").as_deref(),
        Some("Maximum length: 30 minutes")
    );
}
