//! The pop-up menu in a real window, through the Home pop-ups: it floats over the page and out
//! of its group, shows eight whole options and scrolls, follows the arrow keys, and a choice,
//! Esc, or a press outside closes it with the focus back on its button. Tab closes it too.

use crate::controller::testkit::{self, Rig};
use crate::mic::MicEvent;
use crate::shell::Shell;
use crate::theme::{size, space};
use gpui_kit::test::{ElementSnapshot, TestWindowExt as _};
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Point, ScrollDelta, SharedString,
    TestAppContext, Window, WindowBounds, WindowOptions, point, px,
};
use hushpen_audio::InputDevice;

/// More microphones than the menu shows at once.
const MICS: usize = 12;

struct Home {
    rig: Rig,
    handle: AnyWindowHandle,
}

/// Home in a window, with `MICS` microphones; the first is the system default.
fn home(cx: &mut TestAppContext) -> Home {
    let rig = testkit::rig(cx, &["base"], "base");
    let mics = (1..=MICS)
        .map(|n| InputDevice {
            id: format!("mic{n}"),
            name: format!("Microphone {n}"),
            is_default: n == 1,
        })
        .collect();
    rig.mic
        .update(cx, |mic, cx| mic.handle(MicEvent::Devices(Ok(mics)), cx));
    cx.update(gpui_kit::init);
    let (handle, shell) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: gpui_kit::size(px(space::WINDOW_WIDTH), px(space::WINDOW_HEIGHT)),
                })),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Shell::new(window, cx)),
        )
        .expect("open test window")
    });
    shell.update(cx, |shell, cx| {
        shell.attach_mic(rig.mic.clone(), cx);
        shell.attach_dictation(rig.dictation.clone(), cx);
    });
    let home = Home { rig, handle };
    act(cx, &home, |_, _| {});
    home
}

