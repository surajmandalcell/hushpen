//! Offscreen pixel tests: the real window at 780x520 rendered by GPUI's wgpu
//! headless renderer (Mesa lavapipe), with no X server and no display.
//!
//! Linux containers only: `cargo test -p hushpen-app --features pixel-tests
//! -- --test-threads=2` (services.yaml `test-pixel`). Captures are at scale 2,
//! so a 780x520 window is 1560x1040 pixels and sample points scale with
//! `width / 780`.
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
use gpui_kit::{AppContext as _, HeadlessAppContext, px, size};
use gpui_wgpu::CosmicTextSystem;
use hushpen_app::assets::{AppAssets, register_fonts};
use hushpen_app::flow_bar::view::{Picker, Preview, Props, size_of};
use hushpen_app::shell::Shell;
use hushpen_app::theme::{self, space};
use hushpen_app::views::View;
use hushpen_core::flow_bar::BarState;
use image::RgbaImage;
use std::path::PathBuf;
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

fn capture_every_view() -> Vec<Capture> {
    let mut cx = headless();
    let handle = cx
        .open_window(
            size(px(space::WINDOW_WIDTH), px(space::WINDOW_HEIGHT)),
            |window, cx| {
                let shell = cx.new(|cx| Shell::new(window, cx));
                cx.new(|cx| gpui_kit::base::Root::new(shell, window, cx))
            },
        )
        .expect("open the headless window");
    let handle = handle.into();
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
fn captures_are_780x520_at_scale_two_and_use_the_design_tokens() {
    let captures = capture_every_view();
    for capture in &captures {
        assert_eq!(
            capture.image.dimensions(),
            (1560, 1040),
            "{}",
            capture.view.key()
        );
    }
    for capture in &captures {
        let image = &capture.image;
        let name = capture.view.key();
        assert_eq!(hex_at(image, 500, 300), "#2F2F2F", "{name}: content");
        assert_eq!(hex_at(image, 100, 450), "#3C3C3C", "{name}: sidebar");
        assert_eq!(hex_at(image, 26, 24), "#FF5F57", "{name}: close light");
        assert_eq!(hex_at(image, 47, 24), "#FEBC2E", "{name}: minimize light");
        for (index, view) in View::ALL.into_iter().enumerate() {
            let (x, y) = nav_fill_point(index as u32);
            let want = if view == capture.view {
                "#686868"
            } else {
                "#3C3C3C"
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

fn flow_bar_states() -> Vec<(&'static str, Props)> {
    let mut listening = Props::new(BarState::Listening);
    listening.levels = (0..32)
        .map(|n| (0.5 + 0.5 * (n as f32 * 0.55).sin()) * (n as f32 / 31.0))
        .collect();
    let mut transcribing = Props::new(BarState::Transcribing);
    transcribing.dot = 1;
    let mut result = Props::new(BarState::Result);
    result.message = "Inserted".into();
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
        ("flowbar-result", result),
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
