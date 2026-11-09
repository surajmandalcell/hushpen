//! The pure part of update checks: versions, the manifest `latest.json`, and the 24 hour rule.
//!
//! The signature check and the network calls live in `hushpen-app/src/net/`. Nothing here
//! reads a clock, a file, or a socket.

use serde_json::Value;
use std::cmp::Ordering;
use std::collections::BTreeMap;

/// An automatic check runs at most this often.
pub const CHECK_INTERVAL_MS: u64 = 24 * 60 * 60 * 1000;

/// Whether an automatic check may run. `last_check_ms` is the stored time of the last check in
/// milliseconds since 1970, or 0 when there was none. A clock that went back before that time
/// counts as due, so a wrong date never blocks checks for days.
pub fn check_due(last_check_ms: u64, now_ms: u64) -> bool {
    last_check_ms == 0 || now_ms < last_check_ms || now_ms - last_check_ms >= CHECK_INTERVAL_MS
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Number(u64),
    Text(String),
}

impl Part {
    fn parse(text: &str) -> Option<Self> {
        if text.is_empty() || !text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return None;
        }
        Some(match text.parse::<u64>() {
            Ok(number) if text.bytes().all(|b| b.is_ascii_digit()) => Part::Number(number),
            _ => Part::Text(text.to_owned()),
        })
    }
}

impl Ord for Part {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Part::Number(a), Part::Number(b)) => a.cmp(b),
            (Part::Text(a), Part::Text(b)) => a.cmp(b),
            (Part::Number(_), Part::Text(_)) => Ordering::Less,
            (Part::Text(_), Part::Number(_)) => Ordering::Greater,
        }
    }
}

impl PartialOrd for Part {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A semantic version such as `2.0.0`, `2.0.0-rc.1`, or `2.0.0-dev`. A pre-release sorts before
/// its release, and the text after `+` is ignored.
#[derive(Debug, Clone)]
pub struct Version {
    numbers: [u64; 3],
    pre: Vec<Part>,
    text: String,
}

const MAX_VERSION_LEN: usize = 64;

impl Version {
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let text = text.strip_prefix('v').unwrap_or(text);
        if text.is_empty() || text.len() > MAX_VERSION_LEN {
            return None;
        }
        let without_build = text.split('+').next().unwrap_or("");
        let (core, pre) = match without_build.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (without_build, None),
        };
        let mut numbers = [0u64; 3];
        let mut parts = core.split('.');
        for slot in &mut numbers {
            let part = parts.next()?;
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            *slot = part.parse().ok()?;
        }
        if parts.next().is_some() {
            return None;
        }
        let pre = match pre {
            Some(pre) => pre
                .split('.')
                .map(Part::parse)
                .collect::<Option<Vec<_>>>()?,
            None => Vec::new(),
        };
        Some(Self {
            numbers,
            pre,
            text: text.to_owned(),
        })
    }

    /// The version as it was written, without a leading `v`.
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.numbers.cmp(&other.numbers).then_with(|| {
            match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => self.pre.cmp(&other.pre),
            }
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Version {}

/// One downloadable file of a release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    pub url: String,
    /// Lower-case hex.
    pub sha256: String,
    pub size: u64,
}

/// The signed `latest.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub version: Version,
    pub notes_url: String,
    pub pub_date: Option<String>,
    /// Keyed by target, for example `macos-arm64`, `linux-x64`, `windows-x64`.
    pub platforms: BTreeMap<String, Artifact>,
}

/// Why a manifest was refused. The text names a field and never repeats its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestError {
    NotJson,
    Missing(&'static str),
    Invalid(&'static str),
}

const MAX_URL_LEN: usize = 2048;

fn web_url(text: &str) -> bool {
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"));
    rest.is_some_and(|rest| !rest.is_empty())
        && text.len() <= MAX_URL_LEN
        && text.is_ascii()
        && !text.bytes().any(|b| b.is_ascii_control() || b == b' ')
}

fn string_field<'a>(
    object: &'a serde_json::Map<String, Value>,
    name: &'static str,
) -> Result<&'a str, ManifestError> {
    match object.get(name) {
        None => Err(ManifestError::Missing(name)),
        Some(Value::String(text)) => Ok(text),
        Some(_) => Err(ManifestError::Invalid(name)),
    }
}

