//! Offscreen pixel tests: the real window at 940x600 rendered by GPUI's wgpu
//! headless renderer (Mesa lavapipe), with no X server and no display.
//!
//! Linux containers only: `cargo test -p hushpen-app --features pixel-tests
//! -- --test-threads=2` (services.yaml `test-pixel`). Captures are at scale 2,
//! so a 940x600 window is 1880x1200 pixels and sample points scale with
//! `width / 940`.
//!
//! Environment:
//! - `HUSHPEN_PIXEL_BASELINES`: folder with the approved PNGs. Default:
//!   `tests/pixel/baselines` in the crate.
//! - `HUSHPEN_PIXEL_OUT`: where actual and diff images go on a failure.
//!   Default: the temp folder (the container sets `TMPDIR` to the data drive).
//! - `HUSHPEN_PIXEL_UPDATE=1`: write the captures into the baselines folder
//!   instead of comparing. The repo mounts read-only in the container, so an
//!   update run points `HUSHPEN_PIXEL_BASELINES` at a writable folder first.
#![cfg(feature = "pixel-tests")]

mod compare;

use compare::{Outcome, compare, diff_image};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext as _, HeadlessAppContext, ScrollDelta, point, px, size};
use gpui_wgpu::CosmicTextSystem;
use hushpen_app::about::{About, SystemOpener};
use hushpen_app::assets::{AppAssets, register_fonts};
use hushpen_app::controller::{Controller, Engine, KeysStatus, monotonic_clock};
use hushpen_app::dictation::Dictation;
use hushpen_app::flow_bar::view::{Picker, Preview, Props, size_of};
use hushpen_app::mic::{CpalBackend, Mic, MicBackend, MicSession};
use hushpen_app::models::{self, Models};
use hushpen_app::net::endpoint::Transport;
use hushpen_app::net::endpoint::UreqTransport;
use hushpen_app::onboarding::{Onboarding, Parts as OnboardingParts, SystemGuide};
use hushpen_app::settings::endpoint::EndpointForm;
use hushpen_app::settings::{Parts as SettingsParts, Section, Settings};
use hushpen_app::shell::Shell;
use hushpen_app::shortcuts::Shortcuts;
use hushpen_app::storage;
use hushpen_app::theme::{self, Appearance, chrome, space};
use hushpen_app::views::View;
use hushpen_audio::{CaptureError, EventSink, InputDevice};
use hushpen_core::flow_bar::BarState;
use hushpen_core::onboarding::{Session, Step};
use hushpen_core::permission::{Access, Preflight};
use hushpen_core::shortcut::Platform;
use hushpen_engine::JobOutcome;
use hushpen_store::data_dir::DataDir;
use image::RgbaImage;
use serde_json::json;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

fn baselines_dir() -> PathBuf {
    std::env::var_os("HUSHPEN_PIXEL_BASELINES")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/pixel/baselines"))
}

