//! Every DESIGN.md token, in one place, and the appearance that picks the palette. Views take
//! colors, sizes, and type from here and never write a literal. The tests at the bottom read
//! DESIGN.md and fail when a token drifts.
//!
//! The appearance follows the system: [`install`] reads it, [`observe`] keeps it current, and
//! every view calls [`palette`] while it renders, so a change repaints the open window.

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{
    App, FontWeight, Global, Rgba, Styled, Subscription, Window, WindowAppearance, px, relative,
    rgba,
};
use std::cell::Cell;

macro_rules! tokens {
    ($table:ident: $ty:ty { $($name:ident = $value:expr, $token:literal;)* }) => {
        $(pub const $name: $ty = $value;)*
        /// DESIGN.md token name and value, for the drift test.
        #[cfg(test)]
        pub(crate) const $table: &[(&str, $ty)] = &[$(($token, $name)),*];
    };
}

/// The Dark colors as `0xRRGGBBAA`.
pub mod dark {
    tokens!(TABLE: u32 {
        CONTENT = 0x1E1E1EFF, "dark-content";
        SIDEBAR = 0x2B2B2DFF, "dark-sidebar";
        GROUP = 0xFFFFFF0D, "dark-group";
        SELECTION = 0xFFFFFF1A, "dark-selection";
        CONTROL = 0xFFFFFF1F, "dark-control";
        LABEL = 0xFFFFFFD9, "dark-label";
        LABEL_SECONDARY = 0xFFFFFF8C, "dark-label-secondary";
        LABEL_TERTIARY = 0xFFFFFF40, "dark-label-tertiary";
        SEPARATOR = 0xFFFFFF1A, "dark-separator";
        ACCENT = 0x0A84FFFF, "dark-accent";
        SUCCESS = 0x30D158FF, "dark-success";
        WARNING = 0xFF9F0AFF, "dark-warning";
        ERROR = 0xFF453AFF, "dark-error";
    });
}

/// The Light colors as `0xRRGGBBAA`.
pub mod light {
    tokens!(TABLE: u32 {
        CONTENT = 0xF5F5F5FF, "light-content";
        SIDEBAR = 0xE8E8EAFF, "light-sidebar";
        GROUP = 0x00000009, "light-group";
        SELECTION = 0x00000017, "light-selection";
        CONTROL = 0x00000014, "light-control";
        LABEL = 0x000000D9, "light-label";
        LABEL_SECONDARY = 0x00000080, "light-label-secondary";
        LABEL_TERTIARY = 0x00000042, "light-label-tertiary";
        SEPARATOR = 0x0000001A, "light-separator";
        ACCENT = 0x007AFFFF, "light-accent";
        SUCCESS = 0x34C759FF, "light-success";
        WARNING = 0xFF9500FF, "light-warning";
        ERROR = 0xFF3B30FF, "light-error";
    });
}

/// Colors that do not change with the appearance, and the window chrome.
pub mod chrome {
    tokens!(TABLE: u32 {
        ON_ACCENT = 0xFFFFFFFF, "on-accent";
        WINDOW_CLOSE = 0xFF5F57FF, "window-close";
        WINDOW_CLOSE_GLYPH = 0x6E140FFF, "window-close-glyph";
        WINDOW_MINIMIZE = 0xFEBC2EFF, "window-minimize";
        WINDOW_MINIMIZE_GLYPH = 0x8F5B00FF, "window-minimize-glyph";
    });

    /// The solid Linux sidebar as `0xRRGGBB`. macOS has no sidebar fill: the window background
    /// is blurred and the sidebar shows it.
    pub const SIDEBAR_SOLID_DARK: u32 = super::dark::SIDEBAR >> 8;
    pub const SIDEBAR_SOLID_LIGHT: u32 = super::light::SIDEBAR >> 8;

    /// Whether the window background is blurred and the sidebar shows it: macOS only.
    pub const TRANSLUCENT_SIDEBAR: bool = cfg!(target_os = "macos");

