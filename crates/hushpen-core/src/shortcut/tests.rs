use super::*;
use crate::dictation::AppEvent;

const MAC: Platform = Platform::MacOs;
const LINUX: Platform = Platform::Linux;

fn combo(text: &str, platform: Platform) -> Combo {
    Combo::parse(text, platform).unwrap_or_else(|problem| panic!("{text}: {problem:?}"))
}

fn ctrl_alt(letter: char) -> Combo {
    Combo::new(
        [
            Key::Any(Modifier::Ctrl),
            Key::Any(Modifier::Alt),
            Key::Char(letter),
        ],
        LINUX,
    )
    .unwrap()
}

#[test]
fn the_factory_defaults_parse_on_their_own_system_and_print_back_the_same() {
    for platform in [MAC, LINUX] {
        for slot in Slot::ALL {
            let text = slot.default_text(platform);
            match slot {
                Slot::HandsFree => {
                    let parsed = HandsFree::parse(text, platform).unwrap();
                    assert_eq!(parsed, HandsFree::Default);
                    assert_eq!(parsed.to_setting(platform), text);
                }
                _ => assert_eq!(combo(text, platform).to_setting(platform), text, "{slot:?}"),
            }
        }
    }
}

#[test]
fn macos_accepts_right_option_and_fn_as_modifier_only_keys() {
    assert_eq!(
        combo("RightOption", MAC).keys(),
        [Key::Right(Modifier::Alt)]
    );
    assert_eq!(combo("Fn", MAC).keys(), [Key::Fn]);
    assert_eq!(combo("Fn", MAC).display(MAC), "Fn");
    assert_eq!(combo("RightOption", MAC).display(MAC), "Right ⌥");
    assert_eq!(
        combo("RightOption+RightShift", MAC).display(MAC),
        "Right ⌥+Right ⇧"
    );
}

#[test]
fn macos_chords_read_with_the_mac_symbols() {
    assert_eq!(combo("Ctrl+Cmd+V", MAC).display(MAC), "⌃⌘V");
    assert_eq!(combo("Ctrl+Option+7", MAC).display(MAC), "⌃⌥7");
    assert_eq!(combo("Ctrl+Option+7", MAC).to_setting(MAC), "Ctrl+Option+7");
}

#[test]
fn linux_names_use_words_and_no_mac_symbols_or_option_and_command() {
    for text in [
        "RightAlt",
        "RightCtrl",
        "RightAlt+RightShift",
        "Ctrl+Alt+V",
        "Ctrl+Alt+Shift+Super+9",
    ] {
        let shown = combo(text, LINUX).display(LINUX);
        for banned in ["⌘", "⌥", "⌃", "⇧", "Option", "Command"] {
            assert!(!shown.contains(banned), "{shown} has {banned}");
        }
    }
    assert_eq!(combo("RightAlt", LINUX).display(LINUX), "Right Alt");
    assert_eq!(combo("Ctrl+Alt+V", LINUX).display(LINUX), "Ctrl+Alt+V");
    assert_eq!(
        combo("RightAlt+RightShift", LINUX).display(LINUX),
        "Right Alt+Right Shift"
    );
    assert_eq!(combo("Ctrl+Super+V", LINUX).display(LINUX), "Ctrl+Super+V");
}

#[test]
fn the_default_hands_free_names_the_hold_key_of_each_system() {
    let linux = HandsFree::Default.display(&combo("RightAlt", LINUX), LINUX);
    assert_eq!(linux, "Right Alt ×2 or Right Alt+Space");
    let mac = HandsFree::Default.display(&combo("RightOption", MAC), MAC);
    assert_eq!(mac, "Right ⌥ ×2 or Right ⌥+Space");
}

#[test]
fn right_ctrl_is_a_modifier_only_key_on_both_systems() {
    for platform in [MAC, LINUX] {
        let parsed = combo("RightCtrl", platform);
        assert_eq!(parsed.keys(), [Key::Right(Modifier::Ctrl)]);
        assert_eq!(parsed.to_setting(platform), "RightCtrl");
        assert_eq!(combo("Right Ctrl", platform), parsed);
    }
    assert_eq!(combo("RightCtrl", LINUX).display(LINUX), "Right Ctrl");
    assert_eq!(combo("RightCtrl", MAC).display(MAC), "Right ⌃");
}

#[test]
fn a_chord_parses_and_prints_back_to_the_same_value_on_both_systems() {
    for (platform, text) in [
        (LINUX, "Ctrl+Alt+H"),
        (LINUX, "Ctrl+Alt+Shift+9"),
        (LINUX, "RightCtrl+RightShift"),
        (LINUX, "Ctrl+Super+P"),
        (MAC, "Ctrl+Cmd+V"),
        (MAC, "Ctrl+Option+Shift+Cmd+K"),
        (MAC, "Fn"),
        (MAC, "RightCmd"),
    ] {
        assert_eq!(combo(text, platform).to_setting(platform), text);
    }
}

#[test]
fn setting_text_parses_in_any_order_case_and_spacing_and_alias() {
    let want = combo("Ctrl+Alt+Shift+P", LINUX);
    for text in [
        "shift+CTRL+alt+P",
        "Control + Option + shift + p",
        "ctrl+opt+Shift+P",
        "Alt+Shift+Ctrl+p",
    ] {
        assert_eq!(combo(text, LINUX), want, "{text}");
    }
    assert_eq!(combo("Super+Ctrl+v", LINUX), combo("Cmd+Ctrl+V", MAC));
}

