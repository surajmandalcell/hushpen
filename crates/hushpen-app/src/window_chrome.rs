//! How the main window is created: its fixed size and, per platform, who draws the title bar.
//!
//! macOS keeps the system's traffic lights on a transparent title bar over a transparent window.
//! The native sidebar material sits behind the sidebar column (see `add_sidebar_material`), so the
//! sidebar shows a blur of what is behind the window. GPUI's own blurred background shows no blur
//! on current macOS. Linux has no system chrome to keep: the window is frameless and the app
//! draws close and minimize itself (see `window_controls`). With a compositor the Linux window is
//! transparent too, with rounded corners, a translucent sidebar, and the KDE blur property (see
//! `apply_look`). Without one it stays opaque and square, because a transparent
//! window on a bare X server renders black.

use crate::native::native_handle;
use crate::theme::{self, chrome, size as sizes, space};
use gpui_kit::base::Root;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity, Global, Hsla, PlatformDisplay,
    Point, Rgba, Styled as _, TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds,
    WindowOptions, point, px, size, transparent_black,
};
use hushpen_platform::window::{Frame, SizeLimits};
use std::rc::Rc;
use std::time::Duration;

/// How often the app reads whether a compositor runs. One selection-owner query, so cheap.
const COMPOSITOR_POLL: Duration = Duration::from_secs(1);

/// The tallest the window grows, on a screen with room for it.
pub const MAX_HEIGHT: f32 = 900.0;

/// The setting that holds the height the window had at the last resize.
pub const HEIGHT_KEY: &str = "ui.windowHeight";

/// How long a height must hold still before it is saved, so a drag writes the file once.
const SAVE_DELAY: Duration = Duration::from_millis(500);

/// The tallest the window may be on a screen whose usable height is `screen`. A screen shorter
/// than the least height still gets the least height.
pub fn max_height(screen: f32) -> f32 {
    MAX_HEIGHT.min(screen).max(space::WINDOW_HEIGHT)
}

/// The height the window opens with: the saved one, held between 600 and what the screen allows.
/// Nothing saved, or a value that is not a number, gives the least height.
pub fn clamp_height(saved: Option<f32>, screen: f32) -> f32 {
    match saved {
        Some(height) if height.is_finite() => {
            height.clamp(space::WINDOW_HEIGHT, max_height(screen))
        }
        _ => space::WINDOW_HEIGHT,
    }
}

/// What to write for the current height, or `None` when it is the height already saved.
fn height_to_save(current: f32, saved: f32, screen: f32) -> Option<i64> {
    let height = clamp_height(Some(current), screen).round();
    (height != saved.round()).then_some(height as i64)
}

/// The width and height limits the platform gives the native window.
pub fn size_limits(screen: f32) -> SizeLimits {
    SizeLimits {
        width: space::WINDOW_WIDTH as u16,
        min_height: space::WINDOW_HEIGHT as u16,
        max_height: max_height(screen) as u16,
    }
}

/// The usable height of the screen the app opens on. With no screen known, the window is not
/// held below its own limit.
pub fn screen_height(cx: &App) -> f32 {
    cx.primary_display().map_or(MAX_HEIGHT, visible_height)
}

fn visible_height(display: Rc<dyn PlatformDisplay>) -> f32 {
    f32::from(display.visible_bounds().size.height)
}

/// The options for the main window, at `saved` height when there is one. The caller adds the
/// app id.
pub fn main_window_options(saved: Option<f32>, cx: &App) -> WindowOptions {
    let height = clamp_height(saved, screen_height(cx));
    let window_size = size(px(space::WINDOW_WIDTH), px(height));
    options(
        Bounds::centered(None, window_size, cx),
        cfg!(target_os = "macos"),
        chrome::sidebar(cx).see_through(),
    )
}

/// Writes the window height after it has held still, and holds the window inside its limits on
/// a system that does not enforce them.
struct HeightMemory {
    saved: f32,
    /// A resize that comes before the delay ends cancels the save of the one before it.
    generation: u64,
    save: Rc<dyn Fn(i64)>,
}

/// Keeps the memory alive: the window bounds observer ends when its entity is dropped.
struct Kept(
    #[expect(dead_code, reason = "held only to keep the entity alive")] Entity<HeightMemory>,
);

impl Global for Kept {}