/// Reads `latest.json`. Call it only after the signature of `bytes` was checked.
pub fn parse_manifest(bytes: &[u8]) -> Result<Manifest, ManifestError> {
    let Ok(Value::Object(object)) = serde_json::from_slice::<Value>(bytes) else {
        return Err(ManifestError::NotJson);
    };
    let version = Version::parse(string_field(&object, "version")?)
        .ok_or(ManifestError::Invalid("version"))?;
    let notes_url = string_field(&object, "notes_url")?;
    if !web_url(notes_url) {
        return Err(ManifestError::Invalid("notes_url"));
    }
    let pub_date = match object.get("pub_date") {
        None | Some(Value::Null) => None,
        Some(Value::String(date)) if date.len() <= 64 => Some(date.clone()),
        Some(_) => return Err(ManifestError::Invalid("pub_date")),
    };
    let mut platforms = BTreeMap::new();
    match object.get("platforms") {
        None | Some(Value::Null) => {}
        Some(Value::Object(targets)) => {
            for (target, entry) in targets {
                let Value::Object(entry) = entry else {
                    return Err(ManifestError::Invalid("platforms"));
                };
                let url =
                    string_field(entry, "url").map_err(|_| ManifestError::Invalid("platforms"))?;
                let sha256 = string_field(entry, "sha256")
                    .map_err(|_| ManifestError::Invalid("platforms"))?
                    .to_ascii_lowercase();
                let size = entry
                    .get("size")
                    .and_then(Value::as_u64)
                    .ok_or(ManifestError::Invalid("platforms"))?;
                if !web_url(url)
                    || sha256.len() != 64
                    || !sha256.bytes().all(|b| b.is_ascii_hexdigit())
                {
                    return Err(ManifestError::Invalid("platforms"));
                }
                platforms.insert(
                    target.clone(),
                    Artifact {
                        url: url.to_owned(),
                        sha256,
                        size,
                    },
                );
            }
        }
        Some(_) => return Err(ManifestError::Invalid("platforms")),
    }
    Ok(Manifest {
        version,
        notes_url: notes_url.to_owned(),
        pub_date,
        platforms,
    })
}

/// What a manifest means for the running version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Offer {
    /// The manifest is older than or equal to the running version.
    UpToDate,
    Available(Manifest),
}

impl Offer {
    pub fn is_available(&self) -> bool {
        matches!(self, Offer::Available(_))
    }
}

pub fn offer(current: &Version, manifest: Manifest) -> Offer {
    if manifest.version > *current {
        Offer::Available(manifest)
    } else {
        Offer::UpToDate
    }
}

/// How this copy of the app was installed. Every package carries an `install-kind` file with
/// one of these words; a build that runs from a source tree has none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallKind {
    /// The macOS `.app` from the zip or the dmg.
    App,
    AppImage,
    Deb,
    Rpm,
    /// The Linux tar.gz.
    Tar,
    WindowsSetup,
    WindowsZip,
    /// No `install-kind` file: a build that runs from a source tree.
    Unknown,
}

impl InstallKind {
    /// Reads the text of an `install-kind` file. Anything else is [`InstallKind::Unknown`].
    pub fn parse(text: &str) -> Self {
        match text.trim() {
            "app" => Self::App,
            "appimage" => Self::AppImage,
            "deb" => Self::Deb,
            "rpm" => Self::Rpm,
            "tar" => Self::Tar,
            "windows-setup" => Self::WindowsSetup,
            "windows-zip" => Self::WindowsZip,
            _ => Self::Unknown,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::AppImage => "appimage",
            Self::Deb => "deb",
            Self::Rpm => "rpm",
            Self::Tar => "tar",
            Self::WindowsSetup => "windows-setup",
            Self::WindowsZip => "windows-zip",
            Self::Unknown => "unknown",
        }
    }
}

/// The manifest key of an operating system and CPU.
pub fn target_key(os: crate::os::Os, arch: &str) -> Option<&'static str> {
    use crate::os::Os;
    match (os, arch) {
        (Os::Mac, "aarch64") => Some("macos-arm64"),
        (Os::Linux, "x86_64") => Some("linux-x64"),
        (Os::Linux, "aarch64") => Some("linux-arm64"),
        (Os::Windows, "x86_64") => Some("windows-x64"),
        _ => None,
    }
}

/// The manifest key of the running build.
pub fn current_target() -> Option<&'static str> {
    target_key(crate::os::Os::current(), std::env::consts::ARCH)
}

/// What the app does with an offered version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delivery {
    /// Download this file, check its sha256, stage it beside the install, and swap it in.
    Artifact(Artifact),
    /// A package manager or the user owns this install: show the release link and nothing else.
    Notice,
    /// A build that runs from a source tree. It offers the install like the others, and
    /// refuses it because there is nothing to replace.
    Unpackaged,
}

