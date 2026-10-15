//! The cargo-packager and cargo-generate-rpm configuration, rendered per run so
//! the version, the paths, and the derived dependencies have one source.

use std::path::{Path, PathBuf};

use super::deps::Deps;

pub const PRODUCT: &str = "Hushpen";
pub const BUNDLE_ID: &str = "io.github.surajmandalcell.hushpen";
const DESCRIPTION: &str = "Private speech recognition and dictation that runs on your computer";
const HOMEPAGE: &str = "https://github.com/surajmandalcell/asrpro";
const PUBLISHER: &str = "Suraj Mandal";
pub const MIN_MACOS: &str = "13.0";

/// The `.desktop` file for the rpm; cargo-packager writes its own for deb and AppImage.
pub fn desktop_entry() -> String {
    format!(
        "[Desktop Entry]\nCategories=Utility;\nComment={DESCRIPTION}\nExec=hushpen\nIcon=hushpen\nName={PRODUCT}\nTerminal=false\nType=Application\n"
    )
}

/// The rpm has no symlink asset, so `/usr/bin/hushpen` is a one-line launcher
/// that hands over to the binary next to its sibling worker.
pub fn launcher_script() -> &'static str {
    "#!/bin/sh\nexec /usr/lib/hushpen/hushpen \"$@\"\n"
}

fn quote(text: &str) -> String {
    format!("{text:?}")
}

fn quote_path(path: &Path) -> String {
    quote(&path.display().to_string())
}

fn list(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|item| quote(item)).collect();
    format!("[{}]", quoted.join(", "))
}

pub struct Common<'a> {
    pub version: &'a str,
    pub work: &'a Path,
    pub icons: &'a [PathBuf],
    /// Source file and its place under the package's resource folder.
    pub resources: &'a [(PathBuf, String)],
}

fn common(input: &Common) -> String {
    let icons: Vec<String> = input
        .icons
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    let resources: Vec<String> = input
        .resources
        .iter()
        .map(|(src, target)| {
            format!(
                "{{ src = {}, target = {} }}",
                quote_path(src),
                quote(target)
            )
        })
        .collect();
    format!(
        "name = \"hushpen\"\nproduct-name = {product}\nversion = {version}\nidentifier = {id}\n\
         description = {description}\nlong-description = {description}\npublisher = {publisher}\n\
         authors = [{publisher}]\nhomepage = {homepage}\ncategory = \"Utility\"\n\
         out-dir = {work}\nbinaries-dir = {work}\nicons = {icons}\n\
         binaries = [{{ path = \"hushpen\", main = true }}, {{ path = \"hushpen-llm\" }}]\n\
         resources = [{resources}]\n",
        product = quote(PRODUCT),
        version = quote(input.version),
        id = quote(BUNDLE_ID),
        description = quote(DESCRIPTION),
        publisher = quote(PUBLISHER),
        homepage = quote(HOMEPAGE),
        work = quote_path(input.work),
        icons = list(&icons),
        resources = resources.join(", "),
    )
}

/// The `.app` config. `sign_script` runs before the dmg step so the app inside
/// the dmg carries the ad-hoc signature too.
pub fn mac(input: &Common, info_plist: &Path, sign_script: &Path) -> String {
    format!(
        "{}before-each-package-command = {}\n\n[macos]\nminimum-system-version = {}\ninfo-plist-path = {}\n",
        common(input),
        quote(&format!("sh {}", sign_script.display())),
        quote(MIN_MACOS),
        quote_path(info_plist),
    )
}

pub fn deb(input: &Common, deps: &Deps) -> String {
    format!(
        "{}\n[deb]\nsection = \"sound\"\ndepends = {}\n",
        common(input),
        list(&deps.deb_depends)
    )
}

pub fn appimage(input: &Common) -> String {
    common(input)
}

/// What the app tells the script that signs it: ad-hoc, no identity.
pub fn sign_script(app: &Path) -> String {
    format!(
        "#!/bin/sh\n[ \"$CARGO_PACKAGER_FORMAT\" = dmg ] || exit 0\ncodesign --force --deep -s - {}\n",
        shell_quote(&app.display().to_string())
    )
}

