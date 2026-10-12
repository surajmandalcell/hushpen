use super::endpoint::EndpointForm;
use super::{Parts, Section, Settings};
use crate::net::endpoint::{Config, Failure, Kind, Transport, UreqTransport};
use crate::shell::Shell;
use crate::storage;
use crate::theme::space;
use crate::views::View;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Bounds, Entity, Point, ScrollDelta, TestAppContext,
    WindowBounds, WindowOptions, point, px, size,
};
use hushpen_store::data_dir::DataDir;
use serde_json::{Value, json};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const KEY: &str = "sk-test-123";

#[derive(Default)]
struct Fake {
    calls: Mutex<Vec<Config>>,
    answer: Mutex<Option<Failure>>,
}

impl Transport for Fake {
    fn test_connection(&self, config: &Config) -> Result<(), Failure> {
        self.calls.lock().unwrap().push(config.clone());
        match self.answer.lock().unwrap().clone() {
            Some(failure) => Err(failure),
            None => Ok(()),
        }
    }

    fn complete(&self, _: &Config, _: &str, _: &str, _: u32) -> Result<String, Failure> {
        unreachable!("Settings never asks for a completion")
    }
}

struct Rig {
    handle: AnyWindowHandle,
    settings: Entity<Settings>,
    form: Entity<EndpointForm>,
    storage: Rc<storage::Storage>,
    fake: Arc<Fake>,
    _tmp: tempfile::TempDir,
}

