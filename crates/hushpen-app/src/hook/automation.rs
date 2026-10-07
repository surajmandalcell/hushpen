//! The app side of the test hook: reads the element tree and the app state,
//! clicks and runs actions through the normal handlers. Compiled only with
//! the `test-automation` feature.
//!
//! Hook clients run on their own threads. A request becomes a [`Job`] that the
//! GPUI main thread answers, so every reading and every click happens where
//! the UI lives.

use crate::shell::Shell;
use crate::views::View;
use futures::StreamExt;
use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use gpui_kit::base::test_support::{ElementSnapshot, snapshots};
use gpui_kit::{
    AnyWindowHandle, App, BorrowAppContext as _, ElementId, Entity, Global, InputEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Window,
};
use hushpen_store::data_dir::DataDir;
use hushpen_testhook::{
    ActionInfo, ActionRegistry, Backend, Bounds, ElementInfo, HookServer, Router, StateRegistry,
    endpoint,
};
use serde_json::{Map, Value, json};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Mutex;
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

/// How long a hook client waits for the UI thread.
const UI_ANSWER_TIMEOUT: Duration = Duration::from_secs(10);

type FeedWav = Box<dyn Fn(&mut App, &Path) -> Result<Value, String>>;

thread_local! {
    /// Ids of elements the last frame drew as disabled. GPUI divs have no disabled flag in
    /// the accessibility tree, so the views report it here and the tree and `click` honor it.
    static DISABLED: std::cell::RefCell<std::collections::HashSet<String>> =
        std::cell::RefCell::default();
}

/// Records whether the element `id` is drawn disabled. Call it while rendering the element.
pub fn mark_disabled(id: &str, disabled: bool) {
    DISABLED.with(|set| {
        let mut set = set.borrow_mut();
        if disabled {
            set.insert(id.to_owned());
        } else {
            set.remove(id);
        }
    });
}

fn is_disabled(id: &str) -> bool {
    DISABLED.with(|set| set.borrow().contains(id))
}

/// What features register. It lives in a GPUI global so any handler can add
/// an action or replace a state section.
pub struct Hooks {
    actions: ActionRegistry<App>,
    state: StateRegistry<App>,
    feed_wav: Option<FeedWav>,
}

impl Global for Hooks {}

/// Adds a named action that `hookctl action <name>` runs. The handler should
/// call the same function the UI calls. Also list it in
/// `library/user-testing.md`.
pub fn register_action(
    cx: &mut App,
    name: &str,
    description: &str,
    run: impl Fn(&mut App, Value) -> Result<Value, String> + 'static,
) -> Result<(), String> {
    cx.update_global::<Hooks, _>(|hooks, _| hooks.actions.register(name, description, run))
}

/// Sets (or replaces) a top-level section of `hookctl state`.
pub fn set_state_section(cx: &mut App, name: &str, read: impl Fn(&mut App) -> Value + 'static) {
    cx.update_global::<Hooks, _>(|hooks, _| hooks.state.set_section(name, read));
}

/// Connects `hookctl feed-wav` to the capture path.
pub fn set_wav_feeder(
    cx: &mut App,
    feed: impl Fn(&mut App, &Path) -> Result<Value, String> + 'static,
) {
    cx.update_global::<Hooks, _>(|hooks, _| hooks.feed_wav = Some(Box::new(feed)));
}

enum Call {
    Tree,
    State,
    Click(String),
    Actions,
    RunAction { name: String, args: Value },
    FeedWav(PathBuf),
}

enum Answer {
    Tree(Vec<ElementInfo>),
    Actions(Vec<ActionInfo>),
    Value(Value),
}

pub struct Job {
    call: Call,
    reply: std_mpsc::Sender<Result<Answer, String>>,
}

/// The [`Backend`] hook clients talk to; it queues jobs for the main thread.
struct Bridge {
    jobs: Mutex<UnboundedSender<Job>>,
}