pub fn delivery(kind: InstallKind, target: Option<&str>, manifest: &Manifest) -> Delivery {
    match kind {
        InstallKind::Deb | InstallKind::Rpm | InstallKind::Tar | InstallKind::WindowsZip => {
            Delivery::Notice
        }
        InstallKind::Unknown => Delivery::Unpackaged,
        InstallKind::App | InstallKind::AppImage | InstallKind::WindowsSetup => target
            .and_then(|target| manifest.platforms.get(target))
            .map_or(Delivery::Notice, |artifact| {
                Delivery::Artifact(artifact.clone())
            }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckError {
    pub code: &'static str,
    /// Names a host or a field, without private request values.
    pub detail: String,
}

impl CheckError {
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum State {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available(Manifest),
    Downloading(Manifest, Option<u8>),
    Ready(Manifest),
    Error(CheckError),
}

impl State {
    pub fn key(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Checking => "checking",
            Self::UpToDate => "up_to_date",
            Self::Available(_) => "available",
            Self::Downloading(..) => "downloading",
            Self::Ready(_) => "ready",
            Self::Error(_) => "error",
        }
    }

    pub fn manifest(&self) -> Option<&Manifest> {
        match self {
            Self::Available(manifest) | Self::Downloading(manifest, _) | Self::Ready(manifest) => {
                Some(manifest)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerKind {
    Install,
    Notice,
    Downloading(Option<u8>),
    Restart,
}

#[derive(Debug, Default)]
pub struct Machine {
    state: State,
    runs: u64,
    restart_dismissed: bool,
}

impl Machine {
    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn can_check(&self) -> bool {
        !matches!(
            self.state,
            State::Checking | State::Downloading(..) | State::Ready(_)
        )
    }

    pub fn check_due(&self, checks_on: bool, last: u64, now: u64) -> bool {
        checks_on && check_due(last, now) && self.can_check()
    }

    pub fn begin_check(&mut self) -> Option<u64> {
        if !self.can_check() {
            return None;
        }
        self.runs += 1;
        self.state = State::Checking;
        Some(self.runs)
    }

    pub fn checks_changed(&mut self, checks_on: bool) {
        if !checks_on && self.state == State::Checking {
            self.runs += 1;
            self.state = State::Idle;
        }
    }

    /// Returns false for an answer from a cancelled check.
    pub fn finish_check(&mut self, run: u64, result: Result<Offer, CheckError>) -> bool {
        if run != self.runs {
            return false;
        }
        self.state = match result {
            Ok(Offer::UpToDate) => State::UpToDate,
            Ok(Offer::Available(manifest)) => State::Available(manifest),
            Err(error) => State::Error(error),
        };
        true
    }

    pub fn should_install(&self, auto_install: bool) -> bool {
        auto_install && matches!(self.state, State::Available(_))
    }

    /// A notice never starts an install; a source build records the request only.
    pub fn install(&mut self, kind: InstallKind, target: Option<&str>) -> Option<Delivery> {
        let State::Available(manifest) = &self.state else {
            return None;
        };
        let result = delivery(kind, target, manifest);
        match &result {
            Delivery::Notice => return None,
            Delivery::Artifact(_) => self.state = State::Downloading(manifest.clone(), None),
            Delivery::Unpackaged => {}
        }
        Some(result)
    }

    pub fn downloading(&mut self, percent: Option<u8>) -> bool {
        let State::Downloading(manifest, _) = &self.state else {
            return false;
        };
        self.state = State::Downloading(manifest.clone(), percent);
        true
    }

    pub fn ready(&mut self) -> bool {
        let State::Downloading(manifest, _) = &self.state else {
            return false;
        };
        self.state = State::Ready(manifest.clone());
        self.restart_dismissed = false;
        true
    }

    pub fn finish_stage(&mut self, result: Result<(), CheckError>) -> bool {
        if !matches!(self.state, State::Downloading(..)) {
            return false;
        }
        match result {
            Ok(()) => {
                self.ready();
            }
            Err(error) => self.failed(error),
        }
        true
    }

    pub fn can_restart(&self) -> bool {
        matches!(self.state, State::Ready(_))
    }

    pub fn failed(&mut self, error: CheckError) {
        self.state = State::Error(error);
    }

    /// Returns a version to persist, or hides this session's restart banner.
    pub fn dismiss(&mut self) -> Option<String> {
        let manifest = self.state.manifest()?;
        if matches!(self.state, State::Ready(_)) {
            self.restart_dismissed = true;
            None
        } else {
            Some(manifest.version.as_str().to_owned())
        }
    }

    pub fn banner_kind(
        &self,
        kind: InstallKind,
        target: Option<&str>,
        auto_install: bool,
        dismissed: Option<&str>,
    ) -> Option<BannerKind> {
        match &self.state {
            State::Available(manifest) => {
                if dismissed == Some(manifest.version.as_str()) {
                    return None;
                }
                match delivery(kind, target, manifest) {
                    Delivery::Notice => Some(BannerKind::Notice),
                    _ => (!auto_install).then_some(BannerKind::Install),
                }
            }
            State::Downloading(_, percent) => {
                (!auto_install).then_some(BannerKind::Downloading(*percent))
            }
            State::Ready(_) => (!self.restart_dismissed).then_some(BannerKind::Restart),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
