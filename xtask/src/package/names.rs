//! Targets, formats, and the artifact names. The name list is the one in the
//! mission's `library/release.md`; `latest.json` and the release workflow
//! read the same pattern.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    MacosArm64,
    LinuxX64,
    LinuxArm64,
    WindowsX64,
}

impl Target {
    pub fn parse(text: &str) -> Result<Self, String> {
        match text {
            "macos-arm64" => Ok(Self::MacosArm64),
            "linux-x64" => Ok(Self::LinuxX64),
            "linux-arm64" => Ok(Self::LinuxArm64),
            "windows-x64" => Ok(Self::WindowsX64),
            other => Err(format!(
                "unknown target {other:?}; use macos-arm64, linux-x64, linux-arm64, or windows-x64"
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::MacosArm64 => "macos-arm64",
            Self::LinuxX64 => "linux-x64",
            Self::LinuxArm64 => "linux-arm64",
            Self::WindowsX64 => "windows-x64",
        }
    }

    pub fn is_linux(self) -> bool {
        matches!(self, Self::LinuxX64 | Self::LinuxArm64)
    }

    /// The target that matches the machine xtask runs on.
    pub fn host() -> Result<Self, String> {
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("macos", "aarch64") => Ok(Self::MacosArm64),
            ("linux", "x86_64") => Ok(Self::LinuxX64),
            ("linux", "aarch64") => Ok(Self::LinuxArm64),
            ("windows", "x86_64") => Ok(Self::WindowsX64),
            (os, arch) => Err(format!("no packaging target for {os} on {arch}")),
        }
    }

    pub fn deb_arch(self) -> &'static str {
        match self {
            Self::LinuxX64 => "amd64",
            _ => "arm64",
        }
    }

    pub fn rpm_arch(self) -> &'static str {
        match self {
            Self::LinuxX64 => "x86_64",
            _ => "aarch64",
        }
    }

    pub fn formats(self) -> &'static [Format] {
        match self {
            Self::MacosArm64 => &[Format::Zip, Format::Dmg],
            Self::LinuxX64 | Self::LinuxArm64 => {
                &[Format::AppImage, Format::Deb, Format::Rpm, Format::TarGz]
            }
            Self::WindowsX64 => &[Format::Nsis, Format::Zip],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// The `.app` bundle: an intermediate for the zip and the dmg, not shipped.
    App,
    Zip,
    Dmg,
    AppImage,
    Deb,
    Rpm,
    TarGz,
    Nsis,
}

impl Format {
    pub fn parse(text: &str) -> Result<Self, String> {
        match text {
            "app" => Ok(Self::App),
            "zip" => Ok(Self::Zip),
            "dmg" => Ok(Self::Dmg),
            "appimage" => Ok(Self::AppImage),
            "deb" => Ok(Self::Deb),
            "rpm" => Ok(Self::Rpm),
            "tar.gz" | "targz" => Ok(Self::TarGz),
            "nsis" => Ok(Self::Nsis),
            other => Err(format!(
                "unknown format {other:?}; use app, zip, dmg, appimage, deb, rpm, tar.gz, or nsis"
            )),
        }
    }

    pub fn extension(self) -> Option<&'static str> {
        match self {
            Self::App => None,
            Self::Zip => Some("zip"),
            Self::Dmg => Some("dmg"),
            Self::AppImage => Some("AppImage"),
            Self::Deb => Some("deb"),
            Self::Rpm => Some("rpm"),
            Self::TarGz => Some("tar.gz"),
            Self::Nsis => Some("exe"),
        }
    }

    fn works_on(self, target: Target) -> bool {
        match self {
            Self::App | Self::Dmg => target == Target::MacosArm64,
            Self::Zip => matches!(target, Target::MacosArm64 | Target::WindowsX64),
            Self::Nsis => target == Target::WindowsX64,
            _ => target.is_linux(),
        }
    }

    pub fn check_target(self, target: Target) -> Result<(), String> {
        if self.works_on(target) {
            Ok(())
        } else {
            Err(format!(
                "format {self:?} cannot be built for {}",
                target.name()
            ))
        }
    }
}

/// `Hushpen-<version>-<target>.<ext>`, or `None` for the `.app` intermediate.
pub fn artifact_name(version: &str, target: Target, format: Format) -> Option<String> {
    let ext = format.extension()?;
    if format == Format::Nsis {
        return Some(format!("Hushpen-{version}-{}-setup.exe", target.name()));
    }
    Some(format!("Hushpen-{version}-{}.{ext}", target.name()))
}

/// The folder a tar.gz unpacks to.
pub fn tar_top_folder(version: &str, target: Target) -> String {
    format!("hushpen-{version}-{}", target.name())
}

