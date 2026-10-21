//! How the main window is created: its fixed size and, per platform, who draws the title bar.
//!
//! macOS keeps the system's traffic lights on a transparent title bar over a transparent window.
//! The native sidebar material sits behind the sidebar column (see `add_sidebar_material`), so the
//! sidebar shows a blur of what is behind the window. GPUI's own blurred background shows no blur
//! on current macOS. Linux has no system chrome to keep: the window is frameless and the app
//! draws close and minimize itself (see `window_controls`).

use crate::theme::{chrome, size as sizes, space};
use gpui_kit::base::Root;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    App, Bounds, Hsla, Point, Rgba, Styled as _, TitlebarOptions, Window,
    WindowBackgroundAppearance, WindowBounds, WindowOptions, point, px, size, transparent_black,
};

/// The options for the main window. The caller adds the app id.
pub fn main_window_options(cx: &App) -> WindowOptions {
    let window_size = size(px(space::WINDOW_WIDTH), px(space::WINDOW_HEIGHT));
    options(
        Bounds::centered(None, window_size, cx),
        cfg!(target_os = "macos"),
    )
}

fn options(bounds: Bounds<gpui_kit::Pixels>, native_chrome: bool) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: native_chrome.then(|| TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: Some(traffic_light_position()),
        }),
        window_background: background(native_chrome),
        is_resizable: false,
        is_minimizable: true,
        window_min_size: Some(bounds.size),
        ..Default::default()
    }
}

/// Where AppKit draws the traffic lights.
pub fn traffic_light_position() -> Point<gpui_kit::Pixels> {
    point(
        px(sizes::TRAFFIC_LIGHT_INSET),
        px(sizes::TRAFFIC_LIGHT_INSET),
    )
}

fn background(native_chrome: bool) -> WindowBackgroundAppearance {
    if native_chrome {
        WindowBackgroundAppearance::Transparent
    } else {
        WindowBackgroundAppearance::Opaque
    }
}

/// What `hookctl state` reports for the window background.
pub fn background_name() -> &'static str {
    match background(chrome::TRANSLUCENT_SIDEBAR) {
        WindowBackgroundAppearance::Transparent => "transparent",
        WindowBackgroundAppearance::Blurred => "blurred",
        _ => "opaque",
    }
}

/// What `hookctl state` reports for the sidebar fill: a color, or `none` when the sidebar
/// shows the native sidebar material.
pub fn sidebar_fill_name(dark: bool) -> String {
    match chrome::sidebar_fill(chrome::TRANSLUCENT_SIDEBAR, dark) {
        Some(fill) => format!("#{fill:06X}"),
        None => "none".to_owned(),
    }
}

/// Lets the native sidebar material show through where no view paints. The Kit root
/// paints the theme background under the whole window by default, and the root's own style wins
/// over that default. A window without a Kit root is left alone.
pub fn clear_root_fill(translucent: bool, window: &mut Window, cx: &mut App) {
    if !translucent {
        return;
    }
    let Some(root) = window.root::<Root>().flatten() else {
        return;
    };
    root.update(cx, |root, cx| {
        root.style().background = Some(transparent_black().into());
        cx.notify();
    });
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
        let options = options(bounds(), true);
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
        let options = options(bounds(), true);
        let titlebar = options.titlebar.expect("a native title bar on macOS");
        assert!(titlebar.appears_transparent);
        assert_eq!(
            titlebar.traffic_light_position,
            Some(point(px(20.0), px(20.0)))
        );
    }

    #[test]
    fn macos_makes_the_window_transparent_so_the_sidebar_material_shows() {
        assert_eq!(
            options(bounds(), true).window_background,
            WindowBackgroundAppearance::Transparent
        );
    }

    #[test]
    fn linux_is_frameless_and_opaque() {
        let options = options(bounds(), false);
        assert!(options.titlebar.is_none());
        assert_eq!(
            options.window_background,
            WindowBackgroundAppearance::Opaque
        );
    }

    #[test]
    fn the_sidebar_is_solid_on_linux_and_see_through_on_macos() {
        assert_eq!(chrome::sidebar_fill(false, true), Some(0x2B2B2D));
        assert_eq!(chrome::sidebar_fill(false, false), Some(0xE8E8EA));
        assert_eq!(chrome::sidebar_fill(true, true), None);
        assert_eq!(chrome::sidebar_fill(true, false), None);
    }

    #[test]
    fn the_hook_names_match_the_platform() {
        if cfg!(target_os = "macos") {
            assert_eq!(
                (background_name(), sidebar_fill_name(true).as_str()),
                ("transparent", "none")
            );
        } else {
            assert_eq!(
                (background_name(), sidebar_fill_name(true).as_str()),
                ("opaque", "#2B2B2D")
            );
            assert_eq!(sidebar_fill_name(false), "#E8E8EA");
        }
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
            clear_root_fill(translucent, window, cx);
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
        if chrome::TRANSLUCENT_SIDEBAR {
            assert_eq!(fill, None);
        } else {
            assert_eq!(fill, Some(content.into()));
        }
    }
}