impl Bridge {
    fn call(&self, call: Call) -> Result<Answer, String> {
        let (reply, answer) = std_mpsc::channel();
        self.jobs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .unbounded_send(Job { call, reply })
            .map_err(|_| "the app is shutting down".to_string())?;
        answer
            .recv_timeout(UI_ANSWER_TIMEOUT)
            .map_err(|_| "the UI thread did not answer in time".to_string())?
    }

    fn value(&self, call: Call) -> Result<Value, String> {
        match self.call(call)? {
            Answer::Value(value) => Ok(value),
            _ => Err("internal: unexpected answer".to_string()),
        }
    }
}

impl Backend for Bridge {
    fn tree(&self) -> Result<Vec<ElementInfo>, String> {
        match self.call(Call::Tree)? {
            Answer::Tree(tree) => Ok(tree),
            _ => Err("internal: unexpected answer".to_string()),
        }
    }

    fn state(&self) -> Result<Value, String> {
        self.value(Call::State)
    }

    fn click(&self, id: &str) -> Result<Value, String> {
        self.value(Call::Click(id.to_string()))
    }

    fn actions(&self) -> Result<Vec<ActionInfo>, String> {
        match self.call(Call::Actions)? {
            Answer::Actions(actions) => Ok(actions),
            _ => Err("internal: unexpected answer".to_string()),
        }
    }

    fn run_action(&self, name: &str, args: Value) -> Result<Value, String> {
        self.value(Call::RunAction {
            name: name.to_string(),
            args,
        })
    }

    fn feed_wav(&self, path: &Path) -> Result<Value, String> {
        self.value(Call::FeedWav(path.to_path_buf()))
    }
}

/// Starts the hook listener. Returns the server, which removes its socket
/// when dropped, and the queue the main thread must serve with [`attach`].
/// A hook that cannot start is logged and the app runs without it.
pub fn start(data: &DataDir) -> Option<(HookServer, UnboundedReceiver<Job>)> {
    let endpoint = match endpoint::resolve_from_env(Some(data.root())) {
        Ok(endpoint) => endpoint,
        Err(error) => {
            eprintln!("hushpen: the test hook is off: {error}");
            log::error!("TESTHOOK_OFF {error}");
            return None;
        }
    };
    let (jobs, queue) = mpsc::unbounded();
    let router = Router::new(Bridge {
        jobs: Mutex::new(jobs),
    });
    match HookServer::start(endpoint.clone(), std::sync::Arc::new(router)) {
        Ok(server) => {
            log::info!("TESTHOOK_LISTENING {endpoint}");
            Some((server, queue))
        }
        Err(error) => {
            eprintln!("hushpen: the test hook could not listen on {endpoint}: {error}");
            log::error!("TESTHOOK_OFF could not listen on {endpoint}: {error}");
            None
        }
    }
}

/// What the main thread needs to answer for the window.
pub struct Surface {
    pub window: AnyWindowHandle,
    pub shell: Entity<Shell>,
    /// Current settings values; secrets must appear only as booleans.
    pub settings: Rc<dyn Fn() -> Map<String, Value>>,
}

/// Installs the registries with the built-in actions and state sections, then
/// answers queued jobs on the main thread.
pub fn attach(cx: &mut App, queue: UnboundedReceiver<Job>, surface: Surface) {
    install(cx, &surface);
    let window = surface.window;
    cx.spawn(async move |cx| {
        let mut queue = queue;
        while let Some(job) = queue.next().await {
            let answer = cx.update(|cx| answer(cx, window, job.call));
            let _ = job.reply.send(answer);
        }
    })
    .detach();
}