fn out_dir() -> PathBuf {
    std::env::var_os("HUSHPEN_PIXEL_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("hushpen-pixel"))
}

fn updating() -> bool {
    std::env::var_os("HUSHPEN_PIXEL_UPDATE").is_some_and(|value| value == "1")
}

fn headless() -> HeadlessAppContext {
    headless_in(Appearance::Dark)
}

/// A headless app with the palette forced, so a capture does not depend on the desktop.
fn headless_in(appearance: Appearance) -> HeadlessAppContext {
    // The fallback family is only for glyphs Inter lacks; the app embeds Inter.
    let text = Arc::new(CosmicTextSystem::new("DejaVu Sans"));
    let mut cx = HeadlessAppContext::with_platform(text, Arc::new(AppAssets), || {
        gpui_kit::platform::current_headless_renderer()
    });
    cx.update(|cx| {
        gpui_kit::init(cx);
        register_fonts(cx);
        theme::install(cx);
        theme::force(Some(appearance), cx);
    });
    cx
}

/// Hex color at a point given in logical pixels.
fn hex_at(image: &RgbaImage, x: u32, y: u32) -> String {
    let scale = image.width() / space::WINDOW_WIDTH as u32;
    let [r, g, b, _] = image.get_pixel(x * scale, y * scale).0;
    format!("#{r:02X}{g:02X}{b:02X}")
}

/// A point inside a sidebar row's fill, right of its label. Rows are 28 px high, 2 px apart,
/// and the first one starts below the title bar strip.
fn nav_fill_point(index: u32) -> (u32, u32) {
    (200, space::SIDEBAR_TOP as u32 + 30 * index + 14)
}

/// `color` (`0xRRGGBBAA`) laid over the opaque `base`, as `#RRGGBB`.
fn over(color: u32, base: u32) -> String {
    let alpha = (color & 0xFF) as f32 / 255.0;
    let mix = |shift: u32| {
        let top = ((color >> (24 - shift)) & 0xFF) as f32;
        let under = ((base >> (16 - shift)) & 0xFF) as f32;
        (top * alpha + under * (1.0 - alpha)).round() as u32
    };
    format!("#{:02X}{:02X}{:02X}", mix(0), mix(8), mix(16))
}

fn channels(hex: &str) -> [i32; 3] {
    let value = i32::from_str_radix(hex.trim_start_matches('#'), 16).expect("a hex color");
    [value >> 16 & 0xFF, value >> 8 & 0xFF, value & 0xFF]
}

/// Asserts each channel of `actual` is within `tolerance` of `want`.
fn assert_near(actual: &str, want: &str, tolerance: i32, what: &str) {
    let (a, w) = (channels(actual), channels(want));
    assert!(
        (0..3).all(|index| (a[index] - w[index]).abs() <= tolerance),
        "{what}: {actual} is not within {tolerance} of {want}"
    );
}

struct Capture {
    view: View,
    image: RgbaImage,
}

/// The data folder of a capture: one fixed path per test, so the folder text
/// in the Settings and About rows reads the same in every run. A random
/// tempdir would put its changing suffix into the baselines.
fn pixel_data_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("hushpen-pixel").join(tag);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// A shell with the Settings and About entities the real app attaches, so
/// those views render their content, not the placeholder.
fn open_shell_with_settings(
    cx: &mut HeadlessAppContext,
    data: &std::path::Path,
) -> gpui_kit::AnyWindowHandle {
    open_shell(cx, data, None)
}

/// The same, with the endpoint rows of Cleanup connected to `transport`.
fn open_shell(
    cx: &mut HeadlessAppContext,
    data: &std::path::Path,
    transport: Option<Arc<dyn Transport>>,
) -> gpui_kit::AnyWindowHandle {
    let storage = Rc::new(
        storage::open(DataDir::open(data.join("data")).expect("open the data folder"))
            .expect("open the storage"),
    );
    let handle = cx
        .open_window(
            size(px(space::WINDOW_WIDTH), px(space::WINDOW_HEIGHT)),
            |window, cx| {
                let shell = cx.new(|cx| Shell::new(window, cx));
                let mic = cx.new(|cx| Mic::new(Rc::clone(&storage), Arc::new(CpalBackend), cx));
                let shortcuts =
                    cx.new(|cx| Shortcuts::new(Rc::clone(&storage), Platform::Linux, cx));
                let settings = cx.new(|cx| {
                    Settings::new(
                        SettingsParts {
                            storage: Rc::clone(&storage),
                            mic,
                            login: None,
                            default_data_dir: None,
                            restart: None,
                        },
                        cx,
                    )
                });
                let about = cx.new(|cx| About::new(Rc::clone(&storage), Rc::new(SystemOpener), cx));
                if let Some(transport) = transport {
                    let form =
                        cx.new(|cx| EndpointForm::new(Rc::clone(&storage), transport, window, cx));
                    settings.update(cx, |settings, cx| settings.attach_endpoint(form, cx));
                }
                shell.update(cx, |shell, cx| {
                    shell.attach_shortcuts(shortcuts, cx);
                    shell.attach_settings(settings, cx);
                    shell.attach_about(about, cx);
                });
                cx.new(|cx| gpui_kit::base::Root::new(shell, window, cx))
            },
        )
        .expect("open the headless window");
    handle.into()
}

fn capture_every_view() -> Vec<Capture> {
    let mut cx = headless();
    let handle = open_shell_with_settings(&mut cx, &pixel_data_dir("views"));
    cx.run_until_parked();
    // Home has its own Light and Dark baselines, taken with the panels the app attaches.
    View::ALL
        .into_iter()
        .filter(|view| *view != View::Home)
        .map(|view| {
            let id: &'static str = Box::leak(format!("sidebar.{}", view.key()).into_boxed_str());
            cx.update_window(handle, |_, window, cx| window.click(id, cx))
                .expect("click the sidebar item");
            cx.run_until_parked();
            let image = cx.capture_screenshot(handle).expect("capture the window");
            Capture { view, image }
        })
        .collect()
}

/// The Settings view opened on `section`, captured as `settings-<key>.png`.
fn capture_settings_section(section: Section) -> RgbaImage {
    let mut cx = headless();
    let handle = open_shell_with_settings(&mut cx, &pixel_data_dir("sections"));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.click("sidebar.settings", cx);
        if section != Section::General {
            window.click(
                Box::leak(format!("settings.section.{}", section.key()).into_boxed_str())
                    as &'static str,
                cx,
            );
        }
    })
    .expect("drive the window");
    cx.run_until_parked();
    cx.capture_screenshot(handle).expect("capture the window")
}

