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
    AnyWindowHandle, App, Bounds, Hsla, Point, Rgba, Styled as _, TitlebarOptions, Window,
    WindowBackgroundAppearance, WindowBounds, WindowOptions, point, px, size, transparent_black,
};
use hushpen_platform::window::Frame;
use std::time::Duration;

/// How often the app reads whether a compositor runs. One selection-owner query, so cheap.
const COMPOSITOR_POLL: Duration = Duration::from_secs(1);

/// The options for the main window. The caller adds the app id.
pub fn main_window_options(cx: &App) -> WindowOptions {
    let window_size = size(px(space::WINDOW_WIDTH), px(space::WINDOW_HEIGHT));
    options(
        Bounds::centered(None, window_size, cx),
        cfg!(target_os = "macos"),
        chrome::sidebar(cx).see_through(),
    )
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
        is_resizable: false,
        is_minimizable: true,
        window_min_size: Some(bounds.size),
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
            height: device(space::WINDOW_HEIGHT),
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
    use gpui_kit::AppContext as _;
    use gpui_kit::test::TestWindowExt as _;

    fn bounds() -> Bounds<gpui_kit::Pixels> {
        Bounds {
            origin: Point::default(),
            size: size(px(space::WINDOW_WIDTH), px(space::WINDOW_HEIGHT)),
        }
    }

    #[test]
    fn the_window_is_940_by_600_and_cannot_be_resized() {
        let options = options(bounds(), true, true);
        assert_eq!((space::WINDOW_WIDTH, space::WINDOW_HEIGHT), (940.0, 600.0));
        assert!(!options.is_resizable);
        assert!(options.is_minimizable);
        assert_eq!(options.window_min_size, Some(bounds().size));
        assert_eq!(
            options.window_bounds,
            Some(WindowBounds::Windowed(bounds()))
        );
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