/// Sets up the registries. Separate from [`attach`] so tests can use them
/// without a listener.
pub fn install(cx: &mut App, surface: &Surface) {
    let mut hooks = Hooks {
        actions: ActionRegistry::default(),
        state: StateRegistry::default(),
        feed_wav: None,
    };

    let window = surface.window;
    let shell = surface.shell.clone();
    let settings = surface.settings.clone();
    hooks.state.set_section(
        "app",
        |_| json!({"name": "hushpen", "version": hushpen_core::BUILD_VERSION}),
    );
    hooks.state.set_section("view", {
        let shell = shell.clone();
        move |cx| json!(shell.read(cx).active().key())
    });
    hooks
        .state
        .set_section("settings", move |_| Value::Object(settings()));
    hooks
        .state
        .set_section("window", move |cx| window_section(cx, window));
    // Features replace these with real readings when they exist.
    hooks
        .state
        .set_section("pipeline", |_| json!({"state": "idle", "mode": null}));
    hooks
        .state
        .set_section("engine", |_| json!({"pid": null, "state": null}));
    hooks
        .state
        .set_section("llm", |_| json!({"pid": null, "state": null}));
    hooks.state.set_section("tray_menu", |_| Value::Null);
    hooks.state.set_section("last_insert", |_| Value::Null);

    let _ = hooks.actions.register(
        "open-view",
        "Open a main view like a sidebar click. Args: {\"view\": \"home|history|dictionary|import|models|settings|about\"}",
        move |cx, args| open_view(cx, &shell, &args),
    );
    cx.set_global(hooks);
}

fn open_view(cx: &mut App, shell: &Entity<Shell>, args: &Value) -> Result<Value, String> {
    let key = args
        .get("view")
        .and_then(Value::as_str)
        .ok_or_else(|| "needs args like {\"view\": \"models\"}".to_string())?;
    let view = View::from_key(key).ok_or_else(|| {
        let known: Vec<_> = View::ALL.iter().map(|view| view.key()).collect();
        format!("unknown view '{key}'; one of: {}", known.join(", "))
    })?;
    shell.update(cx, |shell, cx| shell.select(view, cx));
    Ok(json!({"view": view.key()}))
}

fn window_section(cx: &mut App, window: AnyWindowHandle) -> Value {
    window
        .update(cx, |_, window, _| window_state(window))
        .unwrap_or_else(|_| json!({"open": false}))
}

pub fn window_state(window: &Window) -> Value {
    let size = window.viewport_size();
    json!({
        "open": true,
        "width": round(f32::from(size.width)),
        "height": round(f32::from(size.height)),
        "resizable": window.is_resizable(),
        "maximized": window.is_maximized(),
        "fullscreen": window.is_fullscreen(),
        "active": window.is_window_active(),
    })
}

fn answer(cx: &mut App, window: AnyWindowHandle, call: Call) -> Result<Answer, String> {
    match call {
        Call::Tree => window
            .update(cx, |_, window, _| Answer::Tree(tree(window)))
            .map_err(|_| "the window is closed".to_string()),
        Call::State => {
            Ok(Answer::Value(cx.update_global::<Hooks, _>(|hooks, cx| {
                hooks.state.snapshot(cx)
            })))
        }
        Call::Click(id) => window
            .update(cx, |_, window, cx| click(window, cx, &id))
            .map_err(|_| "the window is closed".to_string())?
            .map(Answer::Value),
        Call::Actions => Ok(Answer::Actions(cx.global::<Hooks>().actions.list())),
        Call::RunAction { name, args } => cx
            .update_global::<Hooks, _>(|hooks, cx| hooks.actions.run(cx, &name, args))
            .map(Answer::Value),
        Call::FeedWav(path) => cx
            .update_global::<Hooks, _>(|hooks, cx| match &hooks.feed_wav {
                Some(feed) => feed(cx, &path),
                None => Err("no capture path accepts test audio yet".to_string()),
            })
            .map(Answer::Value),
    }
}

fn round(value: f32) -> f64 {
    (f64::from(value) * 100.0).round() / 100.0
}

fn bounds_of(bounds: gpui_kit::Bounds<gpui_kit::Pixels>, origin: (f64, f64)) -> Bounds {
    Bounds {
        x: round(f32::from(bounds.origin.x)) + origin.0,
        y: round(f32::from(bounds.origin.y)) + origin.1,
        width: round(f32::from(bounds.size.width)),
        height: round(f32::from(bounds.size.height)),
    }
}

fn id_of(snapshot: &ElementSnapshot) -> String {
    snapshot
        .path()
        .last()
        .map(ToString::to_string)
        .unwrap_or_default()
}