/// Cleanup on the endpoint, with a long failure reason in the connection status row. The row
/// wraps to two lines and keeps its button.
fn capture_endpoint_failure() -> RgbaImage {
    let mut cx = headless();
    let handle = open_shell(
        &mut cx,
        &pixel_data_dir("endpoint-failure"),
        Some(Arc::new(UreqTransport)),
    );
    cx.run_until_parked();
    let other_computer = format!("http://{}:11434/v1", "10.0.0.2");
    cx.update_window(handle, |_, window, cx| {
        window.click("sidebar.settings", cx);
        window.click("settings.section.cleanup", cx);
    })
    .expect("open Cleanup");
    cx.run_until_parked();
    for step in [
        "settings.cleanup.provider",
        "settings.cleanup.provider.option.endpoint",
    ] {
        click_scrolled(&mut cx, handle, step);
    }
    click_scrolled(&mut cx, handle, "settings.cleanup.endpointUrl");
    cx.update_window(handle, |_, window, cx| window.input(&other_computer, cx))
        .expect("type the address");
    cx.run_until_parked();
    click_scrolled(&mut cx, handle, "settings.cleanup.endpointTest");
    cx.update_window(handle, |_, window, cx| {
        window.scroll(
            "content.scroll",
            ScrollDelta::Pixels(point(px(0.0), px(-5_000.0))),
            cx,
        )
    })
    .expect("scroll to the status row");
    cx.run_until_parked();
    cx.capture_screenshot(handle).expect("capture the window")
}

/// Clicks `id`, scrolling the page first like the mouse wheel does.
fn click_scrolled(
    cx: &mut HeadlessAppContext,
    handle: gpui_kit::AnyWindowHandle,
    id: &'static str,
) {
    let scroll = |cx: &mut HeadlessAppContext, pixels: f32| {
        cx.update_window(handle, |_, window, cx| {
            window.scroll(
                "content.scroll",
                ScrollDelta::Pixels(point(px(0.0), px(pixels))),
                cx,
            )
        })
        .expect("scroll the page");
        cx.run_until_parked();
    };
    scroll(cx, 5_000.0);
    for _ in 0..40 {
        let shown = cx
            .update_window(handle, |_, window, _| {
                window.try_find(id).is_some_and(|found| found.visible())
            })
            .expect("look for the control");
        if shown {
            break;
        }
        scroll(cx, -120.0);
    }
    cx.update_window(handle, |_, window, cx| window.click(id, cx))
        .expect("click the control");
    cx.run_until_parked();
}