    /// The sidebar fill for an appearance, or `None` where the sidebar is see-through.
    pub fn sidebar_fill(translucent: bool, dark: bool) -> Option<u32> {
        match (translucent, dark) {
            (true, _) => None,
            (false, true) => Some(SIDEBAR_SOLID_DARK),
            (false, false) => Some(SIDEBAR_SOLID_LIGHT),
        }
    }
}

/// Corner radii in logical pixels.
pub mod radius {
    tokens!(TABLE: f32 {
        NONE = 0.0, "none";
        BUTTON = 5.0, "button";
        SELECTION = 6.0, "selection";
        GROUP = 8.0, "group";
        APP_ICON = 16.0, "app-icon";
        FULL = 9999.0, "full";
    });
}

/// Spacing and fixed layout sizes in logical pixels.
pub mod space {
    tokens!(TABLE: f32 {
        HAIRLINE = 0.5, "hairline";
        XXS = 2.0, "xxs";
        XS = 4.0, "xs";
        SM = 8.0, "sm";
        MD = 12.0, "md";
        LG = 16.0, "lg";
        XL = 20.0, "xl";
        XXL = 24.0, "2xl";
        XXXL = 32.0, "3xl";
        SIDEBAR_WIDTH = 232.0, "sidebar-width";
        SIDEBAR_TOP = 48.0, "sidebar-top";
        TOOLBAR_HEIGHT = 52.0, "toolbar-height";
        ONBOARDING_WIDTH = 640.0, "onboarding-width";
        WINDOW_WIDTH = 940.0, "window-width";
        WINDOW_HEIGHT = 600.0, "window-height";
    });
}

/// Sizes taken from the `components` block of DESIGN.md.
pub mod size {
    pub const NAV_ITEM_HEIGHT: f32 = 28.0;
    pub const NAV_ITEM_PADDING: f32 = 8.0;
    pub const NAV_ICON: f32 = 16.0;
    pub const ROW_MIN_HEIGHT: f32 = 36.0;
    pub const ROW_PADDING: f32 = 10.0;
    pub const BUTTON_HEIGHT: f32 = 22.0;
    pub const BUTTON_PADDING: f32 = 12.0;
    pub const TRAFFIC_LIGHT: f32 = 12.0;
    /// Where the window controls sit, native on macOS and drawn on Linux: the top left corner
    /// of the first circle.
    pub const TRAFFIC_LIGHT_INSET: f32 = 20.0;
    pub const FOCUS_RING: f32 = 2.0;
}

/// The font of the views: the system font on macOS, and the embedded Inter on Linux, where no
/// system font can be assumed.
pub const FONT_FAMILY: &str = if cfg!(target_os = "macos") {
    ".SystemUIFont"
} else {
    "Inter"
};

/// One row of the `typography` block.
#[derive(Debug, Clone, Copy)]
pub struct TextToken {
    pub size: f32,
    pub weight: f32,
    pub line_height: f32,
}

pub const TITLE: TextToken = TextToken {
    size: 22.0,
    weight: 700.0,
    line_height: 1.2,
};
pub const TOOLBAR_TITLE: TextToken = TextToken {
    size: 13.0,
    weight: 600.0,
    line_height: 1.0,
};
pub const BODY: TextToken = TextToken {
    size: 13.0,
    weight: 400.0,
    line_height: 1.3,
};
pub const BUTTON: TextToken = TextToken {
    size: 13.0,
    weight: 500.0,
    line_height: 1.0,
};
pub const SECONDARY: TextToken = TextToken {
    size: 11.0,
    weight: 400.0,
    line_height: 1.3,
};

pub trait StyledType: Styled + Sized {
    fn text_token(self, token: TextToken) -> Self {
        self.text_size(px(token.size))
            .font_weight(FontWeight(token.weight))
            .line_height(relative(token.line_height))
    }
}

impl<T: Styled + Sized> StyledType for T {}

/// A GPUI color from a `0xRRGGBBAA` token.
pub fn rgba_of(token: u32) -> Rgba {
    rgba(token)
}