#[test]
fn only_text_that_names_no_key_is_not_a_shortcut() {
    for (text, expected) in [
        ("", Problem::BadKey),
        ("Ctrl+F36", Problem::BadKey),
        ("Ctrl+Nope", Problem::BadKey),
        ("KeypadA", Problem::BadKey),
        ("Fn", Problem::NoFn),
    ] {
        assert_eq!(Combo::parse(text, LINUX).err(), Some(expected), "{text:?}");
    }
    assert_eq!(
        Combo::parse("Fn+Ctrl+Alt+Shift+Cmd+V+B", MAC).err(),
        Some(Problem::TooMany)
    );
    assert_eq!(Combo::parse("Fn", MAC).err(), None);
    // A repeated modifier is one key.
    assert_eq!(combo("Ctrl+Ctrl+V", LINUX), combo("Ctrl+V", LINUX));
}

/// Hushpen keeps no list of allowed chords: a bare key, a plain modifier, both sides of one
/// modifier, two keys, and the chords every app uses are all shortcuts.
#[test]
fn every_chord_the_keyboard_can_press_is_a_shortcut() {
    for text in [
        "V",
        "Shift+V",
        "Alt",
        "Ctrl+Shift",
        "Ctrl+RightCtrl",
        "Ctrl+V+B",
        "Ctrl+Tab+V",
        "Tab",
        "Space",
        "Ctrl+C",
        "Ctrl+Q",
        "Super+L",
        "F35",
        "CapsLock",
        "PlayPause",
        "Keypad5",
        "Key4242",
    ] {
        let parsed = combo(text, LINUX);
        assert_eq!(combo(&parsed.to_setting(LINUX), LINUX), parsed, "{text}");
    }
    for text in ["Cmd+Q", "Cmd+C", "Cmd+Shift+3", "Ctrl+A", "Cmd", "Shift"] {
        let parsed = combo(text, MAC);
        assert_eq!(combo(&parsed.to_setting(MAC), MAC), parsed, "{text}");
    }
}

#[test]
fn media_keypad_lock_and_unnamed_keys_have_names_on_both_systems() {
    for (text, mac, linux) in [
        ("PlayPause", "Play/Pause", "Play/Pause"),
        ("Cmd+VolumeUp", "⌘+Volume Up", "Super+Volume Up"),
        ("Mute", "Mute", "Mute"),
        ("CapsLock", "⇪", "Caps Lock"),
        ("Keypad1", "Keypad 1", "Keypad 1"),
        ("Ctrl+KeypadPlus", "⌃+Keypad +", "Ctrl+Keypad +"),
        ("KeypadEnter", "⌤", "Keypad Enter"),
        ("Ctrl+Escape", "⌃⎋", "Ctrl+Esc"),
        ("F13", "F13", "F13"),
        ("Ctrl+RightCtrl", "Left ⌃+Right ⌃", "Left Ctrl+Right Ctrl"),
        ("Key4242", "Key 4242", "Key 4242"),
    ] {
        assert_eq!(combo(text, MAC).display(MAC), mac, "{text}");
        assert_eq!(combo(text, LINUX).display(LINUX), linux, "{text}");
    }
    assert_eq!(
        combo("Ctrl+KeypadMinus", LINUX).keys(),
        [Key::Any(Modifier::Ctrl), Key::Named(Named::Keypad('-'))]
    );
    assert_eq!(combo("Key179", MAC).main(), Some(Key::Code(179)));
    assert!(Named::all().all(|named| {
        let text = combo(&format!("Ctrl+{}", super::names_setting(named)), LINUX);
        text.main() == Some(Key::Named(named))
    }));
}

#[test]
fn any_key_of_the_keyboard_can_be_the_key_of_a_shortcut() {
    for (text, shown) in [
        ("Cmd+Tab", "⌘⇥"),
        ("Cmd+`", "⌘`"),
        ("Shift+Cmd+`", "⇧⌘`"),
        ("Ctrl+Space", "⌃Space"),
        ("Cmd+Return", "⌘↩"),
        ("Option+Backspace", "⌥⌫"),
        ("Ctrl+Option+Left", "⌃⌥←"),
        ("Cmd+PageDown", "⌘⇟"),
        ("Cmd+-", "⌘-"),
        ("Cmd+Plus", "⌘+"),
        ("Ctrl+[", "⌃["),
        ("F5", "F5"),
        ("Shift+F12", "⇧F12"),
        ("Cmd+RightCtrl", "⌘+Right ⌃"),
        ("Fn+Delete", "Fn+⌦"),
    ] {
        let parsed = combo(text, MAC);
        assert_eq!(parsed.display(MAC), shown, "{text}");
        assert_eq!(combo(&parsed.to_setting(MAC), MAC), parsed, "{text}");
    }
    for (text, shown) in [
        ("Ctrl+Tab", "Ctrl+Tab"),
        ("Super+`", "Super+`"),
        ("Ctrl+Alt+Left", "Ctrl+Alt+Left"),
        ("Ctrl+PageDown", "Ctrl+Page Down"),
        ("Alt+Return", "Alt+Enter"),
        ("Ctrl+Plus", "Ctrl+Plus"),
        ("Super+RightCtrl", "Super+Right Ctrl"),
        ("F9", "F9"),
    ] {
        let parsed = combo(text, LINUX);
        assert_eq!(parsed.display(LINUX), shown, "{text}");
        assert_eq!(combo(&parsed.to_setting(LINUX), LINUX), parsed, "{text}");
    }
    assert_eq!(combo("Cmd+Tab", MAC).main(), Some(Key::Named(Named::Tab)));
    assert_eq!(combo("Cmd+`", MAC).main(), Some(Key::Char('`')));
    assert_eq!(combo("Cmd+RightCtrl", MAC).main(), None);
    assert_eq!(combo("Ctrl+ü", LINUX).main(), Some(Key::Char('ü')));
    assert_eq!(combo("Ctrl+Ü", LINUX), combo("Ctrl+ü", LINUX));
}

