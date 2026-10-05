//! Every DESIGN.md token, in one place. Views take colors, sizes, and type
//! from here and never write a literal. The test at the bottom reads DESIGN.md
//! and fails when a token drifts.

use gpui_kit::component::Theme;
use gpui_kit::{App, FontWeight, Rgba, Styled, px, relative, rgb};

macro_rules! tokens {
    ($table:ident: $ty:ty { $($name:ident = $value:expr, $token:literal;)* }) => {
        $(pub const $name: $ty = $value;)*
        /// DESIGN.md token name and value, for the drift test.
        #[cfg(test)]
        pub(crate) const $table: &[(&str, $ty)] = &[$(($token, $name)),*];
    };
}

/// Colors as `0xRRGGBB`. Use [`rgb_of`] to get a GPUI color.
pub mod color {
    tokens!(TABLE: u32 {
        BACKGROUND = 0x2F2F2F, "background";
        BACKGROUND_RAISED = 0x333333, "background-raised";
        SIDEBAR = 0x3C3C3C, "sidebar";
        SIDEBAR_ACTIVE = 0x686868, "sidebar-active";
        SURFACE = 0x3A3A3A, "surface";
        SURFACE_SOFT = 0x383838, "surface-soft";
        SURFACE_CONTROL = 0x303030, "surface-control";
        SURFACE_CONTROL_HOVER = 0x3A3A3A, "surface-control-hover";
        SURFACE_ROW_HOVER = 0x424242, "surface-row-hover";
        SURFACE_SELECTED = 0x5A5A5A, "surface-selected";
        SURFACE_ELEVATED = 0x2B2B2B, "surface-elevated";
        BORDER = 0x3F3F3F, "border";
        BORDER_SIDEBAR = 0x545454, "border-sidebar";
        BORDER_CONTROL = 0x5C5C5C, "border-control";
        DIVIDER = 0x474747, "divider";
        TEXT_PRIMARY = 0xEEEEEE, "text-primary";
        TEXT_HEADING = 0xF4F4F4, "text-heading";
        TEXT_BODY = 0xCFCFCF, "text-body";
        TEXT_MUTED = 0xA8A8A8, "text-muted";
        TEXT_SUBTLE = 0x8E8E8E, "text-subtle";
        TEXT_DISABLED = 0x8A8A8A, "text-disabled";
        FOCUS = 0x9BCFFF, "focus";
        ACCENT_BLUE = 0x0A84FF, "accent-blue";
        ACCENT_ORANGE = 0xFF7A32, "accent-orange";
        ACCENT_PURPLE = 0x7167FF, "accent-purple";
        ACCENT_TEAL = 0x92C2C6, "accent-teal";
        STATUS_ERROR = 0xFF9C8F, "status-error";
        STATUS_WARNING = 0xFFB3AA, "status-warning";
        LOGO_SURFACE = 0x10171D, "logo-surface";
        LOGO_INK = 0xEEF4F5, "logo-ink";
        ICON_GLASS = 0x20272D, "icon-glass";
        ICON_GLASS_ACCENT = 0x10171D, "icon-glass-accent";
        ICON_SHADOW = 0x04070A, "icon-shadow";
        WINDOW_CLOSE = 0xFF5F57, "window-close";
        WINDOW_CLOSE_GLYPH = 0x6E140F, "window-close-glyph";
        WINDOW_MINIMIZE = 0xFEBC2E, "window-minimize";
        WINDOW_MINIMIZE_GLYPH = 0x8F5B00, "window-minimize-glyph";
    });
}

/// Corner radii in logical pixels.
pub mod radius {
    tokens!(TABLE: f32 {
        NONE = 0.0, "none";
        XS = 5.0, "xs";
        SM = 7.0, "sm";
        NAV = 9.0, "nav";
        CONTROL = 10.0, "control";
        MD = 12.0, "md";
        APP_ICON = 16.0, "app-icon";
        PANEL = 22.0, "panel";
        FULL = 9999.0, "full";
    });
}

/// Spacing and fixed layout sizes in logical pixels.
pub mod space {
    tokens!(TABLE: f32 {
        HAIRLINE = 1.0, "hairline";
        XXS = 2.0, "xxs";
        XS = 4.0, "xs";
        SM = 8.0, "sm";
        MD = 12.0, "md";
        LG = 16.0, "lg";
        XL = 20.0, "xl";
        XXL = 24.0, "2xl";
        XXXL = 32.0, "3xl";
        SIDEBAR_WIDTH = 208.0, "sidebar-width";
        CONTENT_MAX_WIDTH = 520.0, "content-max-width";
        WINDOW_WIDTH = 780.0, "window-width";
        WINDOW_HEIGHT = 520.0, "window-height";
        TOOLBAR_HEIGHT = 34.0, "toolbar-height";
        SIDEBAR_TITLE_HEIGHT = 48.0, "sidebar-title-height";
    });
}