/// The color with its alpha scaled by `factor`.
pub fn faded(color: Rgba, factor: f32) -> Rgba {
    Rgba {
        a: color.a * factor,
        ..color
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
}

impl Appearance {
    pub fn key(self) -> &'static str {
        match self {
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        }
    }

    pub fn is_dark(self) -> bool {
        self == Appearance::Dark
    }
}

/// What the system says about its appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum System {
    Light,
    Dark,
    /// The desktop did not say. The app uses Dark.
    Unknown,
}

impl System {
    pub fn key(self) -> &'static str {
        match self {
            System::Light => "light",
            System::Dark => "dark",
            System::Unknown => "unknown",
        }
    }
}

/// The system appearance as the platform reports it. Linux asks the settings portal, because
/// GPUI reports Light when the desktop gives no answer. Component tests take the window's value,
/// so a test can move it with the window.
#[cfg(all(target_os = "linux", not(test)))]
pub fn system_of(_window: WindowAppearance) -> System {
    use hushpen_platform::appearance::{Scheme, desktop_scheme};
    match desktop_scheme() {
        Some(Scheme::Dark) => System::Dark,
        Some(Scheme::Light) => System::Light,
        None => System::Unknown,
    }
}

#[cfg(any(not(target_os = "linux"), test))]
pub fn system_of(window: WindowAppearance) -> System {
    system_from_window(window)
}

#[cfg(any(not(target_os = "linux"), test))]
fn system_from_window(window: WindowAppearance) -> System {
    match window {
        WindowAppearance::Light | WindowAppearance::VibrantLight => System::Light,
        WindowAppearance::Dark | WindowAppearance::VibrantDark => System::Dark,
    }
}

/// The palette of one appearance. Views read it with [`palette`] and never write a color.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub appearance: Appearance,
    pub content: Rgba,
    pub sidebar: Rgba,
    pub group: Rgba,
    pub selection: Rgba,
    pub control: Rgba,
    pub label: Rgba,
    pub label_secondary: Rgba,
    pub label_tertiary: Rgba,
    pub separator: Rgba,
    pub accent: Rgba,
    pub success: Rgba,
    pub warning: Rgba,
    pub error: Rgba,
    pub on_accent: Rgba,
}

impl Palette {
    pub fn of(appearance: Appearance) -> Self {
        macro_rules! palette {
            ($set:ident) => {
                Self {
                    appearance,
                    content: rgba($set::CONTENT),
                    sidebar: rgba($set::SIDEBAR),
                    group: rgba($set::GROUP),
                    selection: rgba($set::SELECTION),
                    control: rgba($set::CONTROL),
                    label: rgba($set::LABEL),
                    label_secondary: rgba($set::LABEL_SECONDARY),
                    label_tertiary: rgba($set::LABEL_TERTIARY),
                    separator: rgba($set::SEPARATOR),
                    accent: rgba($set::ACCENT),
                    success: rgba($set::SUCCESS),
                    warning: rgba($set::WARNING),
                    error: rgba($set::ERROR),
                    on_accent: rgba(chrome::ON_ACCENT),
                }
            };
        }
        match appearance {
            Appearance::Dark => palette!(dark),
            Appearance::Light => palette!(light),
        }
    }

    /// The fill of a row under the pointer: half the selection fill.
    pub fn hover(&self) -> Rgba {
        faded(self.selection, 0.6)
    }
}

/// What the app knows about the appearance.
#[derive(Debug, Clone, Copy)]
struct Current {
    system: System,
    forced: Option<Appearance>,
}

impl Global for Current {}

impl Current {
    fn appearance(self) -> Appearance {
        self.forced.unwrap_or(match self.system {
            System::Light => Appearance::Light,
            System::Dark | System::Unknown => Appearance::Dark,
        })
    }
}

/// The appearance in use. Dark until [`install`] ran.
pub fn appearance(cx: &App) -> Appearance {
    cx.try_global::<Current>()
        .map_or(Appearance::Dark, |current| current.appearance())
}