#[test]
fn every_problem_has_a_message_and_a_linux_one_has_no_mac_words() {
    for problem in [
        Problem::Empty,
        Problem::TooMany,
        Problem::NoFn,
        Problem::BadKey,
    ] {
        let text = problem.message(LINUX);
        assert!(!text.is_empty());
        assert!(
            !text.contains("Option") && !text.contains("Command"),
            "{text}"
        );
    }
}

#[test]
fn hands_free_is_the_default_gestures_or_one_recorded_shortcut() {
    assert_eq!(
        HandsFree::parse("doubletap+hold+space", LINUX).unwrap(),
        HandsFree::Default
    );
    let custom = HandsFree::parse("Ctrl+Alt+H", LINUX).unwrap();
    assert_eq!(custom, HandsFree::Custom(ctrl_alt('h')));
    assert_eq!(custom.to_setting(LINUX), "Ctrl+Alt+H");
    assert_eq!(
        custom.display(&combo("RightAlt", LINUX), LINUX),
        "Ctrl+Alt+H"
    );
    assert!(HandsFree::parse("nonsense here", LINUX).is_err());
}

#[test]
fn slots_have_stable_names_and_setting_keys() {
    let keys: Vec<_> = Slot::ALL.iter().map(|slot| slot.setting_key()).collect();
    assert_eq!(
        keys,
        [
            "shortcut.hold",
            "shortcut.handsFree",
            "shortcut.pasteLast",
            "shortcut.command",
            "shortcut.readAloud"
        ]
    );
    for slot in Slot::ALL {
        assert_eq!(Slot::from_key(slot.key()), Some(slot));
    }
}

// The engine.

const R_ALT: Phys = Phys::Modifier(Modifier::Alt, Side::Right);
const L_ALT: Phys = Phys::Modifier(Modifier::Alt, Side::Left);
const L_CTRL: Phys = Phys::Modifier(Modifier::Ctrl, Side::Left);
const R_CTRL: Phys = Phys::Modifier(Modifier::Ctrl, Side::Right);
const R_SHIFT: Phys = Phys::Modifier(Modifier::Shift, Side::Right);
const L_SHIFT: Phys = Phys::Modifier(Modifier::Shift, Side::Left);

fn defaults() -> Bindings {
    Bindings {
        hold: Some(combo("RightAlt", LINUX)),
        hands_free: Some(HandsFree::Default),
        paste_last: Some(combo("Ctrl+Alt+V", LINUX)),
        command: Some(combo("RightAlt+RightShift", LINUX)),
        read_aloud: Some(combo("Ctrl+Alt+S", LINUX)),
    }
}

fn app(outputs: Vec<Output>) -> Vec<AppEvent> {
    outputs
        .into_iter()
        .filter_map(|output| match output {
            Output::App(event) => Some(event),
            Output::Record(_) => None,
        })
        .collect()
}

fn recorded(outputs: Vec<Output>) -> Vec<Recording> {
    outputs
        .into_iter()
        .filter_map(|output| match output {
            Output::Record(recording) => Some(recording),
            Output::App(_) => None,
        })
        .collect()
}

fn chord(engine: &mut Engine, keys: &[Phys]) -> Vec<AppEvent> {
    let mut events = Vec::new();
    for key in keys {
        events.extend(app(engine.press(*key)));
    }
    for key in keys.iter().rev() {
        events.extend(app(engine.release(*key)));
    }
    events
}

const L_CMD: Phys = Phys::Modifier(Modifier::Cmd, Side::Left);
const TAB: Phys = Phys::Named(Named::Tab);

#[test]
fn a_named_key_chord_holds_and_its_key_stops_at_hushpen() {
    let mut engine = Engine::new(Bindings {
        hold: Some(combo("Cmd+Tab", MAC)),
        ..Bindings::default()
    });
    assert_eq!(app(engine.press(L_CMD)), []);
    assert!(!engine.swallows(L_CMD), "a modifier always reaches the app");
    assert_eq!(app(engine.press(TAB)), [AppEvent::HoldDown]);
    assert!(engine.swallows(TAB));
    // A repeat of the held key stops too.
    assert_eq!(app(engine.press(TAB)), []);
    assert!(engine.swallows(TAB));
    assert!(engine.swallows(TAB), "asked before the release");
    assert_eq!(app(engine.release(TAB)), [AppEvent::HoldUp]);
    assert!(!engine.swallows(TAB));
    engine.release(L_CMD);
}

