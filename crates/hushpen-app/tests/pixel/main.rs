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
//! - `HUSHPEN_PIXEL_OUT`: where actual and diff images and the scan reports go.
//!   Default: the temp folder (the container sets `TMPDIR` to the data drive).
//! - `HUSHPEN_PIXEL_UPDATE=1`: write the captures into the baselines folder
//!   instead of comparing. The repo mounts read-only in the container, so an
//!   update run points `HUSHPEN_PIXEL_BASELINES` at a writable folder first.
#![cfg(feature = "pixel-tests")]

mod compare;
mod scan;
mod scenes;

use compare::{Outcome, compare, diff_image};
use gpui_kit::{AppContext as _, HeadlessAppContext, px, size};
use gpui_wgpu::CosmicTextSystem;
use hushpen_app::assets::{AppAssets, register_fonts};
use hushpen_app::flow_bar::view::{Picker, Preview, Props, size_of};
use hushpen_app::mic::{MicBackend, MicSession};
use hushpen_app::settings::Section;
use hushpen_app::theme::{self, Appearance, chrome, space};
use hushpen_app::views::View;
use hushpen_audio::{CaptureError, EventSink, InputDevice};
use hushpen_core::flow_bar::BarState;
use hushpen_core::onboarding::Step;
use hushpen_core::permission::{Access, Preflight};
use image::RgbaImage;
use scenes::{Scene, Shot};
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

/// The data folder of a capture: one fixed path per tag, so the folder text in the Settings and
/// About rows reads the same in every run. A random tempdir would put its changing suffix into
/// the baselines.
fn pixel_data_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("hushpen-pixel").join(tag);
    let _ = std::fs::remove_dir_all(&dir);
    dir
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

/// One default microphone, so the capture does not depend on the container's audio devices.
pub struct OneMicrophone;

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
pub struct Revoked;

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

/// The sidebar row a scene highlights.
fn selected_view(scene: Scene) -> Option<View> {
    match scene {
        Scene::Shell | Scene::History | Scene::HistoryDetail | Scene::HistorySelect => {
            Some(View::History)
        }
        Scene::HistoryClear => Some(View::History),
        Scene::Home => Some(View::Home),
        Scene::Dictionary => Some(View::Dictionary),
        Scene::Import => Some(View::Import),
        Scene::Models => Some(View::Models),
        Scene::About => Some(View::About),
        Scene::Settings(_) | Scene::EndpointFailure | Scene::ShortcutRecorder => {
            Some(View::Settings)
        }
        Scene::Onboarding(_) | Scene::PermissionsChanged => None,
    }
}

/// Runs the three scans over a capture: color only inside the controls and words that may
/// have it, only the lines a group draws, and rows of one line plus at most one secondary line.
/// The reports go to the output folder. Returns what broke a rule.
fn scan_shot(scene: Scene, name: &str, shot: &Shot, failures: &mut Vec<String>) {
    let (colors, color_problems) = scan::colored_regions(&shot.image, &shot.nodes);
    let (lines, line_problems) = scan::line_report(&shot.image, &shot.nodes);
    let (rows, row_problems) = if scene.has_rows() {
        scan::row_report(&shot.nodes)
    } else {
        (String::new(), Vec::new())
    };
    let out = out_dir();
    std::fs::create_dir_all(&out).expect("create the output folder");
    std::fs::write(
        out.join(format!("{name}-scan.txt")),
        format!("{colors}{lines}{rows}"),
    )
    .expect("write the scan report");
    for problem in color_problems
        .into_iter()
        .chain(line_problems)
        .chain(row_problems)
    {
        failures.push(format!("{name}: {problem}"));
    }
}

fn check_scenes(scenes: &[Scene], appearance: Appearance) {
    let mut failures = Vec::new();
    for scene in scenes {
        let shot = scenes::capture(*scene, appearance);
        let name = format!("{}-{}", scene.name(), appearance.key());
        assert_eq!(shot.image.dimensions(), (1880, 1200), "{name}");
        check_image(&name, &shot.image, &mut failures);
        scan_shot(*scene, &name, &shot, &mut failures);
        if let Some(view) = selected_view(*scene) {
            assert_shell_tokens(&shot.image, appearance, Some(view), &name);
        }
    }
    assert!(
        failures.is_empty(),
        "pixel baselines failed:\n{}",
        failures.join("\n")
    );
}

#[test]
fn the_shell_home_and_onboarding_match_their_approved_dark_baselines() {
    check_scenes(&Scene::shell_and_onboarding(), Appearance::Dark);
}

#[test]
fn the_shell_home_and_onboarding_match_their_approved_light_baselines() {
    check_scenes(&Scene::shell_and_onboarding(), Appearance::Light);
}

#[test]
fn every_view_and_dialog_matches_its_approved_dark_baseline() {
    check_scenes(&Scene::views(), Appearance::Dark);
}

#[test]
fn every_view_and_dialog_matches_its_approved_light_baseline() {
    check_scenes(&Scene::views(), Appearance::Light);
}

/// Every scene has one Light and one Dark baseline, at 940x600 at scale two, that differ and
/// follow the palette; no baseline of the old window size or of a scene that is gone is left.
#[test]
fn the_light_and_dark_baselines_differ_and_follow_the_palette_for_every_scene() {
    if updating() {
        return;
    }
    let load = |name: String| {
        image::open(baselines_dir().join(format!("{name}.png")))
            .unwrap_or_else(|error| panic!("{name}: {error}"))
            .to_rgba8()
    };
    let mut known = Vec::new();
    for scene in Scene::all() {
        let name = scene.name();
        let dark = load(format!("{name}-dark"));
        let light = load(format!("{name}-light"));
        assert_eq!(dark.dimensions(), (1880, 1200), "{name}: dark size");
        assert_eq!(light.dimensions(), (1880, 1200), "{name}: light size");
        assert_ne!(dark, light, "{name}");
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
        known.push(format!("{name}-dark.png"));
        known.push(format!("{name}-light.png"));
    }
    let flow_bars: Vec<String> = flow_bar_states()
        .into_iter()
        .map(|(name, _)| format!("{name}.png"))
        .collect();
    for entry in std::fs::read_dir(baselines_dir()).expect("read the baselines folder") {
        let file = entry.expect("a folder entry").file_name();
        let file = file.to_string_lossy();
        assert!(
            known.iter().any(|name| *name == file) || flow_bars.iter().any(|name| *name == file),
            "{file} is a baseline no scene or flow bar state takes"
        );
    }
}

#[test]
fn onboarding_takes_the_whole_window_with_no_sidebar() {
    for (appearance, content) in [
        (Appearance::Dark, "#1E1E1E"),
        (Appearance::Light, "#F5F5F5"),
    ] {
        let shot = scenes::capture(Scene::Onboarding(Step::Permissions), appearance);
        assert_eq!(hex_at(&shot.image, 100, 450), content, "no sidebar color");
        assert_eq!(hex_at(&shot.image, 26, 26), "#FF5F57", "close light");
        assert_eq!(hex_at(&shot.image, 46, 26), "#FEBC2E", "minimize light");
    }
}

/// The seven Settings sections each have a baseline of their own.
#[test]
fn every_settings_section_has_a_scene() {
    let names: Vec<String> = Scene::views().into_iter().map(Scene::name).collect();
    for section in Section::ALL {
        assert!(
            names.contains(&format!("settings-{}", section.key())),
            "{}",
            section.key()
        );
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
