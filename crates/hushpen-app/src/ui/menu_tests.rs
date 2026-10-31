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

fn type_text(cx: &mut TestAppContext, home: &Home, text: &str) {
    act(cx, home, |window, cx| window.input(text, cx));
}

fn present(cx: &mut TestAppContext, home: &Home, id: &str) -> bool {
    find(cx, home, id).is_some()
}

const SEARCH: &str = "home.mic.list.search";

#[gpui_kit::test]
fn a_long_menu_opens_with_a_focused_search_field_the_width_of_the_menu(cx: &mut TestAppContext) {
    let home = home(cx);
    open_mics(cx, &home);
    assert!(focused(cx, &home, SEARCH), "the search field has the focus");
    let list = bounds(cx, &home, "home.mic.list");
    let field = bounds(cx, &home, SEARCH);
    assert_eq!(field.size.height, px(size::BUTTON_HEIGHT));
    assert_eq!(field.size.width, list.size.width, "as wide as the options");
    assert!(field.bottom() <= list.top(), "it sits above the options");
    assert_eq!(
        find(cx, &home, SEARCH).unwrap().label(),
        Some("Search options")
    );
}

#[gpui_kit::test]
fn a_short_menu_has_no_search_field(cx: &mut TestAppContext) {
    let home = home(cx);
    let few = (1..=size::MENU_OPTIONS_SHOWN - 1)
        .map(|n| InputDevice {
            id: format!("mic{n}"),
            name: format!("Microphone {n}"),
            is_default: n == 1,
        })
        .collect();
    home.rig
        .mic
        .update(cx, |mic, cx| mic.handle(MicEvent::Devices(Ok(few)), cx));
    open_mics(cx, &home);
    assert!(present(cx, &home, "home.mic.list"));
    assert!(!present(cx, &home, SEARCH));
    assert!(!focused(cx, &home, SEARCH));
}

#[gpui_kit::test]
fn typing_keeps_the_options_that_have_the_text_in_any_case(cx: &mut TestAppContext) {
    let home = home(cx);
    open_mics(cx, &home);
    type_text(cx, &home, "MICROPHONE 1");
    for n in [1, 10, 11, 12] {
        assert!(present(cx, &home, &mic(n)), "Microphone {n} stays");
    }
    for n in [2, 9] {
        assert!(!present(cx, &home, &mic(n)), "Microphone {n} goes");
    }
    type_text(cx, &home, "2");
    assert!(present(cx, &home, &mic(12)));
    assert!(!present(cx, &home, &mic(11)));
    assert!(!present(cx, &home, "home.mic.list.empty"));
}

#[gpui_kit::test]
fn the_menu_keeps_its_width_while_a_query_leaves_shorter_options(cx: &mut TestAppContext) {
    let home = home(cx);
    let mics = (1..=MICS)
        .map(|n| InputDevice {
            id: format!("mic{n}"),
            name: format!("Studio condenser microphone number {n}"),
            is_default: n == 1,
        })
        .collect();
    home.rig
        .mic
        .update(cx, |mic, cx| mic.handle(MicEvent::Devices(Ok(mics)), cx));
    open_mics(cx, &home);
    act(cx, &home, |_, _| {});
    let before = bounds(cx, &home, "home.mic.list").size.width;
    assert!(
        before > px(size::MENU_MIN_WIDTH + 20.0),
        "the names make it wide"
    );
    type_text(cx, &home, "zebra");
    act(cx, &home, |_, _| {});
    assert_eq!(bounds(cx, &home, "home.mic.list").size.width, before);
    press(cx, &home, "escape");
    type_text(cx, &home, "number 12");
    act(cx, &home, |_, _| {});
    assert_eq!(bounds(cx, &home, "home.mic.list").size.width, before);
}