#[test]
fn a_key_that_is_not_exactly_a_live_shortcut_reaches_the_app() {
    let mut engine = Engine::new(Bindings {
        paste_last: Some(combo("Cmd+Tab", MAC)),
        ..Bindings::default()
    });
    // Cmd+Shift+Tab is another shortcut.
    engine.press(L_CMD);
    engine.press(L_SHIFT);
    engine.press(TAB);
    assert!(!engine.swallows(TAB));
    engine.release(TAB);
    engine.release(L_SHIFT);
    // Tab alone, and a key no shortcut has.
    engine.release(L_CMD);
    engine.press(TAB);
    assert!(!engine.swallows(TAB));
    engine.release(TAB);
    engine.press(L_CMD);
    engine.press(Phys::Char('`'));
    assert!(!engine.swallows(Phys::Char('`')));
    engine.release(Phys::Char('`'));
    engine.press(TAB);
    assert!(engine.swallows(TAB));
    assert_eq!(app(engine.release(TAB)), []);
    assert_eq!(app(engine.release(L_CMD)), [AppEvent::PasteLast]);
}

#[test]
fn every_key_but_a_modifier_stops_at_hushpen_while_recording() {
    let mut engine = Engine::new(defaults());
    engine.start_recording();
    engine.press(L_CMD);
    engine.press(TAB);
    assert!(!engine.swallows(L_CMD));
    assert!(engine.swallows(TAB));
    engine.release(TAB);
    assert_eq!(
        recorded(engine.release(L_CMD)),
        [Recording::Captured(vec![
            Key::Any(Modifier::Cmd),
            Key::Named(Named::Tab)
        ])]
    );
    engine.start_recording();
    assert_eq!(recorded(engine.press(Phys::Esc)), [Recording::Cancelled]);
    assert!(
        engine.swallows(Phys::Esc),
        "the cancel does not reach the app"
    );
}

#[test]
fn space_for_the_built_in_hands_free_stops_at_hushpen() {
    let mut engine = Engine::new(Bindings {
        command: None,
        ..defaults()
    });
    engine.press(R_ALT);
    assert_eq!(
        app(engine.press(Phys::Char(' '))),
        [AppEvent::HandsFreeToggle]
    );
    assert!(engine.swallows(Phys::Char(' ')));
    engine.release(Phys::Char(' '));
    engine.release(R_ALT);
    engine.press(Phys::Char(' '));
    assert!(!engine.swallows(Phys::Char(' ')), "plain Space types");
}

#[test]
fn the_hold_key_sends_down_and_up_and_ignores_repeats() {
    let mut engine = Engine::new(Bindings {
        command: None,
        ..defaults()
    });
    assert_eq!(app(engine.press(R_ALT)), [AppEvent::HoldDown]);
    assert_eq!(app(engine.press(R_ALT)), []);
    assert_eq!(app(engine.press(Phys::Char('a'))), []);
    assert_eq!(app(engine.release(Phys::Char('a'))), []);
    assert_eq!(app(engine.release(R_ALT)), [AppEvent::HoldUp]);
    assert_eq!(app(engine.release(R_ALT)), []);
}

#[test]
fn only_the_configured_hold_key_counts() {
    let mut engine = Engine::new(defaults());
    assert_eq!(chord(&mut engine, &[L_ALT]), []);
    assert_eq!(chord(&mut engine, &[R_CTRL]), []);
    let mut engine = Engine::new(Bindings {
        hold: Some(combo("RightCtrl", LINUX)),
        ..defaults()
    });
    assert_eq!(chord(&mut engine, &[R_ALT]), []);
    assert_eq!(
        chord(&mut engine, &[R_CTRL]),
        [AppEvent::HoldDown, AppEvent::HoldUp]
    );
}

#[test]
fn a_hold_chord_is_on_while_every_key_is_down() {
    let mut engine = Engine::new(Bindings {
        hold: Some(combo("RightCtrl+RightShift", LINUX)),
        ..Bindings::default()
    });
    assert_eq!(app(engine.press(R_CTRL)), []);
    assert_eq!(app(engine.press(R_SHIFT)), [AppEvent::HoldDown]);
    assert_eq!(app(engine.release(R_CTRL)), [AppEvent::HoldUp]);
}

#[test]
fn a_chord_fires_once_when_the_last_key_is_let_go() {
    let mut engine = Engine::new(defaults());
    assert_eq!(app(engine.press(L_CTRL)), []);
    assert_eq!(app(engine.press(L_ALT)), []);
    assert_eq!(app(engine.press(Phys::Char('v'))), []);
    assert_eq!(app(engine.release(Phys::Char('v'))), []);
    assert_eq!(app(engine.release(L_CTRL)), []);
    assert_eq!(app(engine.release(L_ALT)), [AppEvent::PasteLast]);
}

#[test]
fn the_modifiers_may_come_up_before_the_letter() {
    let mut engine = Engine::new(defaults());
    for key in [L_CTRL, L_ALT, Phys::Char('v')] {
        engine.press(key);
    }
    assert_eq!(app(engine.release(L_CTRL)), []);
    assert_eq!(app(engine.release(L_ALT)), []);
    assert_eq!(app(engine.release(Phys::Char('v'))), [AppEvent::PasteLast]);
}