fn open(cx: &mut TestAppContext, transport: Option<Arc<dyn Transport>>) -> Rig {
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
    let fake = Arc::new(Fake::default());
    let transport = transport.unwrap_or_else(|| fake.clone());
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
    let form = {
        let storage = Rc::clone(&storage);
        cx.update_window(handle, move |_, window, cx| {
            cx.new(|cx| EndpointForm::new(storage, transport, window, cx))
        })
        .unwrap()
    };
    settings.update(cx, |settings, cx| {
        settings.attach_endpoint(form.clone(), cx);
        settings.select_section(Section::Cleanup, cx);
    });
    shell.update(cx, |shell, cx| {
        shell.attach_settings(settings.clone(), cx);
        shell.select(View::Settings, cx);
    });
    let rig = Rig {
        handle,
        settings,
        form,
        storage,
        fake,
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

fn visible(cx: &mut TestAppContext, rig: &Rig, id: &'static str) -> bool {
    cx.update_window(rig.handle, |_, window, _| {
        window.try_find(id).is_some_and(|found| found.visible())
    })
    .unwrap()
}

fn scroll_by(cx: &mut TestAppContext, rig: &Rig, pixels: f32) {
    cx.update_window(rig.handle, |_, window, cx| {
        window.scroll(
            "content.scroll",
            ScrollDelta::Pixels(point(px(0.0), px(pixels))),
            cx,
        )
    })
    .unwrap();
    frame(cx, rig);
}

/// The page is taller than the window, so a click first scrolls the row into view, like the
/// mouse wheel does.
fn click(cx: &mut TestAppContext, rig: &Rig, id: &'static str) {
    scroll_by(cx, rig, 5_000.0);
    for _ in 0..40 {
        if visible(cx, rig, id) {
            break;
        }
        scroll_by(cx, rig, -120.0);
    }
    cx.update_window(rig.handle, |_, window, cx| window.click(id, cx))
        .unwrap();
    frame(cx, rig);
}

fn type_text(cx: &mut TestAppContext, rig: &Rig, text: &str) {
    cx.update_window(rig.handle, |_, window, cx| window.input(text, cx))
        .unwrap();
    frame(cx, rig);
}

fn press(cx: &mut TestAppContext, rig: &Rig, key: &'static str) {
    cx.update_window(rig.handle, |_, window, cx| window.press(key, cx))
        .unwrap();
    frame(cx, rig);
}

fn present(cx: &mut TestAppContext, rig: &Rig, id: &'static str) -> bool {
    cx.update_window(rig.handle, |_, window, _| window.try_find(id).is_some())
        .unwrap()
}

fn label(cx: &mut TestAppContext, rig: &Rig, id: &'static str) -> Option<String> {
    cx.update_window(rig.handle, |_, window, _| {
        window
            .try_find(id)
            .and_then(|element| element.label().map(str::to_owned))
    })
    .unwrap()
}

fn setting(rig: &Rig, key: &str) -> Value {
    rig.storage.settings.get(key).unwrap_or(Value::Null)
}

fn state(cx: &mut TestAppContext, rig: &Rig) -> Value {
    cx.update(|cx| rig.settings.read(cx).hook_state_json(cx))
}

fn use_the_endpoint(cx: &mut TestAppContext, rig: &Rig) {
    click(cx, rig, "settings.cleanup.provider");
    click(cx, rig, "settings.cleanup.provider.option.endpoint");
}

fn pick_preset(cx: &mut TestAppContext, rig: &Rig, key: &'static str) {
    click(cx, rig, "settings.cleanup.endpointPreset");
    click(cx, rig, key);
}

#[gpui_kit::test]
fn the_endpoint_is_off_and_empty_on_a_fresh_data_folder(cx: &mut TestAppContext) {
    let rig = open(cx, None);

    assert_eq!(setting(&rig, "cleanup.llm.provider"), json!("local"));
    assert_eq!(setting(&rig, "cleanup.llm.endpointUrl"), json!(""));
    assert_eq!(setting(&rig, "cleanup.llm.endpointModel"), json!(""));
    assert!(
        !present(cx, &rig, "settings.cleanup.endpointUrl"),
        "the endpoint rows stay hidden while the provider is local"
    );
    let state = state(cx, &rig);
    assert_eq!(state["ai"]["provider"], "local");
    assert_eq!(state["ai"]["endpoint"]["api_key_set"], false);
}

#[gpui_kit::test]
fn choosing_the_endpoint_shows_its_rows_and_choosing_this_computer_hides_them(
    cx: &mut TestAppContext,
) {
    let rig = open(cx, None);

    use_the_endpoint(cx, &rig);

    assert_eq!(setting(&rig, "cleanup.llm.provider"), json!("endpoint"));
    for id in [
        "settings.cleanup.endpointPreset",
        "settings.cleanup.endpointUrl",
        "settings.cleanup.endpointModel",
        "settings.cleanup.endpointKey",
        "settings.cleanup.endpointTest",
        "settings.cleanup.endpointStatus",
    ] {
        assert!(present(cx, &rig, id), "{id}");
    }
    assert!(!present(cx, &rig, "settings.cleanup.aiModel"));

    click(cx, &rig, "settings.cleanup.provider");
    click(cx, &rig, "settings.cleanup.provider.option.local");
    assert_eq!(setting(&rig, "cleanup.llm.provider"), json!("local"));
    assert!(!present(cx, &rig, "settings.cleanup.endpointUrl"));
}

#[gpui_kit::test]
fn the_presets_fill_the_address(cx: &mut TestAppContext) {
    let rig = open(cx, None);
    use_the_endpoint(cx, &rig);

    pick_preset(cx, &rig, "settings.cleanup.endpointPreset.option.ollama");
    assert_eq!(
        setting(&rig, "cleanup.llm.endpointUrl"),
        json!("http://localhost:11434/v1")
    );
    assert_eq!(setting(&rig, "cleanup.llm.endpointPreset"), json!("ollama"));
    assert_eq!(
        state(cx, &rig)["ai"]["endpoint"]["url"],
        "http://localhost:11434/v1",
        "the field shows it too"
    );

    pick_preset(cx, &rig, "settings.cleanup.endpointPreset.option.lmstudio");
    assert_eq!(
        setting(&rig, "cleanup.llm.endpointUrl"),
        json!("http://localhost:1234/v1")
    );
    assert_eq!(
        setting(&rig, "cleanup.llm.endpointPreset"),
        json!("lmstudio")
    );
}

#[gpui_kit::test]
fn typing_an_address_makes_the_preset_custom_and_saves_it(cx: &mut TestAppContext) {
    let rig = open(cx, None);
    use_the_endpoint(cx, &rig);
    pick_preset(cx, &rig, "settings.cleanup.endpointPreset.option.ollama");

    click(cx, &rig, "settings.cleanup.endpointUrl");
    press(cx, &rig, "end");
    type_text(cx, &rig, "x");

    assert_eq!(setting(&rig, "cleanup.llm.endpointPreset"), json!("custom"));
    assert_eq!(
        setting(&rig, "cleanup.llm.endpointUrl"),
        json!("http://localhost:11434/v1x")
    );

    click(cx, &rig, "settings.cleanup.endpointModel");
    type_text(cx, &rig, "stub-model");
    assert_eq!(
        setting(&rig, "cleanup.llm.endpointModel"),
        json!("stub-model")
    );
}

#[gpui_kit::test]
fn the_key_goes_to_the_secrets_file_and_nowhere_else(cx: &mut TestAppContext) {
    let rig = open(cx, None);
    use_the_endpoint(cx, &rig);

    click(cx, &rig, "settings.cleanup.endpointKey");
    type_text(cx, &rig, KEY);
    press(cx, &rig, "enter");

    assert_eq!(rig.storage.secrets.endpoint_api_key().as_deref(), Some(KEY));
    let settings_file =
        std::fs::read_to_string(rig.storage.data.config_dir().join("settings.json"))
            .unwrap_or_default();
    assert!(!settings_file.contains(KEY));
    assert!(!state(cx, &rig).to_string().contains(KEY));
    assert_eq!(state(cx, &rig)["ai"]["endpoint"]["api_key_set"], true);
    let shown = rig.form.read_with(cx, |form, cx| form.key.read(cx).value());
    assert_eq!(shown.as_ref(), "", "the field is empty again");
    assert!(present(cx, &rig, "settings.cleanup.endpointKeyClear"));

    click(cx, &rig, "settings.cleanup.endpointKeyClear");
    assert_eq!(rig.storage.secrets.endpoint_api_key(), None);
    assert!(!present(cx, &rig, "settings.cleanup.endpointKeyClear"));
}

#[gpui_kit::test]
fn a_key_with_a_space_is_refused_without_naming_it(cx: &mut TestAppContext) {
    let rig = open(cx, None);
    use_the_endpoint(cx, &rig);

    click(cx, &rig, "settings.cleanup.endpointKey");
    type_text(cx, &rig, "sk bad");
    press(cx, &rig, "enter");

    assert_eq!(rig.storage.secrets.endpoint_api_key(), None);
    let notice = rig.settings.read_with(cx, |me, _| me.notice.clone());
    assert!(
        rig.form
            .read_with(cx, |form, _| form.notice.clone())
            .is_some_and(|text| !text.contains("sk")),
        "{notice:?}"
    );
}

#[gpui_kit::test]
fn test_connection_reports_success_and_failure_with_the_code(cx: &mut TestAppContext) {
    let rig = open(cx, None);
    use_the_endpoint(cx, &rig);
    pick_preset(cx, &rig, "settings.cleanup.endpointPreset.option.ollama");
    click(cx, &rig, "settings.cleanup.endpointKey");
    type_text(cx, &rig, KEY);

    click(cx, &rig, "settings.cleanup.endpointTest");

    assert_eq!(
        label(cx, &rig, "settings.cleanup.endpointStatus").as_deref(),
        Some("Connected")
    );
    {
        let calls = rig.fake.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].url, "http://localhost:11434/v1");
        assert_eq!(
            calls[0].key.as_deref(),
            Some(KEY),
            "a key typed but not yet saved is saved before the test"
        );
        assert_eq!(calls[0].timeout, std::time::Duration::from_secs(6));
    }

    *rig.fake.answer.lock().unwrap() = Some(Failure {
        kind: Kind::Down,
        host: "localhost".into(),
    });
    click(cx, &rig, "settings.cleanup.endpointTest");

    let text = label(cx, &rig, "settings.cleanup.endpointStatus").unwrap();
    assert!(text.starts_with("ENDPOINT_FAILED: "), "{text}");
    let test = state(cx, &rig)["ai"]["endpoint"]["test"].clone();
    assert_eq!(test["state"], "failed");
    assert_eq!(test["code"], "ENDPOINT_FAILED");
    assert!(!state(cx, &rig).to_string().contains(KEY));
}

#[gpui_kit::test]
fn an_http_address_to_another_computer_is_refused_before_any_request(cx: &mut TestAppContext) {
    let rig = open(cx, Some(Arc::new(UreqTransport)));
    use_the_endpoint(cx, &rig);
    click(cx, &rig, "settings.cleanup.endpointUrl");
    let other_computer = format!("http://{}:11434/v1", "10.0.0.2");
    type_text(cx, &rig, &other_computer);

    click(cx, &rig, "settings.cleanup.endpointTest");

    let text = label(cx, &rig, "settings.cleanup.endpointStatus").unwrap();
    assert!(text.starts_with("ENDPOINT_FAILED: "), "{text}");
    assert!(text.contains("Use https"), "{text}");
    assert!(
        !text.contains("10.0.0.2"),
        "the reason never repeats the address"
    );
}