/// Sizes taken from the `components` block of DESIGN.md.
pub mod size {
    pub const NAV_ITEM_HEIGHT: f32 = 36.0;
    pub const NAV_ITEM_PADDING: f32 = 10.0;
    pub const NAV_ICON_TILE: f32 = 20.0;
    pub const TRAFFIC_LIGHT: f32 = 13.0;
    pub const FOCUS_RING: f32 = 2.0;
}

pub const FONT_FAMILY: &str = "Inter";

/// One row of the `typography` block.
#[derive(Debug, Clone, Copy)]
pub struct TextToken {
    pub size: f32,
    pub weight: f32,
    pub line_height: f32,
}

pub const DISPLAY_MD: TextToken = TextToken {
    size: 24.0,
    weight: 600.0,
    line_height: 1.17,
};
pub const TITLE_MD: TextToken = TextToken {
    size: 15.0,
    weight: 600.0,
    line_height: 1.0,
};
pub const ROW_TITLE: TextToken = TextToken {
    size: 14.0,
    weight: 600.0,
    line_height: 1.43,
};
pub const BODY_MD: TextToken = TextToken {
    size: 13.0,
    weight: 500.0,
    line_height: 1.54,
};
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

pub fn rgb_of(hex: u32) -> Rgba {
    rgb(hex)
}

pub trait StyledType: Styled + Sized {
    fn text_token(self, token: TextToken) -> Self {
        self.text_size(px(token.size))
            .font_weight(FontWeight(token.weight))
            .line_height(relative(token.line_height))
    }
}

impl<T: Styled + Sized> StyledType for T {}

/// Point the GPUI Kit dark theme at the DESIGN.md palette so Kit controls match
/// the views drawn from tokens.
pub fn install(cx: &mut App) {
    use color::*;
    Theme::change(gpui_kit::component::ThemeMode::Dark, None, cx);
    Theme::update(cx, |theme| {
        theme.font_family = FONT_FAMILY.into();
        theme.radius = px(radius::MD);
        theme.radius_lg = px(radius::PANEL);
        theme.colors.background = rgb(BACKGROUND).into();
        theme.colors.foreground = rgb(TEXT_PRIMARY).into();
        theme.colors.muted_foreground = rgb(TEXT_MUTED).into();
        theme.colors.border = rgb(DIVIDER).into();
        theme.colors.input = rgb(BORDER_CONTROL).into();
        theme.colors.ring = rgb(FOCUS).into();
        theme.colors.popover = rgb(SURFACE_CONTROL).into();
        theme.colors.popover_foreground = rgb(TEXT_PRIMARY).into();
        theme.colors.sidebar = rgb(SIDEBAR).into();
        theme.colors.sidebar_foreground = rgb(TEXT_BODY).into();
        theme.colors.sidebar_border = rgb(BORDER_SIDEBAR).into();
        theme.colors.sidebar_accent = rgb(SIDEBAR_ACTIVE).into();
        theme.colors.sidebar_accent_foreground = rgb(TEXT_PRIMARY).into();
        theme.colors.list_hover = rgb(SURFACE_ROW_HOVER).into();
        theme.colors.list_active = rgb(SURFACE_SELECTED).into();
    });
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

    fn hex(value: &str) -> u32 {
        u32::from_str_radix(value.trim_start_matches('#'), 16).unwrap()
    }

    fn px_value(value: &str) -> f32 {
        value.trim_end_matches("px").parse().unwrap()
    }

    #[test]
    fn every_design_color_is_in_the_theme_with_the_same_value() {
        let design = design_block("colors");
        assert!(design.len() > 30, "parsed {} colors", design.len());
        for (name, value) in &design {
            let theme = color::TABLE.iter().find(|(token, _)| token == name);
            let (_, theme_value) = theme.unwrap_or_else(|| panic!("theme lacks color {name}"));
            assert_eq!(*theme_value, hex(value), "color {name}");
        }
        assert_eq!(
            color::TABLE.len(),
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
            ("display-md", DISPLAY_MD),
            ("title-md", TITLE_MD),
            ("row-title", ROW_TITLE),
            ("body-md", BODY_MD),
            ("body-sm", BODY_SM),
            ("label-md", LABEL_MD),
            ("label-caps", LABEL_CAPS),
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
    }

    #[test]
    fn component_sizes_match_design() {
        let design = include_str!("../../../DESIGN.md");
        assert!(design.contains("    height: 36px"), "nav-item height");
        assert_eq!(size::NAV_ITEM_HEIGHT, 36.0);
        assert!(design.contains("    size: 13px"), "traffic light size");
        assert_eq!(size::TRAFFIC_LIGHT, 13.0);
        assert!(design.contains("    size: 2px"), "focus ring size");
        assert_eq!(size::FOCUS_RING, 2.0);
    }
}