pub fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// `[package.metadata.generate-rpm]` overrides for `--metadata-overwrite`.
pub fn rpm_metadata(version: &str, assets: &[(PathBuf, String, &str)], deps: &Deps) -> String {
    let mut text = format!(
        "name = \"hushpen\"\nversion = {}\nlicense = \"AGPL-3.0-or-later\"\nsummary = {}\n\
         description = {}\nurl = {}\nvendor = {}\n",
        quote(version),
        quote(DESCRIPTION),
        quote(DESCRIPTION),
        quote(HOMEPAGE),
        quote(PUBLISHER),
    );
    text.push_str("assets = [\n");
    for (source, dest, mode) in assets {
        text.push_str(&format!(
            "  {{ source = {}, dest = {}, mode = {} }},\n",
            quote_path(source),
            quote(dest),
            quote(mode)
        ));
    }
    text.push_str("]\n");
    for (table, names) in [
        ("requires", &deps.rpm_requires),
        ("recommends", &deps.rpm_recommends),
    ] {
        text.push_str(&format!("\n[{table}]\n"));
        for name in names {
            text.push_str(&format!("{} = \"*\"\n", quote(name)));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deps() -> Deps {
        Deps {
            deb_depends: vec!["libvulkan1".into(), "a | b".into()],
            deb_recommends: vec!["xdg-desktop-portal".into()],
            rpm_requires: vec!["vulkan-loader".into()],
            rpm_recommends: vec!["xdg-desktop-portal".into()],
        }
    }

    fn input<'a>(
        work: &'a Path,
        icons: &'a [PathBuf],
        resources: &'a [(PathBuf, String)],
    ) -> Common<'a> {
        Common {
            version: "2.0.0",
            work,
            icons,
            resources,
        }
    }

    fn parse(text: &str) -> toml_probe::Table {
        toml_probe::parse(text)
    }

    #[test]
    fn the_mac_config_names_both_binaries_the_plist_and_the_signing_hook() {
        let work = PathBuf::from("/w");
        let icons = [PathBuf::from("/a/icon.icns")];
        let resources = [(PathBuf::from("/a/N.html"), "N.html".to_string())];
        let text = mac(
            &input(&work, &icons, &resources),
            Path::new("/p/Info.plist"),
            Path::new("/w/sign.sh"),
        );
        let table = parse(&text);
        assert_eq!(table.get("version"), Some("2.0.0"));
        assert_eq!(
            table.get("identifier"),
            Some("io.github.surajmandalcell.hushpen")
        );
        assert!(text.contains("path = \"hushpen\", main = true"));
        assert!(text.contains("path = \"hushpen-llm\""));
        assert!(text.contains("minimum-system-version = \"13.0\""));
        assert!(text.contains("info-plist-path = \"/p/Info.plist\""));
        assert!(text.contains("before-each-package-command = \"sh /w/sign.sh\""));
    }

    #[test]
    fn the_deb_config_carries_the_derived_depends() {
        let work = PathBuf::from("/w");
        let text = deb(&input(&work, &[], &[]), &deps());
        assert!(text.contains("depends = [\"libvulkan1\", \"a | b\"]"));
    }

    #[test]
    fn the_rpm_metadata_has_license_requires_recommends_and_assets() {
        let assets = [(
            PathBuf::from("/s/hushpen"),
            "/usr/lib/hushpen/hushpen".to_string(),
            "755",
        )];
        let text = rpm_metadata("2.0.0", &assets, &deps());
        assert!(text.contains("license = \"AGPL-3.0-or-later\""));
        assert!(text.contains("[requires]\n\"vulkan-loader\" = \"*\""));
        assert!(text.contains("[recommends]\n\"xdg-desktop-portal\" = \"*\""));
        assert!(text.contains(
            "{ source = \"/s/hushpen\", dest = \"/usr/lib/hushpen/hushpen\", mode = \"755\" }"
        ));
    }

    #[test]
    fn the_signing_script_is_ad_hoc_and_only_acts_before_the_dmg() {
        let script = sign_script(Path::new("/w/Hushpen.app"));
        assert!(script.contains("CARGO_PACKAGER_FORMAT\" = dmg ] || exit 0"));
        assert!(script.contains("codesign --force --deep -s - '/w/Hushpen.app'"));
    }

    #[test]
    fn the_launcher_hands_over_to_the_binary_beside_its_worker() {
        assert!(launcher_script().contains("exec /usr/lib/hushpen/hushpen \"$@\""));
        assert!(desktop_entry().contains("Exec=hushpen"));
    }

    /// A tiny reader for the flat `key = "value"` lines the tests look at.
    mod toml_probe {
        pub struct Table(Vec<(String, String)>);
        impl Table {
            pub fn get(&self, key: &str) -> Option<&str> {
                self.0
                    .iter()
                    .find(|(k, _)| k == key)
                    .map(|(_, v)| v.as_str())
            }
        }
        pub fn parse(text: &str) -> Table {
            Table(
                text.lines()
                    .filter_map(|line| {
                        let (key, value) = line.split_once(" = ")?;
                        let value = value.strip_prefix('"')?.strip_suffix('"')?;
                        Some((key.to_string(), value.to_string()))
                    })
                    .collect(),
            )
        }
    }
}