#[test]
fn the_endpoint_status_row_wraps_a_long_failure_to_two_lines() {
    let image = capture_endpoint_failure();
    assert_eq!(image.dimensions(), (1880, 1200));
    let mut failures = Vec::new();
    check_image("settings-cleanup-endpoint-failure", &image, &mut failures);
    assert!(
        failures.is_empty(),
        "pixel baselines failed:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_settings_section_matches_its_approved_baseline() {
    let mut failures = Vec::new();
    for section in Section::ALL {
        let image = capture_settings_section(section);
        assert_eq!(image.dimensions(), (1880, 1200), "{}", section.key());
        check_image(
            &format!("settings-{}", section.key()),
            &image,
            &mut failures,
        );
    }
    assert!(
        failures.is_empty(),
        "pixel baselines failed:\n{}",
        failures.join("\n")
    );
}

fn check_against_baseline(capture: &Capture, failures: &mut Vec<String>) {
    check_image(capture.view.key(), &capture.image, failures);
}

fn check_image(name: &str, image: &RgbaImage, failures: &mut Vec<String>) {
    let path = baselines_dir().join(format!("{name}.png"));
    if updating() {
        std::fs::create_dir_all(baselines_dir()).expect("create the baselines folder");
        image.save(&path).expect("write the baseline");
        return;
    }
    let baseline = match image::open(&path) {
        Ok(baseline) => baseline.to_rgba8(),
        Err(error) => {
            failures.push(format!(
                "{name}: no baseline at {} ({error}); approve one with HUSHPEN_PIXEL_UPDATE=1",
                path.display()
            ));
            return;
        }
    };
    let outcome = compare(image, &baseline);
    if outcome.passed() {
        return;
    }
    let out = out_dir();
    std::fs::create_dir_all(&out).expect("create the output folder");
    let actual_path = out.join(format!("{name}-actual.png"));
    let diff_path = out.join(format!("{name}-diff.png"));
    image.save(&actual_path).expect("write the capture");
    diff_image(image, &baseline)
        .save(&diff_path)
        .expect("write the diff");
    let why = match outcome {
        Outcome::SizeChanged { actual, baseline } => {
            format!("size {actual:?} but the baseline is {baseline:?}")
        }
        Outcome::Different {
            pixels,
            largest_step,
        } => format!("{pixels} pixels differ (largest channel step {largest_step})"),
        Outcome::Same => unreachable!("a passing outcome returned above"),
    };
    failures.push(format!("{name}: {why}; diff {}", diff_path.display()));
}

#[test]
fn every_view_matches_its_approved_baseline() {
    let captures = capture_every_view();
    let mut failures = Vec::new();
    for capture in &captures {
        check_against_baseline(capture, &mut failures);
    }
    assert!(
        failures.is_empty(),
        "pixel baselines failed:\n{}",
        failures.join("\n")
    );
}

#[test]
fn captures_are_940x600_at_scale_two_and_use_the_design_tokens() {
    let captures = capture_every_view();
    for capture in &captures {
        assert_eq!(
            capture.image.dimensions(),
            (1880, 1200),
            "{}",
            capture.view.key()
        );
    }
    for capture in &captures {
        assert_shell_tokens(
            &capture.image,
            Appearance::Dark,
            Some(capture.view),
            capture.view.key(),
        );
    }
}

/// The window chrome that every shell capture shares: the content and sidebar colors, the
/// drawn window lights on Linux, a neutral selection fill on the active row only, and a sidebar
/// with no color anywhere (icons and labels are gray). The toolbar has no line under it.
fn assert_shell_tokens(
    image: &RgbaImage,
    appearance: Appearance,
    selected: Option<View>,
    name: &str,
) {
    let (content, selection) = match appearance {
        Appearance::Dark => (theme::dark::CONTENT, theme::dark::SELECTION),
        Appearance::Light => (theme::light::CONTENT, theme::light::SELECTION),
    };
    let content = format!("{:06X}", content >> 8);
    let sidebar_base = chrome::sidebar_fill(false, appearance.is_dark()).expect("a solid sidebar");
    // Between the toolbar and the first panel: the content color in every view.
    assert_eq!(
        hex_at(image, 500, 47),
        format!("#{content}"),
        "{name}: content"
    );
    assert_eq!(
        hex_at(image, 100, 450),
        format!("#{sidebar_base:06X}"),
        "{name}: sidebar"
    );
    assert_eq!(hex_at(image, 26, 26), "#FF5F57", "{name}: close light");
    assert_eq!(hex_at(image, 46, 26), "#FEBC2E", "{name}: minimize light");
    let selection = over(selection, sidebar_base);
    for (index, view) in View::ALL.into_iter().enumerate() {
        let (x, y) = nav_fill_point(index as u32);
        let want = if Some(view) == selected {
            selection.clone()
        } else {
            format!("#{sidebar_base:06X}")
        };
        assert_near(
            &hex_at(image, x, y),
            &want,
            1,
            &format!("{name}: sidebar item {}", view.key()),
        );
    }
    let scale = image.width() / space::WINDOW_WIDTH as u32;
    for y in (space::SIDEBAR_TOP as u32 * scale)..image.height() {
        for x in 0..(space::SIDEBAR_WIDTH as u32 * scale) {
            let [r, g, b, _] = image.get_pixel(x, y).0;
            let spread = r.max(g).max(b) - r.min(g).min(b);
            assert!(
                spread <= 6,
                "{name}: the sidebar has a colored pixel {r},{g},{b} at ({x}, {y})"
            );
        }
    }
    // The toolbar title sits on the content color: no line under the toolbar at its bottom edge.
    let toolbar_bottom = space::TOOLBAR_HEIGHT as u32;
    for x in (space::SIDEBAR_WIDTH as u32 + 4)..(space::WINDOW_WIDTH as u32 - 4) {
        assert_eq!(
            hex_at(image, x, toolbar_bottom),
            hex_at(image, x, toolbar_bottom - 1),
            "{name}: a line under the toolbar at x {x}"
        );
    }
}

/// What a Light and Dark baseline shows.
#[derive(Debug, Clone, Copy)]
enum Scene {
    /// The shell with its sidebar and title bar around a placeholder view.
    Shell,
    /// Home with the dictation and microphone panels the app attaches.
    Home,
    Onboarding(Step),
    /// Onboarding reopened by a revoked macOS permission.
    PermissionsChanged,
}

impl Scene {
    fn name(self) -> String {
        match self {
            Scene::Shell => "shell".into(),
            Scene::Home => "home".into(),
            Scene::Onboarding(step) => format!("onboarding-{}", step.key()),
            Scene::PermissionsChanged => "onboarding-permissions-changed".into(),
        }
    }

    fn all() -> Vec<Scene> {
        let mut scenes = vec![Scene::Shell, Scene::Home];
        scenes.extend(Step::ALL.into_iter().map(Scene::Onboarding));
        scenes.push(Scene::PermissionsChanged);
        scenes
    }
}

/// One default microphone, so the capture does not depend on the container's audio devices.
struct OneMicrophone;

impl MicBackend for OneMicrophone {
    fn list(&self) -> Result<Vec<InputDevice>, CaptureError> {
        Ok(vec![InputDevice {
            id: "built-in".into(),
            name: "Built-in Microphone".into(),
            is_default: true,
        }])
    }

    fn start(
        &self,
        _device: &str,
        _path: PathBuf,
        _sink: EventSink,
    ) -> Result<Box<dyn MicSession>, CaptureError> {
        Err(CaptureError::unavailable("a pixel test records nothing"))
    }
}

/// Microphone granted, Accessibility and Input Monitoring revoked.
struct Revoked;

impl Preflight for Revoked {
    fn microphone(&self) -> Access {
        Access::Granted
    }

    fn post_event(&self) -> Access {
        Access::Denied
    }

    fn listen_event(&self) -> Access {
        Access::Denied
    }
}

/// The main window showing `scene` in `appearance`, on a data folder with no models.
fn capture_scene(scene: Scene, appearance: Appearance) -> RgbaImage {
    // Tests run in parallel and each capture needs its own folder; these scenes show no path.
    static CAPTURES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let serial = CAPTURES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let data = pixel_data_dir(&format!("{}-{}-{serial}", scene.name(), appearance.key()));
    let storage = Rc::new(
        storage::open(DataDir::open(data.join("data")).expect("open the data folder"))
            .expect("open the storage"),
    );
    let mac = matches!(scene, Scene::PermissionsChanged);
    match scene {
        Scene::Onboarding(step) => storage
            .settings
            .set_internal("onboarding.step", json!(step.key()))
            .expect("store the step"),
        Scene::PermissionsChanged => {
            let settings = &storage.settings;
            settings
                .set_internal("onboarding.completed", json!(true))
                .expect("complete onboarding");
            settings
                .set_internal(
                    "permissions.lastGranted",
                    json!({"microphone": true, "accessibility": true, "inputMonitoring": true}),
                )
                .expect("store the earlier grants");
        }
        Scene::Shell | Scene::Home => {}
    }
    let mut cx = headless_in(appearance);
    let handle = cx
        .open_window(
            size(px(space::WINDOW_WIDTH), px(space::WINDOW_HEIGHT)),
            |window, cx| {
                let shell = cx.new(|cx| Shell::new(window, cx));
                let mic = cx.new(|cx| Mic::new(Rc::clone(&storage), Arc::new(OneMicrophone), cx));
                mic.update(cx, |mic, cx| mic.refresh(cx));
                let models = cx.new(|cx| {
                    Models::new(
                        Rc::clone(&storage),
                        hushpen_core::catalog::embedded(),
                        models::production_transfer(),
                        models::thread_spawner(),
                        None,
                        cx,
                    )
                });
                let engine = Engine::new(Arc::new(|_, _| JobOutcome::Cancelled), Rc::new(|| None));
                let controller = cx.new(|cx| {
                    Controller::new(
                        Rc::clone(&storage),
                        mic.clone(),
                        models.clone(),
                        engine,
                        models::thread_spawner(),
                        monotonic_clock(),
                        cx,
                    )
                });
                controller.update(cx, |controller, _| {
                    if mac {
                        controller.attach_permissions(Arc::new(Revoked));
                    }
                    controller.attach_keys(KeysStatus::Available, Rc::new(|_| {}));
                });
                match scene {
                    Scene::Onboarding(_) | Scene::PermissionsChanged => {
                        let onboarding = cx.new(|cx| {
                            Onboarding::new(
                                OnboardingParts {
                                    storage: Rc::clone(&storage),
                                    mic,
                                    models,
                                    controller,
                                    guide: Rc::new(SystemGuide),
                                    platform: if mac {
                                        Platform::MacOs
                                    } else {
                                        Platform::Linux
                                    },
                                    session: Session::X11,
                                },
                                cx,
                            )
                        });
                        shell.update(cx, |shell, cx| shell.attach_onboarding(onboarding, cx));
                    }
                    Scene::Home => {
                        let dictation = cx
                            .new(|cx| Dictation::new(Rc::clone(&storage), models, controller, cx));
                        shell.update(cx, |shell, cx| {
                            shell.attach_mic(mic, cx);
                            shell.attach_dictation(dictation, cx);
                        });
                    }
                    Scene::Shell => {}
                }
                cx.new(|cx| gpui_kit::base::Root::new(shell, window, cx))
            },
        )
        .expect("open the headless window");
    // The microphone list is read on a thread and arrives through a channel.
    std::thread::sleep(std::time::Duration::from_millis(300));
    cx.run_until_parked();
    if matches!(scene, Scene::Shell) {
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("sidebar.history", cx)
        })
        .expect("select a view without content");
        cx.run_until_parked();
    }
    cx.capture_screenshot(handle.into())
        .expect("capture the window")
}