/// Elements that called `.test_support()` in the last frame, sorted by id.
pub fn tree(window: &Window) -> Vec<ElementInfo> {
    let window_bounds = window.bounds();
    let origin = (
        round(f32::from(window_bounds.origin.x)),
        round(f32::from(window_bounds.origin.y)),
    );
    let mut elements: Vec<_> = snapshots(window)
        .iter()
        .map(|snapshot| ElementInfo {
            id: id_of(snapshot),
            role: snapshot.role().map(|role| format!("{role:?}")),
            text: snapshot
                .label()
                .or(snapshot.value())
                .unwrap_or_default()
                .to_string(),
            bounds: bounds_of(snapshot.bounds(), (0.0, 0.0)),
            root_bounds: bounds_of(snapshot.bounds(), origin),
            enabled: snapshot.disabled() != Some(true) && !is_disabled(&id_of(snapshot)),
            focused: is_focused(snapshot),
            visible: snapshot.visible(),
        })
        .collect();
    elements.sort_by(|a, b| a.id.cmp(&b.id));
    elements
}

/// `focused()` panics for an element that offers focus but whose binding was
/// never observed. For the hook that just means "not focused".
fn is_focused(snapshot: &ElementSnapshot) -> bool {
    panic::catch_unwind(AssertUnwindSafe(|| snapshot.focused()))
        .ok()
        .flatten()
        .unwrap_or(false)
}

