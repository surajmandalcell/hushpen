//! The Profile and Prompt rows of Cleanup and the prompt sheet in a window: the sheet shows what
//! a cleanup sends for the model in use, and its changes go into the chosen profile.

use super::{AiParts, Parts, Settings};
use crate::ai_cleanup::{self, PROFILE_EDITS_SETTING, PROFILE_SETTING};
use crate::models::Models;
use crate::shell::Shell;
use crate::storage;
use crate::theme::space;
use crate::views::View;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Bounds, Entity, Point, TestAppContext, WindowBounds,
    WindowOptions, px, size,
};
use hushpen_core::llm_prompt::{
    Profile, PromptFamily, S1_MINI_SYSTEM, Styling, TRANSCRIPT_TAGS, Target, Tuned, cleanup_prompt,
};
use hushpen_store::data_dir::DataDir;
use serde_json::{Value, json};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

struct Rig {
    handle: AnyWindowHandle,
    settings: Entity<Settings>,
    storage: Rc<storage::Storage>,
    _tmp: tempfile::TempDir,
}

fn open(cx: &mut TestAppContext) -> Rig {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Rc::new(storage::open(DataDir::open(tmp.path().join("data")).unwrap()).unwrap());
    cx.update(gpui_kit::init);
    let (handle, shell) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(space::WINDOW_WIDTH), px(space::WINDOW_HEIGHT)),
                })),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Shell::new(window, cx)),
        )
        .expect("open test window")
    });
    let settings = {
        let storage = Rc::clone(&storage);
        cx.new(|cx| {
            Settings::new(
                Parts {
                    storage: Rc::clone(&storage),
                    mic: cx.new(|cx| {
                        crate::mic::Mic::new(
                            storage,
                            Arc::new(crate::controller::testkit::FakeMic {
                                start_error: Mutex::new(None),
                                starts: std::sync::atomic::AtomicUsize::new(0),
                                sink: Mutex::new(None),
                            }),
                            cx,
                        )
                    }),
                    login: None,
                    default_data_dir: None,
                    restart: None,
                },
                cx,
            )
        })
    };
    let spawn: crate::models::Spawner = Rc::new(|_| Ok(()));
    let transfer: Arc<crate::models::Transfer> = Arc::new(
        |_: &crate::net::download::Spec,
         _: &std::sync::atomic::AtomicBool,
         _: &mut dyn FnMut(crate::net::download::Progress)| {
            unreachable!("the prompt sheet downloads nothing")
        },
    );
    let models = cx.new(|cx| {
        Models::new(
            Rc::clone(&storage),
            hushpen_core::catalog::embedded(),
            crate::models::Machine::test(),
            transfer,
            spawn,
            None,
            cx,
        )
    });
    settings.update(cx, |me, cx| {
        me.attach_ai(
            AiParts {
                models,
                catalog: hushpen_core::catalog::embedded(),
                loading: Rc::new(|| false),
            },
            cx,
        );
        // The window shows the top of the list; the search brings the two rows there.
        me.set_query("profile".into(), cx);
    });
    shell.update(cx, |shell, cx| {
        shell.attach_settings(settings.clone(), cx);
        shell.select(View::Settings, cx);
    });
    let rig = Rig {
        handle,
        settings,
        storage,
        _tmp: tmp,
    };
    frame(cx, &rig);
    rig
}