fn check_scenes(scenes: &[Scene], appearance: Appearance) {
    let mut failures = Vec::new();
    for scene in scenes {
        let image = capture_scene(*scene, appearance);
        assert_eq!(image.dimensions(), (1880, 1200), "{}", scene.name());
        check_image(
            &format!("{}-{}", scene.name(), appearance.key()),
            &image,
            &mut failures,
        );
    }
    assert!(
        failures.is_empty(),
        "pixel baselines failed:\n{}",
        failures.join("\n")
    );
}

#[test]
fn the_shell_home_and_onboarding_match_their_approved_dark_baselines() {
    check_scenes(&Scene::all(), Appearance::Dark);
}

#[test]
fn the_shell_home_and_onboarding_match_their_approved_light_baselines() {
    check_scenes(&Scene::all(), Appearance::Light);
}

#[test]
fn light_and_dark_differ_and_follow_the_palette_for_every_scene() {
    for scene in Scene::all() {
        let dark = capture_scene(scene, Appearance::Dark);
        let light = capture_scene(scene, Appearance::Light);
        assert_ne!(dark, light, "{}", scene.name());
        let name = scene.name();
        assert_near(
            &hex_at(&dark, 900, 580),
            "#1E1E1E",
            1,
            &format!("{name}: dark content"),
        );
        assert_near(
            &hex_at(&light, 900, 580),
            "#F5F5F5",
            1,
            &format!("{name}: light content"),
        );
        if matches!(scene, Scene::Shell | Scene::Home) {
            let selected = if matches!(scene, Scene::Home) {
                View::Home
            } else {
                View::History
            };
            assert_shell_tokens(
                &dark,
                Appearance::Dark,
                Some(selected),
                &format!("{name}-dark"),
            );
            assert_shell_tokens(
                &light,
                Appearance::Light,
                Some(selected),
                &format!("{name}-light"),
            );
        }
    }
}