/// Clicks the center of an element with real mouse events, so the normal
/// handlers run.
pub fn click(window: &mut Window, cx: &mut App, id: &str) -> Result<Value, String> {
    let target = ElementId::Name(id.to_string().into());
    let matches: Vec<_> = snapshots(window)
        .into_iter()
        .filter(|snapshot| snapshot.path().last() == Some(&target))
        .collect();
    let snapshot = match matches.as_slice() {
        [one] => one,
        [] => {
            let mut known: Vec<_> = snapshots(window).iter().map(id_of).collect();
            known.sort();
            return Err(format!(
                "no element '{id}'; known ids: {}",
                known.join(", ")
            ));
        }
        _ => return Err(format!("{} elements share the id '{id}'", matches.len())),
    };
    if !snapshot.visible() {
        return Err(format!("element '{id}' is not visible"));
    }
    if snapshot.disabled() == Some(true) || is_disabled(id) {
        return Err(format!("element '{id}' is disabled"));
    }
    let position = snapshot.bounds().center();
    window.dispatch_event(
        MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers: Default::default(),
        }
        .to_platform_input(),
        cx,
    );
    window.dispatch_event(
        MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        }
        .to_platform_input(),
        cx,
    );
    window.dispatch_event(
        MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers: Default::default(),
            click_count: 1,
        }
        .to_platform_input(),
        cx,
    );
    Ok(json!({
        "clicked": id,
        "x": round(f32::from(position.x)),
        "y": round(f32::from(position.y)),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::space;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        AppContext, Bounds as GpuiBounds, Point, TestAppContext, WindowBounds, WindowOptions, px,
        size,
    };
    use hushpen_store::settings::SettingsStore;

    struct Fixture {
        window: AnyWindowHandle,
        shell: Entity<Shell>,
        _dir: tempfile::TempDir,
    }

    fn open(cx: &mut TestAppContext) -> Fixture {
        cx.update(gpui_kit::init);
        let (window, shell) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(GpuiBounds {
                        origin: Point::default(),
                        size: size(px(space::WINDOW_WIDTH), px(space::WINDOW_HEIGHT)),
                    })),
                    is_resizable: false,
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| Shell::new(window, cx)),
            )
            .expect("open test window")
        });
        let dir = tempfile::tempdir().unwrap();
        let store = Rc::new(SettingsStore::open(dir.path()).unwrap());
        cx.update(|cx| {
            install(
                cx,
                &Surface {
                    window,
                    shell: shell.clone(),
                    settings: Rc::new(move || store.values()),
                },
            )
        });
        render(cx, window);
        Fixture {
            window,
            shell,
            _dir: dir,
        }
    }

    fn render(cx: &mut TestAppContext, window: AnyWindowHandle) {
        cx.update_window(window, |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.run_until_parked();
    }

    fn ask(cx: &mut TestAppContext, fixture: &Fixture, call: Call) -> Result<Answer, String> {
        let result = cx.update(|cx| answer(cx, fixture.window, call));
        cx.run_until_parked();
        render(cx, fixture.window);
        result
    }

    fn state(cx: &mut TestAppContext, fixture: &Fixture) -> Value {
        match ask(cx, fixture, Call::State) {
            Ok(Answer::Value(state)) => state,
            _ => panic!("state failed"),
        }
    }

    fn element(tree: &[ElementInfo], id: &str) -> ElementInfo {
        tree.iter()
            .find(|element| element.id == id)
            .unwrap_or_else(|| panic!("no element {id}"))
            .clone()
    }

    fn tree_of(cx: &mut TestAppContext, fixture: &Fixture) -> Vec<ElementInfo> {
        match ask(cx, fixture, Call::Tree) {
            Ok(Answer::Tree(tree)) => tree,
            _ => panic!("tree failed"),
        }
    }

    #[gpui_kit::test]
    fn the_tree_lists_every_sidebar_item_with_text_and_bounds_in_the_sidebar(
        cx: &mut TestAppContext,
    ) {
        let fixture = open(cx);
        let tree = tree_of(cx, &fixture);
        for (index, view) in View::ALL.iter().enumerate() {
            let item = element(&tree, &format!("sidebar.{}", view.key()));
            assert_eq!(item.text, view.title());
            assert!(item.enabled && item.visible);
            assert!(item.bounds.x >= 0.0 && item.bounds.x + item.bounds.width <= 208.0);
            assert_eq!(item.bounds.height, 36.0);
            assert_eq!(item.root_bounds.x, item.bounds.x);
            assert!(!item.focused);
            if index > 0 {
                let above = element(&tree, &format!("sidebar.{}", View::ALL[index - 1].key()));
                assert!(item.bounds.y > above.bounds.y);
            }
        }
        for name in ["window.close", "window.minimize"] {
            let control = element(&tree, name);
            assert!(!control.text.is_empty(), "{name} has a name");
            assert!(control.visible);
        }
    }

    #[gpui_kit::test]
    fn the_tree_ids_are_the_same_in_two_launches(cx: &mut TestAppContext) {
        let first = open(cx);
        let ids = |tree: Vec<ElementInfo>| tree.into_iter().map(|e| e.id).collect::<Vec<_>>();
        let first_ids = ids(tree_of(cx, &first));
        let second = open(cx);
        let second_ids = ids(tree_of(cx, &second));
        assert!(first_ids.contains(&"sidebar.home".to_string()));
        assert_eq!(first_ids, second_ids);
    }

    #[gpui_kit::test]
    fn the_focused_sidebar_item_reports_focus(cx: &mut TestAppContext) {
        let fixture = open(cx);
        cx.update_window(fixture.window, |_, window, cx| window.press("tab", cx))
            .unwrap();
        cx.run_until_parked();
        let tree = tree_of(cx, &fixture);
        let focused: Vec<_> = tree
            .iter()
            .filter(|e| e.focused)
            .map(|e| e.id.clone())
            .collect();
        assert_eq!(focused, ["sidebar.home"]);
    }

    #[gpui_kit::test]
    fn clicking_each_sidebar_item_changes_the_view_and_the_state_agrees(cx: &mut TestAppContext) {
        let fixture = open(cx);
        for view in View::ALL.iter().rev() {
            let id = format!("sidebar.{}", view.key());
            ask(cx, &fixture, Call::Click(id)).unwrap();
            assert_eq!(
                fixture.shell.read_with(cx, |shell, _| shell.active()),
                *view
            );
            assert_eq!(state(cx, &fixture)["view"], view.key());
        }
    }

    #[gpui_kit::test]
    fn clicking_an_unknown_id_names_the_known_ones(cx: &mut TestAppContext) {
        let fixture = open(cx);
        let Err(error) = ask(cx, &fixture, Call::Click("sidebar.nope".into())) else {
            panic!("expected an error");
        };
        assert!(error.contains("no element 'sidebar.nope'"), "{error}");
        assert!(error.contains("sidebar.home"), "{error}");
    }

    #[gpui_kit::test]
    fn state_reports_settings_window_pipeline_engine_tray_and_last_insert(cx: &mut TestAppContext) {
        let fixture = open(cx);
        let state = state(cx, &fixture);
        assert_eq!(state["settings"]["dictation.maxMinutes"], 6);
        assert_eq!(state["window"]["width"], 780.0);
        assert_eq!(state["window"]["height"], 520.0);
        assert_eq!(state["window"]["resizable"], false);
        assert_eq!(state["window"]["maximized"], false);
        assert_eq!(state["window"]["fullscreen"], false);
        assert_eq!(state["pipeline"]["state"], "idle");
        assert!(state["engine"]["pid"].is_null());
        assert!(state["tray_menu"].is_null());
        assert!(state["last_insert"].is_null());
        assert_eq!(state["view"], "home");
        assert_eq!(state["app"]["version"], hushpen_core::BUILD_VERSION);
    }

    #[gpui_kit::test]
    fn a_feature_can_replace_a_state_section(cx: &mut TestAppContext) {
        let fixture = open(cx);
        cx.update(|cx| {
            set_state_section(cx, "pipeline", |_| json!({"state": "listening"}));
        });
        assert_eq!(state(cx, &fixture)["pipeline"]["state"], "listening");
    }

    #[gpui_kit::test]
    fn actions_list_and_open_view_runs_through_the_shell(cx: &mut TestAppContext) {
        let fixture = open(cx);
        let Ok(Answer::Actions(actions)) = ask(cx, &fixture, Call::Actions) else {
            panic!("actions failed");
        };
        assert!(actions.iter().any(|action| action.name == "open-view"));

        let call = Call::RunAction {
            name: "open-view".into(),
            args: json!({"view": "models"}),
        };
        ask(cx, &fixture, call).unwrap();
        assert_eq!(
            fixture.shell.read_with(cx, |shell, _| shell.active()),
            View::Models
        );
        assert_eq!(state(cx, &fixture)["view"], "models");
    }

    #[gpui_kit::test]
    fn open_view_refuses_unknown_views_and_missing_arguments(cx: &mut TestAppContext) {
        let fixture = open(cx);
        for args in [json!({"view": "nowhere"}), Value::Null] {
            let call = Call::RunAction {
                name: "open-view".into(),
                args,
            };
            assert!(ask(cx, &fixture, call).is_err());
        }
        let unknown = Call::RunAction {
            name: "no-such-action".into(),
            args: Value::Null,
        };
        let Err(error) = ask(cx, &fixture, unknown) else {
            panic!("expected an error");
        };
        assert!(error.contains("open-view"), "{error}");
    }

    #[gpui_kit::test]
    fn a_feature_can_register_its_own_action(cx: &mut TestAppContext) {
        let fixture = open(cx);
        cx.update(|cx| {
            register_action(cx, "echo", "Echo the args", |_, args| Ok(args)).unwrap();
            assert!(register_action(cx, "echo", "again", |_, _| Ok(Value::Null)).is_err());
        });
        let call = Call::RunAction {
            name: "echo".into(),
            args: json!({"a": 1}),
        };
        let Ok(Answer::Value(value)) = ask(cx, &fixture, call) else {
            panic!("echo failed");
        };
        assert_eq!(value["a"], 1);
    }

    #[gpui_kit::test]
    fn feed_wav_is_refused_until_a_capture_path_registers(cx: &mut TestAppContext) {
        let fixture = open(cx);
        let Err(error) = ask(cx, &fixture, Call::FeedWav("/tmp/x.wav".into())) else {
            panic!("expected an error");
        };
        assert!(error.contains("no capture path"), "{error}");
        cx.update(|cx| set_wav_feeder(cx, |_, path| Ok(json!({"fed": path.to_string_lossy()}))));
        let Ok(Answer::Value(value)) = ask(cx, &fixture, Call::FeedWav("/tmp/x.wav".into())) else {
            panic!("feed failed");
        };
        assert_eq!(value["fed"], "/tmp/x.wav");
    }
}