#[test]
fn a_chord_with_a_missing_or_extra_key_fires_nothing() {
    let mut engine = Engine::new(defaults());
    assert_eq!(chord(&mut engine, &[L_CTRL, Phys::Char('v')]), []);
    assert_eq!(chord(&mut engine, &[L_ALT, Phys::Char('v')]), []);
    assert_eq!(chord(&mut engine, &[Phys::Char('v')]), []);
    assert_eq!(
        chord(&mut engine, &[L_CTRL, L_ALT, L_SHIFT, Phys::Char('v')]),
        []
    );
    assert_eq!(chord(&mut engine, &[L_CTRL, L_ALT, Phys::Char('b')]), []);
}

#[test]
fn a_key_that_joins_after_the_chord_is_complete_cancels_it() {
    let mut engine = Engine::new(defaults());
    for key in [L_CTRL, L_ALT, Phys::Char('v'), Phys::Char('b')] {
        engine.press(key);
    }
    for key in [Phys::Char('b'), Phys::Char('v'), L_ALT, L_CTRL] {
        assert_eq!(app(engine.release(key)), []);
    }
}

#[test]
fn a_chord_can_start_again_after_it_fired() {
    let mut engine = Engine::new(defaults());
    assert_eq!(
        chord(&mut engine, &[L_CTRL, L_ALT, Phys::Char('v')]),
        [AppEvent::PasteLast]
    );
    assert_eq!(
        chord(&mut engine, &[L_CTRL, L_ALT, Phys::Char('v')]),
        [AppEvent::PasteLast]
    );
}

#[test]
fn the_hold_key_down_keeps_a_chord_from_firing_because_it_belongs_to_the_hold() {
    let mut engine = Engine::new(defaults());
    // Right Alt is the hold key, so it does not count as the Alt of Ctrl+Alt+V.
    assert_eq!(app(engine.press(R_ALT)), [AppEvent::HoldArm]);
    assert_eq!(app(engine.press(L_CTRL)), []);
    assert_eq!(app(engine.press(Phys::Char('v'))), []);
    assert_eq!(app(engine.release(Phys::Char('v'))), []);
    assert_eq!(app(engine.release(L_CTRL)), []);
    assert_eq!(app(engine.release(R_ALT)), [AppEvent::HoldUp]);
}

#[test]
fn a_recorded_hands_free_chord_toggles_on_release() {
    let mut engine = Engine::new(Bindings {
        hands_free: Some(HandsFree::Custom(ctrl_alt('h'))),
        ..defaults()
    });
    assert_eq!(
        chord(&mut engine, &[L_CTRL, L_ALT, Phys::Char('h')]),
        [AppEvent::HandsFreeToggle]
    );
    // The built-in Hold+Space is off once a shortcut is recorded.
    assert_eq!(app(engine.press(R_ALT)), [AppEvent::HoldArm]);
    assert_eq!(app(engine.press(Phys::Char(' '))), []);
}

#[test]
fn modifier_only_toggles_fire_when_the_keys_are_up_and_not_for_a_chord() {
    let mut engine = Engine::new(Bindings {
        hands_free: Some(HandsFree::Custom(combo("RightCtrl+RightShift", LINUX))),
        ..Bindings::default()
    });
    assert_eq!(
        chord(&mut engine, &[R_CTRL, R_SHIFT]),
        [AppEvent::HandsFreeToggle]
    );
    assert_eq!(chord(&mut engine, &[R_CTRL, Phys::Char('v')]), []);
    assert_eq!(chord(&mut engine, &[R_CTRL]), []);
}

#[test]
fn hold_with_space_toggles_hands_free_only_with_the_default() {
    let mut engine = Engine::new(defaults());
    assert_eq!(app(engine.press(Phys::Char(' '))), []);
    engine.release(Phys::Char(' '));
    assert_eq!(app(engine.press(R_ALT)), [AppEvent::HoldArm]);
    assert_eq!(
        app(engine.press(Phys::Char(' '))),
        [AppEvent::HandsFreeToggle]
    );
}

#[test]
fn esc_goes_to_the_pipeline_only_while_it_may() {
    let mut engine = Engine::new(defaults());
    assert_eq!(app(engine.press(Phys::Esc)), []);
    engine.release(Phys::Esc);
    engine.set_escape(true);
    assert_eq!(app(engine.press(Phys::Esc)), [AppEvent::Esc]);
}

#[test]
fn a_slot_with_no_shortcut_fires_nothing() {
    let mut engine = Engine::new(Bindings::default());
    assert_eq!(chord(&mut engine, &[R_ALT]), []);
    assert_eq!(chord(&mut engine, &[L_CTRL, L_ALT, Phys::Char('v')]), []);
}

