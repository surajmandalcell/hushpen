//! Targets, formats, and the artifact names. The name list is the one in the
//! mission's `library/release.md`; `latest.json` and the release workflow
//! read the same pattern.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    MacosArm64,
    LinuxX64,
    LinuxArm64,
}

impl Target {
    pub fn parse(text: &str) -> Result<Self, String> {
        match text {
            "macos-arm64" => Ok(Self::MacosArm64),
            "linux-x64" => Ok(Self::LinuxX64),
            "linux-arm64" => Ok(Self::LinuxArm64),
            other => Err(format!(
                "unknown target {other:?}; use macos-arm64, linux-x64, or linux-arm64"
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::MacosArm64 => "macos-arm64",
            Self::LinuxX64 => "linux-x64",
            Self::LinuxArm64 => "linux-arm64",
        }
    }

    pub fn is_linux(self) -> bool {
        !matches!(self, Self::MacosArm64)
    }

    /// The target that matches the machine xtask runs on.
    pub fn host() -> Result<Self, String> {
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("macos", "aarch64") => Ok(Self::MacosArm64),
            ("linux", "x86_64") => Ok(Self::LinuxX64),
            ("linux", "aarch64") => Ok(Self::LinuxArm64),
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
            _ => &[Format::AppImage, Format::Deb, Format::Rpm, Format::TarGz],
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
            other => Err(format!(
                "unknown format {other:?}; use app, zip, dmg, appimage, deb, rpm, or tar.gz"
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
        }
    }

    fn works_on(self, target: Target) -> bool {
        match self {
            Self::App | Self::Zip | Self::Dmg => !target.is_linux(),
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
    fn the_three_targets_plan_ten_artifacts() {
        let all: usize = [Target::MacosArm64, Target::LinuxX64, Target::LinuxArm64]
            .into_iter()
            .map(|target| planned_artifacts("2.0.0", target).len())
            .sum();
        assert_eq!(all, 10);
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
        assert!(Target::parse("windows-x64").is_err());
        assert_eq!(Format::parse("tar.gz"), Ok(Format::TarGz));
        assert!(Format::parse("msi").is_err());
    }
}
