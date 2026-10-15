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
    /// Keyed by target, for example `macos-arm64`, `linux-x64`, `linux-arm64`.
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
            Self::Unknown => "unknown",
        }
    }
}

/// The manifest key of an operating system and CPU, as `std::env::consts` names them.
pub fn target_key(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("macos", "aarch64") => Some("macos-arm64"),
        ("linux", "x86_64") => Some("linux-x64"),
        ("linux", "aarch64") => Some("linux-arm64"),
        _ => None,
    }
}

/// The manifest key of the running build.
pub fn current_target() -> Option<&'static str> {
    target_key(std::env::consts::OS, std::env::consts::ARCH)
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
        InstallKind::Deb | InstallKind::Rpm | InstallKind::Tar => Delivery::Notice,
        InstallKind::Unknown => Delivery::Unpackaged,
        InstallKind::App | InstallKind::AppImage => target
            .and_then(|target| manifest.platforms.get(target))
            .map_or(Delivery::Notice, |artifact| {
                Delivery::Artifact(artifact.clone())
            }),
    }
}

#[cfg(test)]
mod tests;