/// Runs `run` in the window, then lets the app settle and draws a frame.
fn act(cx: &mut TestAppContext, home: &Home, run: impl FnOnce(&mut Window, &mut App)) {
    cx.update_window(home.handle, |_, window, cx| run(window, cx))
        .unwrap();
    cx.run_until_parked();
    cx.update_window(home.handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn find(cx: &mut TestAppContext, home: &Home, id: &str) -> Option<ElementSnapshot> {
    let id = SharedString::from(id.to_owned());
    cx.update_window(home.handle, |_, window, _| window.try_find(id))
        .unwrap()
}

fn bounds(cx: &mut TestAppContext, home: &Home, id: &str) -> Bounds<gpui_kit::Pixels> {
    find(cx, home, id)
        .unwrap_or_else(|| panic!("{id} is shown"))
        .bounds()
}

fn focused(cx: &mut TestAppContext, home: &Home, id: &str) -> bool {
    find(cx, home, id).and_then(|found| found.focused()) == Some(true)
}

/// Option `n` of the microphone menu: Default at 0, then each microphone.
fn mic(n: usize) -> String {
    if n == 0 {
        "home.mic.device.default".to_owned()
    } else {
        format!("home.mic.device.{n}")
    }
}

fn saved(cx: &mut TestAppContext, home: &Home) -> String {
    home.rig.mic.read_with(cx, |mic, _| mic.saved().to_owned())
}

fn click(cx: &mut TestAppContext, home: &Home, id: &str) {
    let id = SharedString::from(id.to_owned());
    act(cx, home, |window, cx| window.click(id, cx));
}

fn press(cx: &mut TestAppContext, home: &Home, key: &str) {
    act(cx, home, |window, cx| window.press(key, cx));
}

fn open_mics(cx: &mut TestAppContext, home: &Home) {
    click(cx, home, "home.mic.picker");
}

/// The menu drops a few pixels into place as it opens, and it does so on each opening.
#[gpui_kit::test]
fn the_menu_drops_into_place_each_time_it_opens(cx: &mut TestAppContext) {
    use crate::motion;
    let home = home(cx);
    cx.update(|cx| cx.set_global(motion::tests::Play));
    open_mics(cx, &home);
    let opening = bounds(cx, &home, "home.mic.list").top();
    cx.executor().advance_clock(motion::FAST);
    act(cx, &home, |window, cx| {
        window.simulate_next_frame(cx);
    });
    let settled = bounds(cx, &home, "home.mic.list").top();
    assert_eq!(settled - opening, px(motion::TRAVEL), "it starts higher");
    press(cx, &home, "escape");
    assert!(find(cx, &home, "home.mic.list").is_none());
    open_mics(cx, &home);
    assert_eq!(
        bounds(cx, &home, "home.mic.list").top(),
        opening,
        "it drops in again"
    );
}

#[gpui_kit::test]
fn the_microphone_menu_floats_out_of_its_group_with_eight_whole_options(cx: &mut TestAppContext) {
    let home = home(cx);
    open_mics(cx, &home);
    let group = bounds(cx, &home, "home.mic");
    let button = bounds(cx, &home, "home.mic.picker");
    let list = bounds(cx, &home, "home.mic.list");
    assert_eq!(list.size.height, px(size::MENU_LIST_MAX_HEIGHT));
    assert!(list.top() > button.bottom(), "it opens under its button");
    let inset = button.right() - list.right();
    assert!(
        inset > px(0.0) && inset < px(size::MENU_PADDING + 1.0),
        "its right edge lines up with the button: {inset:?}"
    );
    assert!(
        list.bottom() < px(space::WINDOW_HEIGHT),
        "it stays in the window"
    );

    let rows: Vec<ElementSnapshot> = (0..=8).map(|n| find(cx, &home, &mic(n)).unwrap()).collect();
    for (n, row) in rows[..8].iter().enumerate() {
        let row = row.bounds();
        assert_eq!(row.size.height, px(size::MENU_ROW_HEIGHT));
        assert!(
            row.top() >= list.top() && row.bottom() <= list.bottom(),
            "option {n} shows whole"
        );
    }
    assert!(rows[..8].iter().all(ElementSnapshot::visible));
    assert!(
        rows[7].bounds().bottom() > group.bottom(),
        "the group does not cut the menu"
    );
    assert!(!rows[8].visible(), "the ninth option waits for a scroll");
    assert_eq!(rows[0].selected(), Some(true), "Default is the chosen one");
}

#[gpui_kit::test]
fn the_list_scrolls_to_the_last_microphone_and_a_choice_closes_it(cx: &mut TestAppContext) {
    let home = home(cx);
    open_mics(cx, &home);
    act(cx, &home, |window, cx| {
        let down = ScrollDelta::Pixels(point(px(0.0), px(-1000.0)));
        window.scroll("home.mic.list", down, cx)
    });
    let list = bounds(cx, &home, "home.mic.list");
    let last = find(cx, &home, &mic(MICS)).unwrap();
    assert!(last.visible());
    assert_eq!(last.bounds().bottom(), list.bottom(), "the end of the list");

    click(cx, &home, &mic(MICS));
    assert_eq!(saved(cx, &home), format!("mic{MICS}"));
    assert!(
        find(cx, &home, "home.mic.list").is_none(),
        "a choice closes it"
    );
    assert!(focused(cx, &home, "home.mic.picker"));
    assert_eq!(
        find(cx, &home, "home.mic.picker").unwrap().label(),
        Some(format!("Microphone: Microphone {MICS}").as_str())
    );
}

#[gpui_kit::test]
fn the_menu_opens_with_the_chosen_microphone_in_view(cx: &mut TestAppContext) {
    let home = home(cx);
    home.rig
        .mic
        .update(cx, |mic, cx| mic.select(&format!("mic{MICS}"), cx))
        .unwrap();
    open_mics(cx, &home);
    let list = bounds(cx, &home, "home.mic.list");
    let chosen = find(cx, &home, &mic(MICS)).unwrap();
    assert_eq!(chosen.selected(), Some(true));
    assert!(chosen.visible());
    assert!(chosen.bounds().top() >= list.top() && chosen.bounds().bottom() <= list.bottom());
}

#[gpui_kit::test]
fn the_arrow_keys_walk_the_options_in_view_and_enter_picks(cx: &mut TestAppContext) {
    let home = home(cx);
    open_mics(cx, &home);
    press(cx, &home, "down");
    assert!(
        focused(cx, &home, &mic(0)),
        "the first Down reaches the chosen one"
    );
    for _ in 0..10 {
        press(cx, &home, "down");
    }
    press(cx, &home, "up");
    assert!(focused(cx, &home, &mic(9)));
    let list = bounds(cx, &home, "home.mic.list");
    for n in [9, 10] {
        let row = bounds(cx, &home, &mic(n));
        assert!(
            row.top() >= list.top() && row.bottom() <= list.bottom(),
            "the list scrolled with the keys to option {n}"
        );
    }

    press(cx, &home, "enter");
    assert_eq!(saved(cx, &home), "mic9");
    assert!(find(cx, &home, "home.mic.list").is_none());
    assert!(focused(cx, &home, "home.mic.picker"));
}

#[gpui_kit::test]
fn esc_closes_the_menu_and_the_button_keeps_the_focus(cx: &mut TestAppContext) {
    let home = home(cx);
    open_mics(cx, &home);
    press(cx, &home, "down");
    press(cx, &home, "escape");
    assert!(find(cx, &home, "home.mic.list").is_none());
    assert!(focused(cx, &home, "home.mic.picker"));
    assert_eq!(saved(cx, &home), "default", "Esc picks nothing");
}

#[gpui_kit::test]
fn tab_closes_the_menu_and_moves_on(cx: &mut TestAppContext) {
    let home = home(cx);
    open_mics(cx, &home);
    press(cx, &home, "tab");
    assert!(find(cx, &home, "home.mic.list").is_none());
    assert!(focused(cx, &home, "home.mic.record"));
}

#[gpui_kit::test]
fn a_press_outside_only_closes_the_menu(cx: &mut TestAppContext) {
    let home = home(cx);
    open_mics(cx, &home);
    press(cx, &home, "down");
    click(cx, &home, "home.mic.record");
    assert!(find(cx, &home, "home.mic.list").is_none());
    assert_eq!(
        home.rig
            .mic_backend
            .starts
            .load(std::sync::atomic::Ordering::SeqCst),
        0,
        "the press does not reach the button under it"
    );
    assert!(focused(cx, &home, "home.mic.picker"));

    open_mics(cx, &home);
    click(cx, &home, "home.mic.picker");
    assert!(
        find(cx, &home, "home.mic.list").is_none(),
        "a second press of the button closes it"
    );
}

#[gpui_kit::test]
fn the_language_menu_floats_over_the_microphone_group(cx: &mut TestAppContext) {
    let home = home(cx);
    click(cx, &home, "home.language");
    let list = bounds(cx, &home, "home.language.list");
    let dictation = bounds(cx, &home, "home.dictation");
    assert_eq!(list.size.height, px(size::MENU_LIST_MAX_HEIGHT));
    assert!(
        list.bottom() > dictation.bottom(),
        "the group does not cut it"
    );
    let auto = find(cx, &home, "home.language.option.auto").unwrap();
    assert!(auto.visible());
    assert_eq!(auto.selected(), Some(true));

    press(cx, &home, "escape");
    assert!(find(cx, &home, "home.language.list").is_none());
    assert!(focused(cx, &home, "home.language"));
}