/// The palette in use. Views call this while they render, so they follow a change.
pub fn palette(cx: &App) -> Palette {
    Palette::of(appearance(cx))
}

/// Reads the system appearance and sets the Kit controls to match. Call it once at start.
pub fn install(cx: &mut App) {
    let system = system_of(cx.window_appearance());
    cx.set_global(Current {
        system,
        forced: None,
    });
    apply(cx);
}

/// Follows the system appearance for the lifetime of the returned subscription. The open
/// windows repaint with the new palette; no view state is touched.
pub fn observe(window: &mut Window) -> Subscription {
    window.observe_window_appearance(|window, cx| {
        set_system(system_of(window.appearance()), cx);
    })
}

/// A new system appearance, as the platform reports it.
pub fn set_system(system: System, cx: &mut App) {
    change(cx, |current| current.system = system);
}

/// Forces the palette, or follows the system again with `None`. It changes nothing outside this
/// app, and on macOS it also sets the app's own appearance so the native title bar matches.
#[cfg(any(test, feature = "test-automation", feature = "pixel-tests"))]
pub fn force(appearance: Option<Appearance>, cx: &mut App) {
    cx.set_window_appearance(appearance.map(|each| match each {
        Appearance::Light => WindowAppearance::Light,
        Appearance::Dark => WindowAppearance::Dark,
    }));
    change(cx, |current| current.forced = appearance);
    if appearance.is_none() {
        set_system(system_of(cx.window_appearance()), cx);
    }
}

/// What the test hook reports: the system reading and the forced palette.
#[cfg(any(test, feature = "test-automation", feature = "pixel-tests"))]
pub fn sources(cx: &App) -> (System, Option<Appearance>) {
    cx.try_global::<Current>()
        .map_or((System::Unknown, None), |current| {
            (current.system, current.forced)
        })
}

fn change(cx: &mut App, edit: impl FnOnce(&mut Current)) {
    let mut current = cx.try_global::<Current>().copied().unwrap_or(Current {
        system: System::Unknown,
        forced: None,
    });
    let before = current.appearance();
    edit(&mut current);
    let changed = cx.try_global::<Current>().is_none() || current.appearance() != before;
    cx.set_global(current);
    if changed {
        apply(cx);
        cx.refresh_windows();
    }
}