impl HeightMemory {
    fn changed(&mut self, height: f32, screen: f32, cx: &mut Context<Self>) {
        self.generation += 1;
        let Some(next) = height_to_save(height, self.saved, screen) else {
            return;
        };
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SAVE_DELAY).await;
            let _ = this.update(cx, |memory, _| {
                if memory.generation == generation {
                    memory.saved = next as f32;
                    (memory.save)(next);
                }
            });
        })
        .detach();
    }
}

/// Saves the main window height with `save` once a resize settles. `saved` is the height the
/// window opened with.
pub fn remember_height(saved: f32, save: Rc<dyn Fn(i64)>, window: &mut Window, cx: &mut App) {
    let memory = cx.new(|_| HeightMemory {
        saved,
        generation: 0,
        save,
    });
    memory.update(cx, |_, cx| {
        cx.observe_window_bounds(window, |memory, window, cx| {
            let screen = window.display(cx).map_or(MAX_HEIGHT, visible_height);
            let height = f32::from(window.viewport_size().height);
            let tallest = max_height(screen);
            if height > tallest + 0.5 {
                window.resize(size(px(space::WINDOW_WIDTH), px(tallest)));
                return;
            }
            // The blur follows the sidebar, which is as tall as the window.
            #[cfg(target_os = "linux")]
            apply_look(window, cx);
            memory.changed(height, screen, cx);
        })
        .detach();
    });
    cx.set_global(Kept(memory));
}

/// How tall the resize strip is, in points.
#[cfg(not(target_os = "macos"))]
const RESIZE_GRIP: f32 = 4.0;

/// The strip along the bottom edge that starts a resize. Linux draws no frame, so the window
/// manager has no edge to grab. macOS resizes by the frame AppKit draws.
#[cfg(not(target_os = "macos"))]
pub fn resize_grip() -> impl gpui_kit::IntoElement {
    use gpui_kit::{CursorStyle, InteractiveElement as _, MouseButton, ResizeEdge, div};
    div()
        .absolute()
        .left_0()
        .right_0()
        .bottom_0()
        .h(px(RESIZE_GRIP))
        .cursor(CursorStyle::ResizeUpDown)
        .on_mouse_down(MouseButton::Left, |_, window, _| {
            window.start_window_resize(ResizeEdge::Bottom);
        })
}

fn options(
    bounds: Bounds<gpui_kit::Pixels>,
    native_chrome: bool,
    see_through: bool,
) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: native_chrome.then(|| TitlebarOptions {
            title: None,
            appears_transparent: true,
            // The real button frame is read once the window is open (see `app.rs`).
            traffic_light_position: Some(traffic_light_position(sizes::TRAFFIC_LIGHT)),
        }),
        window_background: background(see_through),
        // The native limits (see `size_limits`) keep the width and cap the height.
        is_resizable: true,
        is_minimizable: true,
        window_min_size: Some(size(px(space::WINDOW_WIDTH), px(space::WINDOW_HEIGHT))),
        ..Default::default()
    }
}

/// Where AppKit draws the traffic lights, for a standard button frame `button_height` high.
/// GPUI puts the origin of the button frame here, and the frame is taller than the circle it
/// centers, so the top follows from the frame: its center lands on the chrome center line.
pub fn traffic_light_position(button_height: f32) -> Point<gpui_kit::Pixels> {
    point(
        px(sizes::TRAFFIC_LIGHT_INSET),
        px(theme::chrome_top(button_height)),
    )
}

fn background(see_through: bool) -> WindowBackgroundAppearance {
    if see_through {
        WindowBackgroundAppearance::Transparent
    } else {
        WindowBackgroundAppearance::Opaque
    }
}

/// What `hookctl state` reports for the window background.
pub fn background_name(see_through: bool) -> &'static str {
    match background(see_through) {
        WindowBackgroundAppearance::Transparent => "transparent",
        WindowBackgroundAppearance::Blurred => "blurred",
        _ => "opaque",
    }
}

/// What `hookctl state` reports for the sidebar fill: a color (with its alpha when the fill is
/// translucent), or `none` when the sidebar shows the native sidebar material.
pub fn sidebar_fill_name(look: chrome::Sidebar, dark: bool) -> String {
    match look.fill(dark) {
        Some(fill) => {
            let Rgba { r, g, b, a } = fill;
            let byte = |channel: f32| (channel * 255.0).round() as u8;
            if a >= 1.0 {
                format!("#{:02X}{:02X}{:02X}", byte(r), byte(g), byte(b))
            } else {
                format!(
                    "#{:02X}{:02X}{:02X}{:02X}",
                    byte(r),
                    byte(g),
                    byte(b),
                    byte(a)
                )
            }
        }
        None => "none".to_owned(),
    }
}