pub fn planned_artifacts(version: &str, target: Target) -> Vec<String> {
    target
        .formats()
        .iter()
        .filter_map(|format| artifact_name(version, target, *format))
        .collect()
}

/// The host archive that desktop SSH setup downloads.
pub fn host_artifact_name(version: &str, target: Target) -> Option<String> {
    target
        .is_linux()
        .then(|| format!("Hushpen-{version}-host-{}.tar.gz", target.name()))
}

pub fn android_artifact_name(version: &str) -> String {
    format!("Hushpen-{version}-android.apk")
}

pub fn device_artifacts(version: &str) -> Vec<String> {
    [Target::LinuxX64, Target::LinuxArm64]
        .into_iter()
        .filter_map(|target| host_artifact_name(version, target))
        .chain(std::iter::once(android_artifact_name(version)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_names_match_release_md() {
        assert_eq!(
            planned_artifacts("2.0.0", Target::MacosArm64),
            [
                "Hushpen-2.0.0-macos-arm64.zip",
                "Hushpen-2.0.0-macos-arm64.dmg"
            ]
        );
        for (target, label) in [
            (Target::LinuxX64, "linux-x64"),
            (Target::LinuxArm64, "linux-arm64"),
        ] {
            assert_eq!(
                planned_artifacts("2.0.0", target),
                [
                    format!("Hushpen-2.0.0-{label}.AppImage"),
                    format!("Hushpen-2.0.0-{label}.deb"),
                    format!("Hushpen-2.0.0-{label}.rpm"),
                    format!("Hushpen-2.0.0-{label}.tar.gz"),
                ]
            );
        }
    }

    #[test]
    fn the_four_targets_plan_twelve_artifacts() {
        let all: usize = [
            Target::MacosArm64,
            Target::LinuxX64,
            Target::LinuxArm64,
            Target::WindowsX64,
        ]
        .into_iter()
        .map(|target| planned_artifacts("2.0.0", target).len())
        .sum();
        assert_eq!(all, 12);
    }

    #[test]
    fn the_app_bundle_has_no_artifact_name() {
        assert_eq!(
            artifact_name("2.0.0", Target::MacosArm64, Format::App),
            None
        );
    }

    #[test]
    fn a_format_is_refused_on_the_wrong_os() {
        assert!(Format::Deb.check_target(Target::MacosArm64).is_err());
        assert!(Format::Dmg.check_target(Target::LinuxX64).is_err());
        assert!(Format::Dmg.check_target(Target::MacosArm64).is_ok());
        assert!(Format::TarGz.check_target(Target::LinuxArm64).is_ok());
    }

    #[test]
    fn rc_versions_flow_into_the_names() {
        assert_eq!(
            artifact_name("2.0.0-rc.1", Target::LinuxX64, Format::Deb).as_deref(),
            Some("Hushpen-2.0.0-rc.1-linux-x64.deb")
        );
        assert_eq!(
            tar_top_folder("2.0.0-rc.1", Target::LinuxX64),
            "hushpen-2.0.0-rc.1-linux-x64"
        );
    }

    #[test]
    fn parse_round_trips_and_refuses_unknown_words() {
        assert_eq!(Target::parse("linux-x64"), Ok(Target::LinuxX64));
        assert_eq!(Target::parse("windows-x64"), Ok(Target::WindowsX64));
        assert!(Target::parse("unknown").is_err());
        assert_eq!(Format::parse("tar.gz"), Ok(Format::TarGz));
        assert!(Format::parse("msi").is_err());
    }

    #[test]
    fn windows_plans_a_setup_and_portable_zip() {
        assert!(!Target::WindowsX64.is_linux());
        assert_eq!(
            planned_artifacts("2.4.0", Target::WindowsX64),
            [
                "Hushpen-2.4.0-windows-x64-setup.exe",
                "Hushpen-2.4.0-windows-x64.zip",
            ]
        );
        for format in [
            Format::App,
            Format::Dmg,
            Format::AppImage,
            Format::Deb,
            Format::Rpm,
            Format::TarGz,
        ] {
            assert!(format.check_target(Target::WindowsX64).is_err());
        }
    }

    #[test]
    fn device_artifacts_match_the_ssh_and_android_names() {
        assert_eq!(
            device_artifacts("2.4.0-rc.1"),
            [
                "Hushpen-2.4.0-rc.1-host-linux-x64.tar.gz",
                "Hushpen-2.4.0-rc.1-host-linux-arm64.tar.gz",
                "Hushpen-2.4.0-rc.1-android.apk",
            ]
        );
        assert!(host_artifact_name("2.4.0", Target::MacosArm64).is_none());
        assert!(host_artifact_name("2.4.0", Target::WindowsX64).is_none());
    }
}