/// Points the Kit controls at the palette in use.
fn apply(cx: &mut App) {
    let palette = palette(cx);
    let mode = if palette.appearance.is_dark() {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    Theme::change(mode, None, cx);
    Theme::update(cx, |theme| {
        theme.font_family = FONT_FAMILY.into();
        theme.radius = px(radius::BUTTON);
        theme.radius_lg = px(radius::GROUP);
        theme.colors.background = palette.content.into();
        theme.colors.foreground = palette.label.into();
        theme.colors.muted_foreground = palette.label_secondary.into();
        theme.colors.border = palette.separator.into();
        theme.colors.input = palette.separator.into();
        theme.colors.ring = palette.accent.into();
        theme.colors.popover = rgba(if palette.appearance.is_dark() {
            dark::SIDEBAR
        } else {
            0xFFFFFFFF
        })
        .into();
        theme.colors.popover_foreground = palette.label.into();
        theme.colors.sidebar = palette.sidebar.into();
        theme.colors.sidebar_foreground = palette.label.into();
        theme.colors.sidebar_border = palette.separator.into();
        theme.colors.sidebar_accent = palette.selection.into();
        theme.colors.sidebar_accent_foreground = palette.label.into();
        theme.colors.list_hover = palette.hover().into();
        theme.colors.list_active = palette.selection.into();
    });
}

thread_local! {
    static SCALE: Cell<f32> = const { Cell::new(1.0) };
}

/// Tells the views the pixel density of the window, for the row lines. The shell calls it on
/// every render.
pub fn sync(window: &Window) {
    SCALE.with(|cell| cell.set(window.scale_factor().max(1.0)));
}

/// One device pixel: the width of a line between rows. A layout height of half a logical pixel
/// would vanish on a standard display wherever a row starts between two device pixels.
pub fn hairline() -> gpui_kit::Pixels {
    px(1.0 / SCALE.with(Cell::get))
}

/// Fonts and colors for the flow bar. It stays a dark HUD in both appearances, so it reads these
/// colors whatever the window around it shows.
pub mod hud {
    use super::TextToken;
    pub mod color {
        pub const SURFACE_CONTROL: u32 = 0x303030;
        pub const SURFACE_CONTROL_HOVER: u32 = 0x3A3A3A;
        pub const SURFACE_ROW_HOVER: u32 = 0x424242;
        pub const SURFACE_SELECTED: u32 = 0x5A5A5A;
        pub const SURFACE_ELEVATED: u32 = 0x2B2B2B;
        pub const BORDER_CONTROL: u32 = 0x5C5C5C;
        pub const DIVIDER: u32 = 0x474747;
        pub const TEXT_PRIMARY: u32 = 0xEEEEEE;
        pub const TEXT_BODY: u32 = 0xCFCFCF;
        pub const TEXT_MUTED: u32 = 0xA8A8A8;
        pub const TEXT_SUBTLE: u32 = 0x8E8E8E;
        pub const TEXT_DISABLED: u32 = 0x8A8A8A;
        pub const FOCUS: u32 = 0x9BCFFF;
        pub const STATUS_ERROR: u32 = 0xFF9C8F;
        pub const STATUS_WARNING: u32 = 0xFFB3AA;
    }

    pub mod radius {
        pub const CONTROL: f32 = 10.0;
        pub const MD: f32 = 12.0;
        pub const FULL: f32 = super::super::radius::FULL;
    }

    pub const BODY_SM: TextToken = TextToken {
        size: 12.0,
        weight: 500.0,
        line_height: 1.67,
    };
    pub const LABEL_MD: TextToken = TextToken {
        size: 12.0,
        weight: 600.0,
        line_height: 1.0,
    };
    pub const LABEL_CAPS: TextToken = TextToken {
        size: 11.0,
        weight: 600.0,
        line_height: 1.82,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `name: value` lines of one top-level block in the DESIGN.md front matter.
    fn design_block(name: &str) -> Vec<(String, String)> {
        let design = include_str!("../../../DESIGN.md");
        let mut lines = design.lines().skip_while(|l| *l != format!("{name}:"));
        lines.next();
        lines
            .take_while(|l| l.starts_with("  "))
            .filter_map(|l| {
                let (key, value) = l.trim().split_once(':')?;
                let value = value.trim().trim_matches('"');
                (!value.is_empty()).then(|| (key.trim_matches('"').to_string(), value.to_string()))
            })
            .collect()
    }

    /// `0xRRGGBBAA` of a `#RRGGBB` or `#RRGGBBAA` value.
    fn hex(value: &str) -> u32 {
        let digits = value.trim_start_matches('#');
        let number = u32::from_str_radix(digits, 16).unwrap();
        match digits.len() {
            6 => (number << 8) | 0xFF,
            8 => number,
            other => panic!("{value} has {other} digits"),
        }
    }

    fn px_value(value: &str) -> f32 {
        value.trim_end_matches("px").parse().unwrap()
    }

    /// One field of one entry in the `components` block, such as `("button", "height")`.
    fn component(name: &str, field: &str) -> String {
        let design = include_str!("../../../DESIGN.md");
        let mut lines = design
            .lines()
            .skip_while(|l| *l != "components:")
            .skip_while(|l| l.trim() != format!("{name}:"))
            .skip(1);
        lines
            .by_ref()
            .take_while(|l| l.starts_with("    "))
            .find_map(|l| {
                let (key, value) = l.trim().split_once(':')?;
                (key == field).then(|| value.trim().trim_matches('"').to_string())
            })
            .unwrap_or_else(|| panic!("DESIGN.md has no {name}.{field}"))
    }

    #[test]
    fn every_design_color_is_in_the_theme_with_the_same_value() {
        let design = design_block("colors");
        assert!(design.len() > 30, "parsed {} colors", design.len());
        let theme: Vec<(&str, u32)> = dark::TABLE
            .iter()
            .chain(light::TABLE)
            .chain(chrome::TABLE)
            .copied()
            .collect();
        for (name, value) in &design {
            let (_, theme_value) = theme
                .iter()
                .find(|(token, _)| token == name)
                .unwrap_or_else(|| panic!("theme lacks color {name}"));
            assert_eq!(*theme_value, hex(value), "color {name}");
        }
        assert_eq!(
            theme.len(),
            design.len(),
            "theme has a color DESIGN.md lacks"
        );
    }

    #[test]
    fn every_design_radius_is_in_the_theme_with_the_same_value() {
        let design = design_block("rounded");
        for (name, value) in &design {
            let theme = radius::TABLE.iter().find(|(token, _)| token == name);
            let (_, theme_value) = theme.unwrap_or_else(|| panic!("theme lacks radius {name}"));
            assert_eq!(*theme_value, px_value(value), "radius {name}");
        }
        assert_eq!(
            radius::TABLE.len(),
            design.len(),
            "theme has a radius DESIGN.md lacks"
        );
    }

    #[test]
    fn every_design_spacing_is_in_the_theme_with_the_same_value() {
        let design = design_block("spacing");
        for (name, value) in &design {
            let theme = space::TABLE.iter().find(|(token, _)| token == name);
            let (_, theme_value) = theme.unwrap_or_else(|| panic!("theme lacks spacing {name}"));
            assert_eq!(*theme_value, px_value(value), "spacing {name}");
        }
        assert_eq!(space::TABLE.len(), design.len());
    }

    #[test]
    fn typography_rows_match_design() {
        let design = include_str!("../../../DESIGN.md");
        for (name, token) in [
            ("title", TITLE),
            ("toolbar-title", TOOLBAR_TITLE),
            ("body", BODY),
            ("button", BUTTON),
            ("secondary", SECONDARY),
        ] {
            let block: Vec<&str> = design
                .lines()
                .skip_while(|l| l.trim() != format!("{name}:"))
                .skip(1)
                .take(5)
                .collect();
            let field = |key: &str| -> f32 {
                let line = block.iter().find(|l| l.trim().starts_with(key)).unwrap();
                px_value(line.split(':').nth(1).unwrap().trim())
            };
            assert_eq!(field("fontSize"), token.size, "{name} size");
            assert_eq!(field("fontWeight"), token.weight, "{name} weight");
            assert_eq!(field("lineHeight"), token.line_height, "{name} line height");
        }
        assert_eq!(
            design.matches("fontSize:").count(),
            5,
            "DESIGN.md has a type the theme lacks"
        );
    }

    #[test]
    fn component_sizes_match_design() {
        let size_of = |name, field| px_value(&component(name, field));
        assert_eq!(size_of("sidebar-row", "height"), size::NAV_ITEM_HEIGHT);
        assert_eq!(size_of("sidebar-row", "padding"), size::NAV_ITEM_PADDING);
        assert_eq!(size_of("sidebar-icon", "size"), size::NAV_ICON);
        assert_eq!(size_of("row", "minHeight"), size::ROW_MIN_HEIGHT);
        assert_eq!(size_of("row", "padding"), size::ROW_PADDING);
        assert_eq!(size_of("button", "height"), size::BUTTON_HEIGHT);
        assert_eq!(size_of("button", "padding"), size::BUTTON_PADDING);
        assert_eq!(size_of("traffic-light-close", "size"), size::TRAFFIC_LIGHT);
        assert_eq!(
            size_of("traffic-light-minimize", "size"),
            size::TRAFFIC_LIGHT
        );
        assert_eq!(size_of("focus-ring", "size"), size::FOCUS_RING);
        assert_eq!(
            component("app-window", "width"),
            "{spacing.window-width}",
            "the window width is the spacing token"
        );
        assert_eq!(component("app-window", "height"), "{spacing.window-height}");
        assert_eq!(component("sidebar", "width"), "{spacing.sidebar-width}");
    }

    #[test]
    fn component_radii_and_colors_point_at_the_right_tokens() {
        assert_eq!(component("sidebar-row", "rounded"), "{rounded.selection}");
        assert_eq!(component("group", "rounded"), "{rounded.group}");
        assert_eq!(component("button", "rounded"), "{rounded.button}");
        assert_eq!(
            component("button-default", "backgroundColor"),
            "{colors.dark-accent}"
        );
        assert_eq!(
            component("sidebar-row-selected", "backgroundColor"),
            "{colors.dark-selection}"
        );
        assert_eq!(
            component("focus-ring", "borderColor"),
            "{colors.dark-accent}"
        );
    }

    #[test]
    fn the_solid_sidebar_colors_are_the_design_sidebar_colors() {
        assert_eq!(chrome::SIDEBAR_SOLID_DARK, 0x2B2B2D);
        assert_eq!(chrome::SIDEBAR_SOLID_LIGHT, 0xE8E8EA);
        assert_eq!(chrome::sidebar_fill(false, true), Some(0x2B2B2D));
        assert_eq!(chrome::sidebar_fill(false, false), Some(0xE8E8EA));
        assert_eq!(chrome::sidebar_fill(true, true), None);
        assert_eq!(chrome::sidebar_fill(true, false), None);
    }

    #[test]
    fn each_appearance_reads_its_own_tokens() {
        let dark = Palette::of(Appearance::Dark);
        let light = Palette::of(Appearance::Light);
        assert_eq!(dark.content, rgba(0x1E1E1EFF));
        assert_eq!(light.content, rgba(0xF5F5F5FF));
        assert_eq!(dark.accent, rgba(0x0A84FFFF));
        assert_eq!(light.accent, rgba(0x007AFFFF));
        assert_eq!(dark.on_accent, light.on_accent);
        assert_ne!(dark.label, light.label);
        assert_ne!(dark.separator, light.separator);
    }

    #[test]
    fn the_system_reading_maps_to_a_system_value() {
        assert_eq!(system_from_window(WindowAppearance::Light), System::Light);
        assert_eq!(
            system_from_window(WindowAppearance::VibrantLight),
            System::Light
        );
        assert_eq!(system_from_window(WindowAppearance::Dark), System::Dark);
        assert_eq!(
            system_from_window(WindowAppearance::VibrantDark),
            System::Dark
        );
    }

    #[test]
    fn an_unknown_system_uses_dark_and_a_forced_palette_wins() {
        let resolve = |system, forced| Current { system, forced }.appearance();
        assert_eq!(resolve(System::Unknown, None), Appearance::Dark);
        assert_eq!(resolve(System::Dark, None), Appearance::Dark);
        assert_eq!(resolve(System::Light, None), Appearance::Light);
        assert_eq!(
            resolve(System::Dark, Some(Appearance::Light)),
            Appearance::Light
        );
        assert_eq!(
            resolve(System::Light, Some(Appearance::Dark)),
            Appearance::Dark
        );
    }

    #[test]
    fn the_flow_bar_is_a_dark_hud_whatever_the_window_shows() {
        assert_eq!(hud::color::SURFACE_ELEVATED, 0x2B2B2B);
    }

    #[test]
    fn design_md_keeps_none_of_the_old_values() {
        let design = include_str!("../../../DESIGN.md");
        for old in [
            "#2F2F2F",
            "#3C3C3C",
            "#686868",
            "#9BCFFF",
            "780",
            "520",
            "208",
            "panel:",
            "rounded.panel",
        ] {
            assert!(!design.contains(old), "DESIGN.md still has {old}");
        }
        for word in ["orange", "purple", "Orange", "Purple"] {
            assert!(!design.contains(word), "DESIGN.md still has {word}");
        }
    }
}
