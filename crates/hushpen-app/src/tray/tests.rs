use super::*;
use crate::actions::*;
use crate::controller::testkit::*;
use crate::main_window::testkit::install_fake;
use gpui_kit::{AnyWindowHandle, AppContext as _, TestAppContext, WindowOptions};
use hushpen_core::dictation::State;
use std::rc::Rc;

fn names(listening: bool) -> Vec<String> {
    menu_items(listening)
        .into_iter()
        .map(|item| match item {
            MenuItem::Action { name, .. } => name.to_string(),
            _ => panic!("the tray menu holds only actions"),
        })
        .collect()
}

#[test]
fn the_menu_has_five_entries_in_the_promised_order() {
    assert_eq!(
        names(false),
        [
            "Start dictation",
            "Paste last transcript",
            "Show Hushpen",
            "Settings",
            "Quit"
        ]
    );
}

#[test]
fn only_the_first_entry_changes_while_a_dictation_listens() {
    let idle = names(false);
    let listening = names(true);
    assert_eq!(listening[0], "Stop dictation");
    assert_eq!(idle[1..], listening[1..]);
}

#[test]
fn the_hook_model_reports_the_same_labels() {
    let model = menu_json(true, true);
    let labels: Vec<_> = model["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["label"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(labels, names(true));
    assert_eq!(model["created"], true);
    assert_eq!(menu_json(false, false)["created"], false);
}

#[cfg(not(target_os = "macos"))]
#[test]
fn a_dark_panel_gets_the_white_glyph_and_a_light_panel_the_black_one() {
    assert_eq!(glyph_for(WindowAppearance::Dark), GLYPH_WHITE);
    assert_eq!(glyph_for(WindowAppearance::VibrantDark), GLYPH_WHITE);
    assert_eq!(glyph_for(WindowAppearance::Light), GLYPH_BLACK);
    assert_eq!(glyph_for(WindowAppearance::VibrantLight), GLYPH_BLACK);
}

#[cfg(target_os = "macos")]
#[test]
fn macos_always_gets_the_black_glyph_that_the_system_tints() {
    for appearance in [WindowAppearance::Dark, WindowAppearance::Light] {
        assert_eq!(glyph_for(appearance), GLYPH_BLACK);
    }
}

#[gpui_kit::test]
fn both_glyphs_become_32_px_icons(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    for appearance in [WindowAppearance::Dark, WindowAppearance::Light] {
        let icon = cx.update(|cx| icon_for(appearance, cx));
        assert!(icon.is_some(), "{appearance:?}");
    }
}

struct Fixture {
    rig: Rig,
    shell: Entity<Shell>,
    window: AnyWindowHandle,
    calls: Rc<crate::main_window::testkit::Calls>,
    /// The actions hold the bar weakly, so the test keeps it alive.
    _flow_bar: Entity<FlowBar>,
}

fn fixture(cx: &mut TestAppContext, files: &[&str]) -> Fixture {
    cx.update(gpui_kit::init);
    let rig = rig(cx, files, "base");
    let (window, shell) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| Shell::new(window, cx))
        })
        .expect("open test window")
    });
    let calls = cx.update(|cx| install_fake(cx, window, true));
    let flow_bar = cx.new(|_| {
        FlowBar::new(
            Rc::clone(&rig.storage),
            rig.controller.clone(),
            rig.dictation.clone(),
            rig.mic.clone(),
        )
    });
    let surfaces = Surfaces {
        controller: rig.controller.clone(),
        flow_bar: flow_bar.clone(),
        shell: shell.clone(),
    };
    cx.update(|cx| register_actions(cx, &surfaces));
    Fixture {
        rig,
        shell,
        window,
        calls,
        _flow_bar: flow_bar,
    }
}

/// What a click on a tray entry does: the action goes through GPUI's own dispatch.
fn click(cx: &mut TestAppContext, action: &dyn Action) {
    cx.update(|cx| cx.dispatch_action(action));
    cx.run_until_parked();
}

#[gpui_kit::test]
fn the_dictation_entry_starts_and_then_stops_a_dictation(cx: &mut TestAppContext) {
    let fixture = fixture(cx, &["base"]);
    say(&fixture.rig, outcome_text("hello there", "en"));

    fixture.rig.now.set(11_000);
    click(cx, &ToggleDictation);
    assert_eq!(machine_state(cx, &fixture.rig), State::Listening);
    assert!(cx.update(|cx| listening(&fixture.rig.controller, cx)));

    fixture.rig.now.set(14_000);
    click(cx, &ToggleDictation);
    settle(cx, &fixture.rig);
    assert_eq!(machine_state(cx, &fixture.rig), State::Done);
    assert!(!cx.update(|cx| listening(&fixture.rig.controller, cx)));
}

#[gpui_kit::test]
fn a_start_that_preflight_refuses_leaves_the_pipeline_idle(cx: &mut TestAppContext) {
    let fixture = fixture(cx, &[]);
    fixture.rig.now.set(11_000);
    click(cx, &ToggleDictation);
    assert_eq!(machine_state(cx, &fixture.rig), State::Idle);
    assert_eq!(
        menu_json(false, true)["items"][0]["label"],
        "Start dictation"
    );
}

#[gpui_kit::test]
fn the_paste_entry_sends_paste_last_into_the_pipeline(cx: &mut TestAppContext) {
    let fixture = fixture(cx, &["base"]);
    click(cx, &PasteLastTranscript);
    settle(cx, &fixture.rig);
    let code = fixture
        .rig
        .controller
        .read_with(cx, |c, _| c.notice().map(|notice| notice.code));
    assert_eq!(code, Some("INSERT_NO_TRANSCRIPT"));
}

#[gpui_kit::test]
fn the_settings_entry_opens_settings_and_shows_the_window(cx: &mut TestAppContext) {
    let fixture = fixture(cx, &["base"]);
    cx.update(crate::main_window::step_aside);
    click(cx, &ShowSettings);
    assert_eq!(
        fixture.shell.read_with(cx, |shell, _| shell.active()),
        View::Settings
    );
    assert_eq!(fixture.calls.shown.get(), 1);
    assert_eq!(cx.update(|cx| crate::main_window::away(cx)), None);
}

#[gpui_kit::test]
fn the_show_entry_brings_the_window_back_on_the_view_it_had(cx: &mut TestAppContext) {
    let fixture = fixture(cx, &["base"]);
    fixture
        .shell
        .update(cx, |shell, cx| shell.select(View::Models, cx));
    cx.update(crate::main_window::step_aside);
    click(cx, &ShowHushpen);
    assert_eq!(fixture.calls.shown.get(), 1);
    assert_eq!(
        fixture.shell.read_with(cx, |shell, _| shell.active()),
        View::Models
    );
    let _ = fixture.window;
}