#[gpui_kit::test]
fn the_arrow_keys_and_enter_work_on_the_filtered_list(cx: &mut TestAppContext) {
    let home = home(cx);
    open_mics(cx, &home);
    type_text(cx, &home, "microphone 1");
    press(cx, &home, "down");
    assert!(focused(cx, &home, &mic(0)), "Down starts at the chosen one");
    press(cx, &home, "down");
    assert!(focused(cx, &home, &mic(1)));
    press(cx, &home, "down");
    assert!(
        focused(cx, &home, &mic(10)),
        "the next one left, not Microphone 2"
    );
    press(cx, &home, "up");
    press(cx, &home, "up");
    assert!(focused(cx, &home, &mic(0)));
    press(cx, &home, "up");
    assert!(focused(cx, &home, SEARCH), "Up from the first goes back");
    press(cx, &home, "down");
    press(cx, &home, "down");
    press(cx, &home, "down");
    press(cx, &home, "enter");
    assert_eq!(saved(cx, &home), "mic10");
    assert!(!present(cx, &home, "home.mic.list"));
    assert!(focused(cx, &home, "home.mic.picker"));
}

#[gpui_kit::test]
fn enter_in_the_search_field_picks_the_first_option_left(cx: &mut TestAppContext) {
    let home = home(cx);
    open_mics(cx, &home);
    press(cx, &home, "enter");
    assert!(
        present(cx, &home, "home.mic.list"),
        "with nothing typed, Enter picks nothing"
    );
    type_text(cx, &home, "11");
    press(cx, &home, "enter");
    assert_eq!(saved(cx, &home), "mic11");
    assert!(!present(cx, &home, "home.mic.list"));
    assert!(focused(cx, &home, "home.mic.picker"));
}

#[gpui_kit::test]
fn esc_clears_the_query_first_and_closes_the_menu_the_second_time(cx: &mut TestAppContext) {
    let home = home(cx);
    open_mics(cx, &home);
    type_text(cx, &home, "12");
    assert!(!present(cx, &home, &mic(2)));
    press(cx, &home, "escape");
    assert!(
        present(cx, &home, "home.mic.list"),
        "the first Esc keeps it open"
    );
    assert!(present(cx, &home, &mic(2)), "every option is back");
    assert!(focused(cx, &home, SEARCH));
    press(cx, &home, "escape");
    assert!(!present(cx, &home, "home.mic.list"));
    assert!(focused(cx, &home, "home.mic.picker"));
    assert_eq!(saved(cx, &home), "default", "Esc picks nothing");
}

#[gpui_kit::test]
fn a_query_with_no_option_shows_one_disabled_row(cx: &mut TestAppContext) {
    let home = home(cx);
    open_mics(cx, &home);
    type_text(cx, &home, "zebra");
    assert!(!present(cx, &home, &mic(1)));
    let row = bounds(cx, &home, "home.mic.list.empty");
    assert_eq!(row.size.height, px(size::MENU_ROW_HEIGHT));
    press(cx, &home, "down");
    press(cx, &home, "enter");
    assert!(present(cx, &home, "home.mic.list"), "nothing to pick");
    assert_eq!(saved(cx, &home), "default");
    click(cx, &home, "home.mic.list.empty");
    assert!(
        present(cx, &home, "home.mic.list"),
        "the row is not a choice"
    );
}

#[gpui_kit::test]
fn the_menu_opens_again_with_an_empty_search(cx: &mut TestAppContext) {
    let home = home(cx);
    open_mics(cx, &home);
    type_text(cx, &home, "12");
    press(cx, &home, "escape");
    press(cx, &home, "escape");
    open_mics(cx, &home);
    assert!(present(cx, &home, &mic(2)), "no query is kept");
    assert!(focused(cx, &home, SEARCH));
    type_text(cx, &home, "12");
    click(cx, &home, "home.mic.picker");
    open_mics(cx, &home);
    assert!(
        present(cx, &home, &mic(2)),
        "a closed menu forgets the query"
    );
}

#[gpui_kit::test]
fn shift_tab_in_the_search_field_closes_the_menu_and_goes_back_a_control(cx: &mut TestAppContext) {
    let home = home(cx);
    open_mics(cx, &home);
    type_text(cx, &home, "12");
    press(cx, &home, "shift-tab");
    assert!(!present(cx, &home, "home.mic.list"));
    assert!(
        !focused(cx, &home, "home.mic.record"),
        "not the next control"
    );
    assert!(
        !focused(cx, &home, "sidebar.home"),
        "no wrap to the sidebar"
    );
    assert_eq!(saved(cx, &home), "default");
}
