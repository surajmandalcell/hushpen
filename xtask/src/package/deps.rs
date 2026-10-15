//! Package dependencies derived from the Linux binary: the libraries it links
//! (`readelf -d` NEEDED entries) and the ones it loads at run time (the
//! soname strings inside it). Every soname maps to a Debian and a Fedora
//! package. A linked library with no mapping stops the build, so a new
//! dependency can never ship without a package line.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

struct Lib {
    soname: &'static str,
    deb: &'static str,
    rpm: &'static str,
}

const fn lib(soname: &'static str, deb: &'static str, rpm: &'static str) -> Lib {
    Lib { soname, deb, rpm }
}

const LIBS: &[Lib] = &[
    lib("libvulkan.so.1", "libvulkan1", "vulkan-loader"),
    lib("libxkbcommon.so.0", "libxkbcommon0", "libxkbcommon"),
    lib(
        "libxkbcommon-x11.so.0",
        "libxkbcommon-x11-0",
        "libxkbcommon-x11",
    ),
    lib("libxcb.so.1", "libxcb1", "libxcb"),
    lib("libX11.so.6", "libx11-6", "libX11"),
    lib("libX11-xcb.so.1", "libx11-xcb1", "libX11-xcb"),
    lib("libXi.so.6", "libxi6", "libXi"),
    lib("libasound.so.2", "libasound2t64 | libasound2", "alsa-lib"),
    lib("libfontconfig.so.1", "libfontconfig1", "fontconfig"),
    lib("libfreetype.so.6", "libfreetype6", "freetype"),
    lib(
        "libwayland-client.so.0",
        "libwayland-client0",
        "libwayland-client",
    ),
    lib(
        "libwayland-cursor.so.0",
        "libwayland-cursor0",
        "libwayland-cursor",
    ),
    lib("libwayland-egl.so.1", "libwayland-egl1", "libwayland-egl"),
    lib("libdbus-1.so.3", "libdbus-1-3", "dbus-libs"),
    lib("libz.so.1", "zlib1g", "zlib"),
    lib("libstdc++.so.6", "libstdc++6", "libstdc++"),
    lib("libgomp.so.1", "libgomp1", "libgomp"),
];

/// Libraries every libc-based system has; they need no package line.
const BASE: &[&str] = &[
    "libc.so.6",
    "libm.so.6",
    "libdl.so.2",
    "libpthread.so.0",
    "librt.so.1",
    "libutil.so.1",
    "libgcc_s.so.1",
    "ld-linux-aarch64.so.1",
    "ld-linux-x86-64.so.2",
];

/// The graphics, input, audio, and font libraries the app cannot start
/// without, listed on every install. The binary does not always name them:
/// an x86-64 release build turns the short loader names (`libvulkan.so.1`)
/// into immediate values in the code, so no string is left to find, and
/// text layout reads fontconfig through a library it never names.
const BASELINE: &[&str] = &[
    "libvulkan.so.1",
    "libxkbcommon.so.0",
    "libxkbcommon-x11.so.0",
    "libasound.so.2",
    "libfontconfig.so.1",
    "libwayland-client.so.0",
    "libwayland-egl.so.1",
];

/// The Vulkan loader finds no device without a driver package. The binary
/// cannot name one, so it is part of the Vulkan line.
const DEB_VULKAN_DRIVER: &str = "mesa-vulkan-drivers | libgl1-mesa-dri";
const RPM_VULKAN_DRIVER: &str = "mesa-vulkan-drivers";

/// The Save dialog and the file picker go through the desktop portal.
const PORTAL: &[&str] = &["xdg-desktop-portal", "xdg-desktop-portal-gtk"];

#[derive(Debug, PartialEq, Eq)]
pub struct Deps {
    pub deb_depends: Vec<String>,
    pub deb_recommends: Vec<String>,
    pub rpm_requires: Vec<String>,
    pub rpm_recommends: Vec<String>,
}

fn find(soname: &str) -> Option<&'static Lib> {
    LIBS.iter().find(|lib| lib.soname == soname)
}

/// The known sonames the binary names: the libraries it opens at run time
/// with dlopen. Rust string literals sit next to each other without a
/// terminator, so each known name is searched as a plain substring.
pub fn named_sonames(bytes: &[u8]) -> BTreeSet<&'static str> {
    LIBS.iter()
        .map(|lib| lib.soname)
        .filter(|soname| bytes.windows(soname.len()).any(|w| w == soname.as_bytes()))
        .collect()
}

/// Sonames from `readelf -d` output.
pub fn parse_needed(readelf: &str) -> Vec<String> {
    readelf
        .lines()
        .filter(|line| line.contains("(NEEDED)"))
        .filter_map(|line| {
            let open = line.find('[')?;
            let close = line.rfind(']')?;
            Some(line[open + 1..close].to_string())
        })
        .collect()
}