#[test]
fn onboarding_takes_the_whole_window_with_no_sidebar() {
    for (appearance, content) in [
        (Appearance::Dark, "#1E1E1E"),
        (Appearance::Light, "#F5F5F5"),
    ] {
        let image = capture_scene(Scene::Onboarding(Step::Permissions), appearance);
        assert_eq!(hex_at(&image, 100, 450), content, "no sidebar color");
        assert_eq!(hex_at(&image, 26, 26), "#FF5F57", "close light");
        assert_eq!(hex_at(&image, 46, 26), "#FEBC2E", "minimize light");
    }
}

fn flow_bar_states() -> Vec<(&'static str, Props)> {
    let mut listening = Props::new(BarState::Listening);
    listening.levels = (0..32)
        .map(|n| (0.5 + 0.5 * (n as f32 * 0.55).sin()) * (n as f32 / 31.0))
        .collect();
    let mut transcribing = Props::new(BarState::Transcribing);
    transcribing.dot = 1;
    let mut preparing = Props::new(BarState::Transcribing);
    preparing.dot = 1;
    preparing.preparing = true;
    let mut result = Props::new(BarState::Result);
    result.message = "Inserted".into();
    let mut undo = Props::new(BarState::Undo);
    undo.message = "Inserted".into();
    let mut error = Props::new(BarState::Error);
    error.message = "No speech heard".into();
    error.open_history = true;
    let mut blocked = Props::new(BarState::Blocked);
    blocked.message = "Another app holds the keyboard".into();
    blocked.open_history = true;
    let mut spanish = Props::new(BarState::Idle);
    spanish.language.label = "ES".into();
    let mut list = Props::new(BarState::Idle);
    list.picker = Picker::List {
        codes: vec!["auto", "es", "en", "de", "fr", "it"],
        selected: "es".into(),
        below: false,
    };
    let mut off = Props::new(BarState::Idle);
    off.language.enabled = false;
    off.language.reason = Some("This model understands English only.".into());
    off.picker = Picker::Reason {
        text: "This model understands English only.".into(),
        below: true,
    };
    vec![
        ("flowbar-idle", Props::new(BarState::Idle)),
        ("flowbar-idle-spanish", spanish),
        ("flowbar-listening", listening),
        ("flowbar-transcribing", transcribing),
        ("flowbar-transcribing-preparing", preparing),
        ("flowbar-result", result),
        ("flowbar-undo", undo),
        ("flowbar-error", error),
        ("flowbar-blocked", blocked),
        ("flowbar-picker", list),
        ("flowbar-picker-off", off),
    ]
}