#[test]
fn the_recorder_captures_the_most_keys_that_were_down_together() {
    let mut engine = Engine::new(defaults());
    engine.start_recording();
    assert!(engine.is_recording());
    assert_eq!(
        recorded(engine.press(L_CTRL)),
        [Recording::Progress(vec![Key::Any(Modifier::Ctrl)])]
    );
    engine.press(L_ALT);
    engine.press(Phys::Char('h'));
    assert_eq!(recorded(engine.release(Phys::Char('h'))), []);
    assert_eq!(recorded(engine.release(L_ALT)), []);
    assert_eq!(
        recorded(engine.release(L_CTRL)),
        [Recording::Captured(vec![
            Key::Any(Modifier::Ctrl),
            Key::Any(Modifier::Alt),
            Key::Char('h')
        ])]
    );
    assert!(!engine.is_recording());
}

#[test]
fn recording_a_modifier_only_key_gives_the_right_hand_key() {
    let mut engine = Engine::new(defaults());
    engine.start_recording();
    engine.press(R_CTRL);
    assert_eq!(
        recorded(engine.release(R_CTRL)),
        [Recording::Captured(vec![Key::Right(Modifier::Ctrl)])]
    );
}

#[test]
fn the_recorder_blocks_every_live_shortcut_and_esc_alone_closes_it() {
    let mut engine = Engine::new(defaults());
    engine.start_recording();
    assert_eq!(app(engine.press(R_ALT)), []);
    // With a key held, Esc is part of the shortcut.
    assert_eq!(
        recorded(engine.press(Phys::Esc)),
        [Recording::Progress(vec![
            Key::Right(Modifier::Alt),
            Key::Named(Named::Escape)
        ])]
    );
    assert!(engine.swallows(Phys::Esc));
    engine.release(Phys::Esc);
    engine.release(R_ALT);
    engine.start_recording();
    assert_eq!(recorded(engine.press(Phys::Esc)), [Recording::Cancelled]);
    assert!(!engine.is_recording());
    engine.release(Phys::Esc);
    // Live again.
    assert_eq!(
        chord(&mut engine, &[R_ALT]),
        [AppEvent::HoldArm, AppEvent::HoldUp]
    );
}

#[test]
fn a_bare_key_a_media_key_and_both_sides_of_ctrl_are_live_shortcuts() {
    let mac = |text: &str| Combo::parse(text, Platform::MacOs).unwrap();
    let mut engine = Engine::new(Bindings {
        hold: Some(mac("PlayPause")),
        hands_free: None,
        paste_last: Some(mac("Ctrl+RightCtrl")),
        command: Some(mac("Key179")),
        read_aloud: None,
    });
    let play = Phys::Named(Named::PlayPause);
    assert_eq!(app(engine.press(play)), [AppEvent::HoldDown]);
    assert!(engine.swallows(play));
    assert_eq!(app(engine.release(play)), [AppEvent::HoldUp]);
    // Right Ctrl alone is not Left Ctrl+Right Ctrl.
    assert_eq!(chord(&mut engine, &[R_CTRL]), []);
    assert_eq!(chord(&mut engine, &[L_CTRL, R_CTRL]), [AppEvent::PasteLast]);
    assert_eq!(
        chord(&mut engine, &[Phys::Code(179)]),
        [AppEvent::CommandDown, AppEvent::CommandUp]
    );
}

#[test]
fn keys_down_before_the_recorder_opened_are_ignored_until_they_come_up() {
    let mut engine = Engine::new(defaults());
    engine.press(R_CTRL);
    engine.start_recording();
    assert_eq!(recorded(engine.release(R_CTRL)), []);
    assert!(engine.is_recording());
}

#[test]
fn rebinding_lets_go_of_a_hold_and_ignores_a_key_already_down() {
    let mut engine = Engine::new(defaults());
    engine.press(R_ALT);
    let out = engine.set_bindings(Bindings {
        hold: Some(combo("RightCtrl", LINUX)),
        ..defaults()
    });
    assert_eq!(app(out), [AppEvent::HoldUp]);
    assert_eq!(app(engine.release(R_ALT)), []);
    assert_eq!(chord(&mut engine, &[R_ALT]), []);
    assert_eq!(
        chord(&mut engine, &[R_CTRL]),
        [AppEvent::HoldDown, AppEvent::HoldUp]
    );
}

#[test]
fn a_reset_lets_go_of_a_hold_that_was_on() {
    let mut engine = Engine::new(defaults());
    engine.press(R_ALT);
    assert_eq!(app(engine.reset()), [AppEvent::HoldUp]);
    assert_eq!(app(engine.reset()), []);
}

// Registration.

use std::collections::{BTreeMap, BTreeSet};

fn held(platform: Platform) -> BTreeMap<Slot, Setting> {
    Slot::ALL
        .into_iter()
        .map(|slot| (slot, Setting::default_for(slot, platform)))
        .collect()
}

#[test]
fn a_new_shortcut_that_is_free_passes() {
    let others = held(LINUX);
    assert_eq!(
        check(Slot::PasteLast, &ctrl_alt('p'), &others, &BTreeSet::new()),
        Ok(())
    );
    assert_eq!(
        check(
            Slot::Hold,
            &combo("RightCtrl", LINUX),
            &others,
            &BTreeSet::new()
        ),
        Ok(())
    );
}

#[test]
fn a_chord_every_app_uses_is_the_user_s_choice() {
    let others = held(LINUX);
    assert_eq!(
        check(
            Slot::HandsFree,
            &combo("Ctrl+Q", LINUX),
            &others,
            &BTreeSet::new()
        ),
        Ok(())
    );
}