pub fn needed_of(binary: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("readelf")
        .arg("-d")
        .arg(binary)
        .output()
        .map_err(|e| format!("cannot run readelf (install binutils): {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "readelf -d {} failed: {}",
            binary.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(parse_needed(&String::from_utf8_lossy(&output.stdout)))
}

/// Builds the package dependency lists from the linked and the named
/// libraries of one binary.
pub fn derive(needed: &[String], bytes: &[u8]) -> Result<Deps, String> {
    let named = named_sonames(bytes);
    let mut wanted: BTreeSet<&'static str> = BTreeSet::new();

    for soname in needed {
        if BASE.contains(&soname.as_str()) {
            continue;
        }
        let lib = find(soname).ok_or_else(|| {
            format!("the binary links {soname}, which has no deb and rpm package in xtask/src/package/deps.rs")
        })?;
        wanted.insert(lib.soname);
    }
    wanted.extend(named);
    wanted.extend(BASELINE);

    let mut deb_depends: Vec<String> = wanted
        .iter()
        .filter_map(|soname| find(soname).map(|lib| lib.deb.to_string()))
        .collect();
    deb_depends.push(DEB_VULKAN_DRIVER.to_string());
    let mut rpm_requires: Vec<String> = wanted
        .iter()
        .filter_map(|soname| find(soname).map(|lib| lib.rpm.to_string()))
        .collect();
    rpm_requires.push(RPM_VULKAN_DRIVER.to_string());
    for list in [&mut deb_depends, &mut rpm_requires] {
        list.sort();
        list.dedup();
    }

    Ok(Deps {
        deb_depends,
        deb_recommends: PORTAL.iter().map(|p| (*p).to_string()).collect(),
        rpm_requires,
        rpm_recommends: PORTAL.iter().map(|p| (*p).to_string()).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    const READELF: &str = " 0x0000000000000001 (NEEDED)             Shared library: [libxcb.so.1]\n 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]\n 0x000000000000000c (INIT)               0x1000\n";

    fn loaded_names() -> Vec<u8> {
        b"\0libvulkan.so.1libvulkan.so\0libxkbcommon.so.0\0libxkbcommon-x11.so.0\0libasound.so.2\0"
            .to_vec()
    }

    #[test]
    fn needed_entries_are_read_from_readelf_output() {
        assert_eq!(parse_needed(READELF), ["libxcb.so.1", "libc.so.6"]);
    }

    #[test]
    fn sonames_are_found_even_inside_a_longer_literal() {
        let found =
            named_sonames(b"x\0Unable to load libvulkan.so.1libwayland-client.so.0\0lib.so\0");
        assert!(found.contains("libvulkan.so.1"));
        assert!(found.contains("libwayland-client.so.0"));
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn fontconfig_is_a_baseline_dependency_the_binary_never_names() {
        assert!(!named_sonames(&loaded_names()).contains("libfontconfig.so.1"));
        let deps = derive(&[], &loaded_names()).unwrap();
        assert!(deps.deb_depends.iter().any(|d| d == "libfontconfig1"));
        assert!(deps.rpm_requires.iter().any(|d| d == "fontconfig"));
    }

    #[test]
    fn needed_libs_map_to_deb_and_rpm_packages() {
        let deps = derive(&strings(&["libxcb.so.1", "libc.so.6"]), &loaded_names()).unwrap();
        for deb in [
            "libvulkan1",
            "mesa-vulkan-drivers | libgl1-mesa-dri",
            "libxkbcommon0",
            "libxkbcommon-x11-0",
            "libasound2t64 | libasound2",
            "libfontconfig1",
            "libxcb1",
        ] {
            assert!(deps.deb_depends.iter().any(|d| d == deb), "deb {deb}");
        }
        for rpm in [
            "vulkan-loader",
            "mesa-vulkan-drivers",
            "libxkbcommon-x11",
            "alsa-lib",
            "fontconfig",
            "libxcb",
        ] {
            assert!(deps.rpm_requires.iter().any(|d| d == rpm), "rpm {rpm}");
        }
        assert!(
            deps.deb_recommends
                .iter()
                .any(|d| d == "xdg-desktop-portal")
        );
        assert!(
            deps.rpm_recommends
                .iter()
                .any(|d| d == "xdg-desktop-portal")
        );
    }

    #[test]
    fn base_libraries_need_no_package_line() {
        let deps = derive(
            &strings(&["libc.so.6", "libm.so.6", "libgcc_s.so.1"]),
            &loaded_names(),
        )
        .unwrap();
        assert!(!deps.deb_depends.iter().any(|d| d.contains("libc6")));
    }

    #[test]
    fn unknown_lib_fails_loudly() {
        let error = derive(&strings(&["libmystery.so.9"]), &loaded_names()).unwrap_err();
        assert!(error.contains("libmystery.so.9"), "{error}");
    }

    #[test]
    fn the_core_libraries_are_listed_when_the_binary_names_none() {
        let deps = derive(&[], b"nothing to see").unwrap();
        for deb in [
            "libvulkan1",
            "libxkbcommon0",
            "libxkbcommon-x11-0",
            "libasound2t64 | libasound2",
            "libwayland-client0",
            "libwayland-egl1",
        ] {
            assert!(deps.deb_depends.iter().any(|d| d == deb), "deb {deb}");
        }
    }

    #[test]
    fn lists_are_sorted_and_without_duplicates() {
        let deps = derive(&strings(&["libxkbcommon.so.0"]), &loaded_names()).unwrap();
        let mut sorted = deps.deb_depends.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(deps.deb_depends, sorted);
    }
}
