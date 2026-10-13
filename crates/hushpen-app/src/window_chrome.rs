//! How the main window is created: its fixed size and, per platform, who draws the title bar.
//!
//! macOS keeps the system's traffic lights on a transparent title bar over a blurred window, so
//! the sidebar can show what is behind it. Linux has no system chrome to keep: the window is
//! frameless and the app draws close and minimize itself (see `window_controls`).

use crate::theme::{chrome, size as sizes, space};
use gpui_kit::{
    App, Bounds, Point, TitlebarOptions, WindowBackgroundAppearance, WindowBounds, WindowOptions,
    point, px, size,
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
        WindowBackgroundAppearance::Blurred
    } else {
        WindowBackgroundAppearance::Opaque
    }
}

/// What `hookctl state` reports for the window background.
pub fn background_name() -> &'static str {
    match background(chrome::TRANSLUCENT_SIDEBAR) {
        WindowBackgroundAppearance::Blurred => "blurred",
        _ => "opaque",
    }
}

/// What `hookctl state` reports for the sidebar fill: a color, or `none` when the sidebar
/// shows the blurred window.
pub fn sidebar_fill_name() -> String {
    match chrome::sidebar_fill(chrome::TRANSLUCENT_SIDEBAR, true) {
        Some(fill) => format!("#{fill:06X}"),
        None => "none".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn macos_blurs_the_window_so_the_sidebar_can_show_it() {
        assert_eq!(
            options(bounds(), true).window_background,
            WindowBackgroundAppearance::Blurred
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
                (background_name(), sidebar_fill_name().as_str()),
                ("blurred", "none")
            );
        } else {
            assert_eq!(
                (background_name(), sidebar_fill_name().as_str()),
                ("opaque", "#2B2B2D")
            );
        }
    }
}