fn frame(cx: &mut TestAppContext, rig: &Rig) {
    cx.run_until_parked();
    cx.update_window(rig.handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn click(cx: &mut TestAppContext, rig: &Rig, id: &'static str) {
    cx.update_window(rig.handle, |_, window, cx| window.click(id, cx))
        .unwrap();
    frame(cx, rig);
}

fn press(cx: &mut TestAppContext, rig: &Rig, key: &'static str) {
    cx.update_window(rig.handle, |_, window, cx| window.press(key, cx))
        .unwrap();
    frame(cx, rig);
}

fn label(cx: &mut TestAppContext, rig: &Rig, id: &'static str) -> Option<String> {
    cx.update_window(rig.handle, |_, window, _| {
        window
            .try_find(id)
            .and_then(|element| element.label().map(str::to_owned))
    })
    .unwrap()
}

fn present(cx: &mut TestAppContext, rig: &Rig, id: &'static str) -> bool {
    cx.update_window(rig.handle, |_, window, _| window.try_find(id).is_some())
        .unwrap()
}

fn profile_state(cx: &mut TestAppContext, rig: &Rig) -> Value {
    cx.update(|cx| rig.settings.read(cx).hook_state_json(cx))["profile"].clone()
}

fn set(rig: &Rig, key: &str, value: Value) {
    rig.storage.settings.set(key, value).unwrap();
}

fn type_prompt(cx: &mut TestAppContext, rig: &Rig, text: &str) {
    cx.update_window(rig.handle, |_, window, cx| {
        rig.settings
            .update(cx, |me, cx| me.type_prompt(text, window, cx))
    })
    .unwrap();
    frame(cx, rig);
}

#[gpui_kit::test]
fn the_profile_starts_on_general_and_a_pick_is_saved(cx: &mut TestAppContext) {
    let rig = open(cx);
    assert_eq!(
        label(cx, &rig, "settings.cleanup.profile").as_deref(),
        Some("Profile: General")
    );
    assert_eq!(profile_state(cx, &rig)["edited"], false);

    click(cx, &rig, "settings.cleanup.profile");
    click(cx, &rig, "settings.cleanup.profile.option.email");

    assert_eq!(
        rig.storage.settings.get(PROFILE_SETTING),
        Some(json!("email"))
    );
    assert_eq!(
        label(cx, &rig, "settings.cleanup.profile").as_deref(),
        Some("Profile: Email")
    );
    assert_eq!(
        ai_cleanup::profile(&rig.storage),
        Tuned::built_in(Profile::Email)
    );
}

#[gpui_kit::test]
fn the_qwen_sheet_edits_the_prompt_that_is_sent_and_resets_it(cx: &mut TestAppContext) {
    let rig = open(cx);
    set(&rig, "cleanup.llm.modelId", json!("qwen3.5-2b"));
    set(&rig, PROFILE_SETTING, json!("notes"));
    frame(cx, &rig);
    assert_eq!(
        label(cx, &rig, "settings.cleanup.prompt").as_deref(),
        Some("Edit\u{2026}")
    );

    click(cx, &rig, "settings.cleanup.prompt");
    let sheet = profile_state(cx, &rig)["sheet"].clone();
    assert_eq!(sheet["target"], "qwen35");
    assert_eq!(sheet["editable"], true);
    assert_eq!(sheet["text"], Profile::Notes.prompt());
    assert_eq!(
        label(cx, &rig, "settings.cleanup.prompt.sheet.title").as_deref(),
        Some("Notes prompt")
    );
    assert_eq!(
        label(cx, &rig, "settings.cleanup.prompt.examples").as_deref(),
        Some("4 cleanups, sent as earlier turns")
    );

    type_prompt(cx, &rig, "List what I say, one item a line.");
    let tuned = ai_cleanup::profile(&rig.storage);
    assert_eq!(tuned.prompt, "List what I say, one item a line.");
    assert!(tuned.edited());
    let keep = ["Zorvath".to_owned()];
    assert!(
        cleanup_prompt(Target::Local(PromptFamily::Qwen35), &tuned, &keep)
            .system()
            .starts_with("List what I say, one item a line. These words")
    );

    type_prompt(cx, &rig, "   ");
    assert!(present(cx, &rig, "settings.cleanup.prompt.notice"));
    assert_eq!(
        ai_cleanup::profile(&rig.storage).prompt,
        Profile::Notes.prompt()
    );
    type_prompt(cx, &rig, "Mine.");

    click(cx, &rig, "settings.cleanup.prompt.reset");
    assert_eq!(
        ai_cleanup::profile(&rig.storage),
        Tuned::built_in(Profile::Notes)
    );
    assert_eq!(
        rig.storage.settings.get(PROFILE_EDITS_SETTING),
        Some(json!({}))
    );
    assert_eq!(
        profile_state(cx, &rig)["sheet"]["text"],
        Profile::Notes.prompt()
    );

    press(cx, &rig, "escape");
    assert_eq!(profile_state(cx, &rig)["sheet"], Value::Null);
}

#[gpui_kit::test]
fn the_s1_mini_sheet_shows_its_fixed_prompt_and_saves_the_trained_values(cx: &mut TestAppContext) {
    let rig = open(cx);
    set(&rig, "cleanup.llm.modelId", json!("s1-mini"));
    frame(cx, &rig);
    assert_eq!(
        label(cx, &rig, "settings.cleanup.prompt").as_deref(),
        Some("View\u{2026}")
    );

    click(cx, &rig, "settings.cleanup.prompt");
    let sheet = profile_state(cx, &rig)["sheet"].clone();
    assert_eq!(sheet["target"], "s1mini");
    assert_eq!(sheet["editable"], false);
    assert_eq!(sheet["text"], S1_MINI_SYSTEM);
    assert_eq!(
        sheet["control"],
        "[Styling: semi-formal] [Structure: prose] [Context: general]"
    );
    // Typing into the fixed prompt changes nothing that is sent.
    type_prompt(cx, &rig, "Write a poem.");
    assert!(!ai_cleanup::profile(&rig.storage).edited());

    click(cx, &rig, "settings.cleanup.prompt.styling");
    click(cx, &rig, "settings.cleanup.prompt.styling.option.casual");
    let tuned = ai_cleanup::profile(&rig.storage);
    assert_eq!(tuned.controls.styling, Styling::Casual);
    assert_eq!(
        label(cx, &rig, "settings.cleanup.prompt.controlLine").as_deref(),
        Some("[Styling: casual] [Structure: prose] [Context: general]")
    );
    assert_eq!(
        cleanup_prompt(Target::Local(PromptFamily::S1Mini), &tuned, &[]).job_system(),
        "[Styling: casual] [Structure: prose] [Context: general]"
    );
    assert_eq!(profile_state(cx, &rig)["edited"], true);

    click(cx, &rig, "settings.cleanup.prompt.done");
    assert_eq!(profile_state(cx, &rig)["sheet"], Value::Null);
    assert_eq!(
        label(cx, &rig, "settings.cleanup.profile.row.detail").as_deref(),
        Some("How the cleaned text reads. Edited.")
    );
}

#[gpui_kit::test]
fn the_endpoint_sheet_names_the_tag_rule_it_sends(cx: &mut TestAppContext) {
    let rig = open(cx);
    set(&rig, "cleanup.llm.provider", json!("endpoint"));
    frame(cx, &rig);

    click(cx, &rig, "settings.cleanup.prompt");
    assert_eq!(profile_state(cx, &rig)["sheet"]["target"], "endpoint");
    assert_eq!(
        label(cx, &rig, "settings.cleanup.prompt.format").as_deref(),
        Some(TRANSCRIPT_TAGS)
    );
    assert!(!present(cx, &rig, "settings.cleanup.prompt.examples"));
}
