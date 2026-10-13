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
use hushpen_app::flow_bar::view::{Picker, Preview, Props, size_of};
use hushpen_app::mic::{CpalBackend, Mic};
use hushpen_app::models::{self, Models};
use hushpen_app::net::endpoint::Transport;
use hushpen_app::net::endpoint::UreqTransport;
use hushpen_app::onboarding::{Onboarding, Parts as OnboardingParts, SystemGuide};
use hushpen_app::settings::endpoint::EndpointForm;
use hushpen_app::settings::{Parts as SettingsParts, Section, Settings};
use hushpen_app::shell::Shell;
use hushpen_app::shortcuts::Shortcuts;
use hushpen_app::storage;
use hushpen_app::theme::{self, space};
use hushpen_app::views::View;
use hushpen_core::flow_bar::BarState;
use hushpen_core::onboarding::{Session, Step};
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
    // The fallback family is only for glyphs Inter lacks; the app embeds Inter.
    let text = Arc::new(CosmicTextSystem::new("DejaVu Sans"));
    let mut cx = HeadlessAppContext::with_platform(text, Arc::new(AppAssets), || {
        gpui_kit::platform::current_headless_renderer()
    });
    cx.update(|cx| {
        gpui_kit::init(cx);
        register_fonts(cx);
        theme::install(cx);
    });
    cx
}

/// Hex color at a point given in logical pixels.
fn hex_at(image: &RgbaImage, x: u32, y: u32) -> String {
    let scale = image.width() / space::WINDOW_WIDTH as u32;
    let [r, g, b, _] = image.get_pixel(x * scale, y * scale).0;
    format!("#{r:02X}{g:02X}{b:02X}")
}

/// Centre of a sidebar item's fill, left of its icon.
fn nav_fill_point(index: u32) -> (u32, u32) {
    (190, 48 + 40 * index + 18)
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
    View::ALL
        .into_iter()
        .map(|view| {
            if view != View::Home {
                let id: &'static str =
                    Box::leak(format!("sidebar.{}", view.key()).into_boxed_str());
                cx.update_window(handle, |_, window, cx| window.click(id, cx))
                    .expect("click the sidebar item");
                cx.run_until_parked();
            }
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
        let image = &capture.image;
        let name = capture.view.key();
        // Between the toolbar and the first panel: app background in every view. (500, 300)
        // reads panel surface on views like About that fill the column with one surface.
        assert_eq!(hex_at(image, 500, 47), "#2F2F2F", "{name}: content");
        assert_eq!(hex_at(image, 100, 450), "#2B2B2D", "{name}: sidebar");
        assert_eq!(hex_at(image, 26, 26), "#FF5F57", "{name}: close light");
        assert_eq!(hex_at(image, 46, 26), "#FEBC2E", "{name}: minimize light");
        for (index, view) in View::ALL.into_iter().enumerate() {
            let (x, y) = nav_fill_point(index as u32);
            let want = if view == capture.view {
                "#686868"
            } else {
                "#2B2B2D"
            };
            assert_eq!(
                hex_at(image, x, y),
                want,
                "{name}: sidebar item {}",
                view.key()
            );
        }
    }
}

/// The main window with onboarding open at `step`, on a data folder with no models.
fn capture_onboarding(step: &str) -> RgbaImage {
    let tmp = tempfile::tempdir().expect("a scratch folder");
    let storage = Rc::new(
        storage::open(DataDir::open(tmp.path().join("data")).expect("open the data folder"))
            .expect("open the storage"),
    );
    storage
        .settings
        .set_internal("onboarding.step", json!(step))
        .expect("store the step");
    let mut cx = headless();
    let handle = cx
        .open_window(
            size(px(space::WINDOW_WIDTH), px(space::WINDOW_HEIGHT)),
            |window, cx| {
                let shell = cx.new(|cx| Shell::new(window, cx));
                let mic = cx.new(|cx| Mic::new(Rc::clone(&storage), Arc::new(CpalBackend), cx));
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
                    controller.attach_keys(KeysStatus::Available, Rc::new(|_| {}));
                });
                let onboarding = cx.new(|cx| {
                    Onboarding::new(
                        OnboardingParts {
                            storage: Rc::clone(&storage),
                            mic,
                            models,
                            controller,
                            guide: Rc::new(SystemGuide),
                            platform: Platform::Linux,
                            session: Session::X11,
                        },
                        cx,
                    )
                });
                shell.update(cx, |shell, cx| shell.attach_onboarding(onboarding, cx));
                cx.new(|cx| gpui_kit::base::Root::new(shell, window, cx))
            },
        )
        .expect("open the headless window");
    cx.run_until_parked();
    cx.capture_screenshot(handle.into())
        .expect("capture the window")
}

#[test]
fn every_onboarding_step_matches_its_approved_baseline() {
    let mut failures = Vec::new();
    for step in Step::ALL {
        let image = capture_onboarding(step.key());
        assert_eq!(image.dimensions(), (1880, 1200), "{}", step.key());
        check_image(&format!("onboarding-{}", step.key()), &image, &mut failures);
    }
    assert!(
        failures.is_empty(),
        "pixel baselines failed:\n{}",
        failures.join("\n")
    );
}

#[test]
fn onboarding_takes_the_whole_window_with_no_sidebar() {
    let image = capture_onboarding("permissions");
    assert_eq!(hex_at(&image, 100, 450), "#2F2F2F", "no sidebar color");
    assert_eq!(hex_at(&image, 26, 26), "#FF5F57", "close light");
    assert_eq!(hex_at(&image, 46, 26), "#FEBC2E", "minimize light");
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
    let mut cx = headless();
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