/// What the Kit root paints under the whole window: `fill`, or the theme background with
/// `None` (its default). A see-through window paints nothing there, so what is behind the
/// window shows where no view paints. A window without a Kit root is left alone.
pub fn set_root_fill(fill: Option<Hsla>, window: &mut Window, cx: &mut App) {
    let Some(root) = window.root::<Root>().flatten() else {
        return;
    };
    root.update(cx, |root, cx| {
        root.style().background = fill.map(Into::into);
        cx.notify();
    });
}

/// The see-through parts that live outside the views: the window background and the root fill.
/// Called when the window opens and when a compositor comes or goes.
pub fn apply_see_through(see_through: bool, window: &mut Window, cx: &mut App) {
    window.set_background_appearance(background(see_through));
    set_root_fill(see_through.then(transparent_black), window, cx);
}

/// The whole see-through look for the current compositor reading: the window background, the
/// root fill, and on Linux the blur behind the sidebar. Called when the window opens and when
/// a compositor comes or goes.
pub fn apply_look(window: &mut Window, cx: &mut App) {
    let look = chrome::sidebar(cx);
    apply_see_through(look.see_through(), window, cx);
    let handle = native_handle(window);
    let blur = if look == chrome::Sidebar::Blurred {
        let scale = window.scale_factor();
        let device = |length: f32| (length * scale).round() as u32;
        let sidebar = Frame {
            x: 0,
            y: 0,
            width: device(space::SIDEBAR_WIDTH),
            height: device(f32::from(window.viewport_size().height)),
        };
        hushpen_platform::window::blur_behind(handle, sidebar)
    } else {
        hushpen_platform::window::clear_blur_behind(handle)
    };
    if let Err(error) = blur {
        log::warn!("could not set the sidebar blur: {error}");
    }
}

/// Follows a compositor that starts or stops while the app runs, with one read a second, and
/// repaints the main window in its new look. macOS always composites, so nothing runs there.
pub fn follow_compositor(main: AnyWindowHandle, cx: &mut App) {
    if cfg!(target_os = "macos") {
        return;
    }
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(COMPOSITOR_POLL).await;
            let running = hushpen_platform::window::compositor_running();
            cx.update(|cx| {
                if chrome::set_compositor(running, cx) {
                    let _ = main.update(cx, |_, window, cx| apply_look(window, cx));
                }
            });
        }
    })
    .detach();
}

/// The fill the Kit root paints under the whole window: its own fill, else the theme
/// background. `None` when it paints nothing.
pub fn root_fill(window: &Window, cx: &mut App) -> Option<Hsla> {
    let own = window
        .root::<Root>()
        .flatten()
        .and_then(|root| root.update(cx, |root, _| root.style().background.clone()));
    let color = match own {
        Some(fill) => fill.color().and_then(|background| background.as_solid()),
        None => Some(cx.theme().background),
    };
    color.filter(|color| !color.is_transparent())
}