fn capture_flow_bar(props: Props) -> RgbaImage {
    capture_flow_bar_in(props, Appearance::Dark)
}

fn capture_flow_bar_in(props: Props, appearance: Appearance) -> RgbaImage {
    let mut cx = headless_in(appearance);
    let (width, height) = size_of(&props);
    let handle = cx
        .open_window(size(px(width), px(height)), move |window, cx| {
            let bar = cx.new(|_| Preview(props));
            cx.new(|cx| gpui_kit::base::Root::new(bar, window, cx))
        })
        .expect("open the headless window");
    cx.run_until_parked();
    cx.capture_screenshot(handle.into())
        .expect("capture the window")
}

#[test]
fn every_flow_bar_state_matches_its_approved_baseline() {
    let mut failures = Vec::new();
    for (name, props) in flow_bar_states() {
        check_image(name, &capture_flow_bar(props), &mut failures);
    }
    assert!(
        failures.is_empty(),
        "pixel baselines failed:\n{}",
        failures.join("\n")
    );
}

#[test]
fn the_flow_bar_pill_stays_dark_in_a_light_appearance() {
    // The picker windows are taller; the pill is not at this point in them.
    for (name, props) in flow_bar_states()
        .into_iter()
        .filter(|(name, _)| !name.contains("picker"))
    {
        let light = capture_flow_bar_in(props, Appearance::Light);
        let [r, g, b, _] = light.get_pixel(24, 32).0;
        assert_eq!(
            format!("#{r:02X}{g:02X}{b:02X}"),
            "#2B2B2B",
            "{name}: the pill keeps its dark HUD color in Light"
        );
    }
}

#[test]
fn the_flow_bar_is_a_compact_dark_pill_at_scale_two() {
    let image = capture_flow_bar(Props::new(BarState::Listening));
    assert_eq!(image.dimensions(), (320, 64), "160x32 at scale two");
    let [r, g, b, a] = image.get_pixel(24, 32).0;
    assert_eq!(
        format!("#{r:02X}{g:02X}{b:02X}"),
        "#2B2B2B",
        "the pill is the elevated surface"
    );
    assert_eq!(a, 255);
}