#[test]
fn a_chord_another_slot_already_holds_is_refused_with_the_reserved_code() {
    let others = held(LINUX);
    let refusal = check(Slot::HandsFree, &ctrl_alt('v'), &others, &BTreeSet::new()).unwrap_err();
    assert_eq!(refusal.code, "SHORTCUT_RESERVED");
    assert!(
        refusal.message.contains("paste last"),
        "{}",
        refusal.message
    );
}

#[test]
fn a_slot_may_keep_its_own_shortcut_and_a_superset_of_another_is_fine() {
    let others = held(LINUX);
    assert_eq!(
        check(Slot::PasteLast, &ctrl_alt('v'), &others, &BTreeSet::new()),
        Ok(())
    );
    // The Command Mode default is the hold key plus Right Shift, which is not the same key set.
    assert_eq!(
        check(
            Slot::Command,
            &combo("RightAlt+RightShift", LINUX),
            &others,
            &BTreeSet::new()
        ),
        Ok(())
    );
}

#[test]
fn a_conflict_keeps_its_steps_and_an_invalid_refusal_says_why() {
    let refusal = Refusal::conflict(Conflict {
        message: "macOS uses this shortcut.".to_owned(),
        steps: "Turn it off.".to_owned(),
        open: Some(Opener::Url("x-apple.systempreferences:x".to_owned())),
    });
    assert_eq!(refusal.code, "SHORTCUT_IN_USE");
    assert_eq!(refusal.steps.as_deref(), Some("Turn it off."));
    assert!(refusal.open.is_some());
    let refusal = Refusal::invalid(Problem::BadKey, LINUX);
    assert_eq!(refusal.code, "SHORTCUT_INVALID");
    assert_eq!(refusal.message, Problem::BadKey.message(LINUX));
    assert_eq!(refusal.steps, None);
}

#[test]
fn settings_parse_per_slot_and_print_back() {
    for platform in [MAC, LINUX] {
        for slot in Slot::ALL {
            let setting = Setting::default_for(slot, platform);
            assert_eq!(setting.to_text(platform), slot.default_text(platform));
        }
    }
    assert_eq!(
        Setting::parse(Slot::HandsFree, "Ctrl+Alt+H", LINUX),
        Ok(Setting::HandsFree(HandsFree::Custom(combo(
            "Ctrl+Alt+H",
            LINUX
        ))))
    );
    assert!(Setting::parse(Slot::Hold, "Nope", LINUX).is_err());
}

#[test]
fn bindings_follow_the_slots_and_leave_out_the_ones_another_app_holds() {
    let values = held(LINUX);
    let live = bindings(&values, &BTreeSet::new(), &BTreeSet::new());
    assert_eq!(live, defaults());
    let without_paste = bindings(
        &values,
        &BTreeSet::from([Slot::PasteLast]),
        &BTreeSet::new(),
    );
    assert_eq!(without_paste.paste_last, None);
    assert_eq!(without_paste.hold, defaults().hold);
}

#[test]
fn a_recorded_hands_free_chord_becomes_a_custom_binding() {
    let mut values = held(LINUX);
    values.insert(
        Slot::HandsFree,
        Setting::parse(Slot::HandsFree, "Ctrl+Alt+H", LINUX).unwrap(),
    );
    assert_eq!(
        bindings(&values, &BTreeSet::new(), &BTreeSet::new()).hands_free,
        Some(HandsFree::Custom(ctrl_alt('h')))
    );
}

#[test]
fn an_off_shortcut_does_not_run_and_keeps_its_keys() {
    let values = held(LINUX);
    let off = BTreeSet::from([Slot::PasteLast, Slot::Command]);
    let live = bindings(&values, &BTreeSet::new(), &off);
    assert_eq!(live.paste_last, None);
    assert_eq!(live.command, None);
    assert_eq!(live.hold, defaults().hold);
    assert_eq!(
        values[&Slot::PasteLast],
        Setting::default_for(Slot::PasteLast, LINUX)
    );
}

#[test]
fn with_the_hold_key_off_it_stays_live_only_for_the_built_in_hands_free() {
    let mut values = held(LINUX);
    let off = BTreeSet::from([Slot::Hold]);
    let live = bindings(&values, &BTreeSet::new(), &off);
    assert_eq!(live.hold, defaults().hold);
    assert_eq!(live.hands_free, Some(HandsFree::Default));

    let both = BTreeSet::from([Slot::Hold, Slot::HandsFree]);
    assert_eq!(bindings(&values, &BTreeSet::new(), &both).hold, None);

    values.insert(
        Slot::HandsFree,
        Setting::parse(Slot::HandsFree, "Ctrl+Alt+H", LINUX).unwrap(),
    );
    let custom = bindings(&values, &BTreeSet::new(), &off);
    assert_eq!(custom.hold, None);
    assert_eq!(custom.hands_free, Some(HandsFree::Custom(ctrl_alt('h'))));
}

#[test]
fn the_keys_of_an_off_shortcut_are_free_for_another_one() {
    let others = held(LINUX);
    let off = BTreeSet::from([Slot::PasteLast]);
    assert_eq!(
        check(Slot::HandsFree, &ctrl_alt('v'), &others, &off),
        Ok(())
    );
}