/// What `hookctl state` reports for the root fill: a color, or `none` when the root paints
/// nothing and the window background shows.
pub fn root_fill_name(window: &Window, cx: &mut App) -> String {
    match root_fill(window, cx) {
        Some(color) => {
            let Rgba { r, g, b, .. } = color.into();
            let byte = |channel: f32| (channel * 255.0).round() as u8;
            format!("#{:02X}{:02X}{:02X}", byte(r), byte(g), byte(b))
        }
        None => "none".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::test::TestWindowExt as _;

    fn bounds() -> Bounds<gpui_kit::Pixels> {
        Bounds {
            origin: Point::default(),
            size: size(px(space::WINDOW_WIDTH), px(space::WINDOW_HEIGHT)),
        }
    }

    #[test]
    fn the_window_is_940_wide_and_resizes_in_height_from_600() {
        let options = options(bounds(), true, true);
        assert_eq!((space::WINDOW_WIDTH, space::WINDOW_HEIGHT), (940.0, 600.0));
        assert!(options.is_resizable);
        assert!(options.is_minimizable);
        assert_eq!(options.window_min_size, Some(bounds().size));
        assert_eq!(
            options.window_bounds,
            Some(WindowBounds::Windowed(bounds()))
        );
    }

    #[test]
    fn the_tallest_window_is_900_or_the_screen_when_it_is_shorter() {
        assert_eq!(max_height(1440.0), 900.0);
        assert_eq!(max_height(900.0), 900.0);
        assert_eq!(max_height(780.0), 780.0);
        assert_eq!(max_height(500.0), 600.0, "never below the least height");
    }

    #[test]
    fn a_saved_height_is_held_between_600_and_the_screen_limit() {
        assert_eq!(clamp_height(None, 1000.0), 600.0);
        assert_eq!(clamp_height(Some(720.0), 1000.0), 720.0);
        assert_eq!(clamp_height(Some(300.0), 1000.0), 600.0);
        assert_eq!(clamp_height(Some(2000.0), 1000.0), 900.0);
        assert_eq!(clamp_height(Some(850.0), 700.0), 700.0, "a smaller screen");
        assert_eq!(clamp_height(Some(f32::NAN), 1000.0), 600.0);
        assert_eq!(clamp_height(Some(f32::INFINITY), 1000.0), 600.0);
    }

    #[test]
    fn a_height_is_saved_only_when_it_differs_from_the_saved_one() {
        assert_eq!(height_to_save(700.0, 600.0, 1000.0), Some(700));
        assert_eq!(height_to_save(700.4, 700.0, 1000.0), None);
        assert_eq!(height_to_save(600.0, 600.0, 1000.0), None);
        assert_eq!(height_to_save(1200.0, 900.0, 1000.0), None, "held at 900");
        assert_eq!(height_to_save(1200.0, 600.0, 1000.0), Some(900));
    }

    #[test]
    fn the_native_limits_keep_the_width_and_cap_the_height() {
        assert_eq!(
            size_limits(1440.0),
            SizeLimits {
                width: 940,
                min_height: 600,
                max_height: 900
            }
        );
        assert_eq!(size_limits(750.0).max_height, 750);
    }

    #[gpui_kit::test]
    fn a_resize_saves_its_final_height_once_after_it_settles(cx: &mut gpui_kit::TestAppContext) {
        use std::cell::RefCell;
        let handle = open_shell(cx);
        let saves = Rc::new(RefCell::new(Vec::new()));
        let log = Rc::clone(&saves);
        cx.update_window(handle, |_, window, cx| {
            remember_height(
                600.0,
                Rc::new(move |height| log.borrow_mut().push(height)),
                window,
                cx,
            );
        })
        .unwrap();
        for height in [650.0, 700.0, 720.0] {
            cx.simulate_window_resize(handle, size(px(space::WINDOW_WIDTH), px(height)));
            cx.executor().advance_clock(SAVE_DELAY / 4);
            cx.run_until_parked();
        }
        assert!(saves.borrow().is_empty(), "still moving");
        cx.executor().advance_clock(SAVE_DELAY);
        cx.run_until_parked();
        assert_eq!(*saves.borrow(), [720]);
    }

    #[gpui_kit::test]
    fn a_window_taller_than_the_limit_is_brought_back_to_it(cx: &mut gpui_kit::TestAppContext) {
        let handle = open_shell(cx);
        cx.update_window(handle, |_, window, cx| {
            remember_height(600.0, Rc::new(|_| {}), window, cx);
        })
        .unwrap();
        cx.simulate_window_resize(handle, size(px(space::WINDOW_WIDTH), px(1200.0)));
        // The test platform takes the resize without reporting it back, so read its bounds.
        let height = cx
            .update_window(handle, |_, window, _| {
                f32::from(window.bounds().size.height)
            })
            .unwrap();
        assert_eq!(height, MAX_HEIGHT);
    }

    #[test]
    fn macos_asks_for_a_transparent_title_bar_with_the_lights_at_20_20() {
        let options = options(bounds(), true, true);
        let titlebar = options.titlebar.expect("a native title bar on macOS");
        assert!(titlebar.appears_transparent);
        assert_eq!(
            titlebar.traffic_light_position,
            Some(point(px(20.0), px(20.0)))
        );
    }

    /// AppKit's close button frame is taller than its 12 px circle (14 to 16 px before macOS
    /// 26, more after), and GPUI places the frame, so the lights move up by half the extra.
    #[test]
    fn the_native_lights_center_on_the_chrome_line_for_any_button_frame() {
        for height in [12.0, 14.0, 16.0, 22.0] {
            let origin = traffic_light_position(height);
            let center = f32::from(origin.y) + height / 2.0;
            assert_eq!(f32::from(origin.x), sizes::TRAFFIC_LIGHT_INSET, "{height}");
            assert!(
                (center - sizes::CHROME_CENTER).abs() <= 0.5,
                "a {height} px frame centers at {center}"
            );
        }
        assert_eq!(f32::from(traffic_light_position(14.0).y), 19.0);
    }

    #[test]
    fn macos_makes_the_window_transparent_so_the_sidebar_material_shows() {
        assert_eq!(
            options(bounds(), true, true).window_background,
            WindowBackgroundAppearance::Transparent
        );
    }

    #[test]
    fn linux_is_frameless_and_opaque_without_a_compositor() {
        let options = options(bounds(), false, false);
        assert!(options.titlebar.is_none());
        assert_eq!(
            options.window_background,
            WindowBackgroundAppearance::Opaque
        );
    }

    /// With a compositor the Linux window gets the ARGB visual and what is behind it shows
    /// through; without one a transparent window would render black, so it stays opaque.
    #[test]
    fn linux_with_a_compositor_is_frameless_and_transparent() {
        let options = options(bounds(), false, true);
        assert!(options.titlebar.is_none());
        assert_eq!(
            options.window_background,
            WindowBackgroundAppearance::Transparent
        );
    }

    #[test]
    fn the_hook_names_follow_the_sidebar_look() {
        assert_eq!(background_name(true), "transparent");
        assert_eq!(background_name(false), "opaque");
        assert_eq!(sidebar_fill_name(chrome::Sidebar::Native, true), "none");
        assert_eq!(sidebar_fill_name(chrome::Sidebar::Native, false), "none");
        assert_eq!(sidebar_fill_name(chrome::Sidebar::Solid, true), "#2B2B2D");
        assert_eq!(sidebar_fill_name(chrome::Sidebar::Solid, false), "#E8E8EA");
        assert_eq!(
            sidebar_fill_name(chrome::Sidebar::Blurred, true),
            "#2B2B2DCC"
        );
        assert_eq!(
            sidebar_fill_name(chrome::Sidebar::Blurred, false),
            "#E8E8EACC"
        );
    }

    fn open_shell(cx: &mut gpui_kit::TestAppContext) -> gpui_kit::AnyWindowHandle {
        cx.update(gpui_kit::init);
        cx.update(crate::theme::install);
        let (handle, _) = cx
            .update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds())),
                        ..Default::default()
                    },
                    cx,
                    |window, cx| cx.new(|cx| crate::shell::Shell::new(window, cx)),
                )
            })
            .expect("open test window");
        handle
    }

    fn root_fill_of(
        cx: &mut gpui_kit::TestAppContext,
        handle: gpui_kit::AnyWindowHandle,
        translucent: bool,
    ) -> Option<Hsla> {
        cx.update_window(handle, |_, window, cx| {
            if let Some(root) = window.root::<Root>().flatten() {
                root.update(cx, |root, _| root.style().background = None);
            }
            set_root_fill(translucent.then(transparent_black), window, cx);
            window.render_frame(cx);
            root_fill(window, cx)
        })
        .unwrap()
    }

    #[gpui_kit::test]
    fn a_translucent_window_root_paints_no_fill(cx: &mut gpui_kit::TestAppContext) {
        let handle = open_shell(cx);
        assert_eq!(root_fill_of(cx, handle, true), None);
        let name = cx
            .update_window(handle, |_, window, cx| root_fill_name(window, cx))
            .unwrap();
        assert_eq!(name, "none");
    }

    #[gpui_kit::test]
    fn a_solid_window_root_keeps_the_content_color(cx: &mut gpui_kit::TestAppContext) {
        let handle = open_shell(cx);
        let content = cx.update(|cx| crate::theme::palette(cx).content);
        assert_eq!(root_fill_of(cx, handle, false), Some(content.into()));
    }

    #[gpui_kit::test]
    fn the_main_window_root_matches_the_platform(cx: &mut gpui_kit::TestAppContext) {
        let handle = open_shell(cx);
        cx.run_until_parked();
        let fill = cx
            .update_window(handle, |_, window, cx| root_fill(window, cx))
            .unwrap();
        let content = cx.update(|cx| crate::theme::palette(cx).content);
        let see_through = cx.update(|cx| chrome::sidebar(cx).see_through());
        if see_through {
            assert_eq!(fill, None);
        } else {
            assert_eq!(fill, Some(content.into()));
        }
    }
}
