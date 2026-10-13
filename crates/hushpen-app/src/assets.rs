//! Embedded fonts and icons.

use gpui_kit::assets::{Assets as KitAssets, icon_assets};
use gpui_kit::{App, AssetSource, SharedString};
use std::borrow::Cow;

pub const INTER_MEDIUM: &[u8] = include_bytes!("../../../assets/fonts/Inter-Medium.ttf");
pub const INTER_SEMIBOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf");

/// The font files store their names as UTF-16, which plain `strings` does not
/// show, so the binary also carries the notice as ASCII.
pub const INTER_NOTICE: &str =
    "Inter, Copyright (c) 2016 The Inter Project Authors, SIL Open Font License 1.1";

/// Load Inter (weights 500 and 600). The app never depends on a system copy.
pub fn register_fonts(cx: &App) {
    let fonts = vec![Cow::Borrowed(INTER_MEDIUM), Cow::Borrowed(INTER_SEMIBOLD)];
    match cx.text_system().add_fonts(fonts) {
        Ok(()) => log::info!("embedded font loaded: {INTER_NOTICE}"),
        Err(error) => log::error!("could not load the embedded Inter font: {error}"),
    }
}

icon_assets!(
    ShellIcons,
    [
        House,
        Clock,
        BookA,
        Import,
        Cpu,
        Settings,
        Info,
        X,
        Minus,
        Mic,
        Keyboard,
        Check,
        CircleAlert,
        Square,
        SquareCheck,
        Monitor,
        Download,
        // The views name these too; the coverage test below scans the sources for
        // every icon a view uses.
        ArrowUpRight,
        Globe,
        ChevronDown
    ]
);

#[cfg(test)]
mod icon_coverage_tests {
    use super::*;
    use gpui_kit::assets::IconName;
    use std::collections::BTreeSet;
    use std::path::Path;

    fn sources(folder: &Path, found: &mut BTreeSet<String>) {
        for entry in std::fs::read_dir(folder).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                sources(&path, found);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                let mut rest = text.as_str();
                while let Some(at) = rest.find("IconName::") {
                    rest = &rest[at + "IconName::".len()..];
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric())
                        .collect();
                    if !name.is_empty() && name.chars().next().is_some_and(char::is_uppercase) {
                        found.insert(name);
                    }
                }
            }
        }
    }

    /// An icon that only the kit defaults provide loads but draws nothing, and no check fails.
    /// Every icon a view names must be in `ShellIcons`.
    #[test]
    fn every_icon_a_view_names_is_in_shell_icons() {
        let mut named = BTreeSet::new();
        sources(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut named,
        );
        named.remove("ALL");
        assert!(named.len() > 15, "the scan found {named:?}");
        let missing: Vec<_> = named
            .iter()
            .filter(|name| {
                let icon = IconName::ALL
                    .iter()
                    .find(|icon| format!("{icon:?}") == **name)
                    .unwrap_or_else(|| panic!("IconName::{name} does not exist"));
                ShellIcons.load(&icon.path()).unwrap().is_none()
            })
            .collect();
        assert!(missing.is_empty(), "missing from ShellIcons: {missing:?}");
    }
}

/// The app's own icons first, then the GPUI Kit defaults its controls use.
pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        match ShellIcons.load(path)? {
            Some(bytes) => Ok(Some(bytes)),
            None => KitAssets.load(path),
        }
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<SharedString>> {
        let mut listed = ShellIcons.list(path)?;
        listed.extend(KitAssets.list(path)?);
        Ok(listed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inter_is_embedded_with_its_license_beside_it() {
        assert!(INTER_MEDIUM.starts_with(&[0, 1, 0, 0]), "TrueType header");
        assert!(INTER_SEMIBOLD.starts_with(&[0, 1, 0, 0]), "TrueType header");
        let license = include_str!("../../../assets/fonts/OFL.txt");
        assert!(license.contains("SIL OPEN FONT LICENSE Version 1.1"));
        assert!(license.contains("The Inter Project Authors"));
    }

    #[test]
    fn shell_icons_load_before_kit_defaults() {
        for icon in [
            "icons/house.svg",
            "icons/clock.svg",
            "icons/x.svg",
            "icons/chevron-down.svg",
        ] {
            assert!(AppAssets.load(icon).unwrap().is_some(), "{icon}");
        }
        assert!(AppAssets.load("icons/missing.svg").is_err());
    }
}