#[test]
fn a_slot_is_on_unless_the_setting_turns_it_off() {
    let enabled = serde_json::json!({"pasteLast": false, "command": true});
    assert!(Slot::Hold.is_on(Some(&enabled)));
    assert!(Slot::Command.is_on(Some(&enabled)));
    assert!(!Slot::PasteLast.is_on(Some(&enabled)));
    assert!(Slot::PasteLast.is_on(None));
}

// Command Mode events.

#[test]
fn the_command_keys_go_down_after_the_hold_and_come_up_before_it() {
    let mut engine = Engine::new(defaults());
    assert_eq!(app(engine.press(R_ALT)), [AppEvent::HoldArm]);
    assert_eq!(app(engine.press(R_SHIFT)), [AppEvent::CommandDown]);
    assert_eq!(app(engine.release(R_SHIFT)), [AppEvent::CommandUp]);
    assert_eq!(app(engine.release(R_ALT)), [AppEvent::HoldUp]);
}

#[test]
fn letting_go_of_the_hold_key_first_ends_the_command_and_the_hold_together() {
    let mut engine = Engine::new(defaults());
    engine.press(R_ALT);
    engine.press(R_SHIFT);
    assert_eq!(
        app(engine.release(R_ALT)),
        [AppEvent::CommandUp, AppEvent::HoldUp]
    );
    assert_eq!(app(engine.release(R_SHIFT)), []);
}

#[test]
fn a_command_chord_pressed_in_the_other_order_starts_no_hold() {
    let mut engine = Engine::new(defaults());
    assert_eq!(app(engine.press(R_SHIFT)), []);
    assert_eq!(app(engine.press(R_ALT)), [AppEvent::CommandDown]);
    assert_eq!(
        app(engine.release(R_ALT)),
        [AppEvent::CommandUp, AppEvent::HoldUp]
    );
}

#[test]
fn a_hold_key_that_the_command_chord_may_join_is_only_armed() {
    let mut engine = Engine::new(defaults());
    assert_eq!(app(engine.press(R_ALT)), [AppEvent::HoldArm]);
    assert_eq!(app(engine.release(R_ALT)), [AppEvent::HoldUp]);
}

#[test]
fn a_hold_key_pressed_with_a_key_that_cannot_make_the_chord_starts_at_once() {
    let mut engine = Engine::new(defaults());
    engine.press(Phys::Char('a'));
    assert_eq!(app(engine.press(R_ALT)), [AppEvent::HoldDown]);
    engine.release(R_ALT);
    engine.release(Phys::Char('a'));
    let mut engine = Engine::new(Bindings {
        command: None,
        ..defaults()
    });
    assert_eq!(app(engine.press(R_ALT)), [AppEvent::HoldDown]);
}

#[test]
fn right_shift_alone_or_left_shift_with_the_hold_key_is_no_command() {
    let mut engine = Engine::new(defaults());
    assert_eq!(chord(&mut engine, &[R_SHIFT]), []);
    assert_eq!(
        chord(&mut engine, &[R_ALT, L_SHIFT]),
        [AppEvent::HoldArm, AppEvent::HoldUp]
    );
}

#[test]
fn a_command_shortcut_that_is_not_the_hold_key_plus_more_runs_on_its_own() {
    let mut engine = Engine::new(Bindings {
        hold: Some(combo("RightCtrl", LINUX)),
        ..defaults()
    });
    assert_eq!(
        chord(&mut engine, &[R_ALT, R_SHIFT]),
        [AppEvent::CommandDown, AppEvent::CommandUp]
    );
}

#[test]
fn no_command_binding_means_no_command_event() {
    let mut engine = Engine::new(Bindings {
        command: None,
        ..defaults()
    });
    assert_eq!(
        chord(&mut engine, &[R_ALT, R_SHIFT]),
        [AppEvent::HoldDown, AppEvent::HoldUp]
    );
}

#[test]
fn a_command_that_is_on_is_let_go_when_the_bindings_change_or_the_stream_resets() {
    let mut engine = Engine::new(defaults());
    engine.press(R_ALT);
    engine.press(R_SHIFT);
    assert_eq!(
        app(engine.set_bindings(Bindings {
            command: None,
            ..defaults()
        })),
        [AppEvent::CommandUp, AppEvent::HoldUp]
    );
    let mut engine = Engine::new(defaults());
    engine.press(R_ALT);
    engine.press(R_SHIFT);
    assert_eq!(app(engine.reset()), [AppEvent::CommandUp, AppEvent::HoldUp]);
}

#[test]
fn no_command_event_fires_while_a_shortcut_is_being_recorded() {
    let mut engine = Engine::new(defaults());
    engine.start_recording();
    assert_eq!(app(engine.press(R_ALT)), []);
    assert_eq!(app(engine.press(R_SHIFT)), []);
    assert_eq!(app(engine.release(R_SHIFT)), []);
    assert_eq!(app(engine.release(R_ALT)), []);
}

#[test]
fn space_during_a_command_is_not_the_hands_free_toggle() {
    let mut engine = Engine::new(defaults());
    engine.press(R_ALT);
    engine.press(R_SHIFT);
    assert_eq!(app(engine.press(Phys::Char(' '))), []);
}
