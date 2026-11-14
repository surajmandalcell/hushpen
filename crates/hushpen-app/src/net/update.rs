//! The update check: one signed `latest.json` and its `.minisig`, plus one anonymous count.
//!
//! Each manifest request is a plain GET with no query string, no cookie, and no header that
//! names this machine or this install. The manifest counts only after its signature matches the
//! public key that ships inside the app (`assets/update-key.pub`). The count ([`UpdateFeed::count`])
//! sends the app name, platform, version, and day or month flags to the shared update API, and
//! nothing that identifies the person or the install (D-14).

use super::download::{Downloader, Outcome, Progress, Spec};
use super::fetch::{FetchError, Fetcher, UreqFetcher};
use super::policy::{Blocked, MAX_REDIRECTS, UpdatePolicy, resolve};
use super::{Purpose, record};
use hushpen_core::error::{UPDATE_CHECK_FAILED, UPDATE_SIGNATURE_INVALID};
use hushpen_core::update::{Artifact, Offer, Version, offer, parse_manifest};
use minisign_verify::{PublicKey, Signature};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// The release feed. GitHub answers it with a redirect to the newest release asset.
pub const FEED_URL: &str =
    "https://github.com/surajmandalcell/hushpen/releases/latest/download/latest.json";

/// The shared update API of the apps. The only file that names this host is this one (net-audit).
const COUNT_URL: &str = "https://apps.mandalsuraj.com/v1/hushpen/latest";
const COUNT_TIMEOUT: Duration = Duration::from_secs(8);

const SIGNATURE_SUFFIX: &str = ".minisig";
const EMBEDDED_KEY: &str = include_str!("../../../../assets/update-key.pub");

const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
const MAX_SIGNATURE_BYTES: u64 = 4 * 1024;

pub use hushpen_core::update::CheckError;

fn failed(detail: impl Into<String>) -> CheckError {
    CheckError::new(UPDATE_CHECK_FAILED, detail)
}

fn signature_error(detail: impl Into<String>) -> CheckError {
    CheckError::new(UPDATE_SIGNATURE_INVALID, detail)
}

fn blocked_error(blocked: &Blocked) -> CheckError {
    record(Purpose::UpdateCheck, &blocked.host, "blocked");
    failed(blocked.reason.clone())
}

/// One GET with an 8 second limit, no redirect, no cookie. Silent on any error.
fn send_count(url: &str) -> bool {
    let config = ureq::Agent::config_builder()
        .max_redirects(0)
        .http_status_as_error(false)
        .https_only(true)
        .timeout_global(Some(COUNT_TIMEOUT))
        .user_agent(format!("Hushpen/{}", hushpen_core::BUILD_VERSION))
        .build();
    // Purpose::UpdateCheck. Host: apps.mandalsuraj.com.
    match ureq::Agent::new_with_config(config).get(url).call() {
        Ok(response) => {
            let status = response.status().as_u16();
            record(
                Purpose::UpdateCheck,
                "apps.mandalsuraj.com",
                &status.to_string(),
            );
            response.status().is_success()
        }
        Err(_) => {
            record(Purpose::UpdateCheck, "apps.mandalsuraj.com", "error");
            false
        }
    }
}

/// Where the manifest is, which key signs it, and which hosts may be asked.
pub struct UpdateFeed {
    manifest_url: String,
    key: PublicKey,
    policy: UpdatePolicy,
}

impl UpdateFeed {
    /// The feed of this build. A shipped build always uses the release feed and the embedded key.
    pub fn current() -> Result<Self, CheckError> {
        #[cfg(feature = "update-test-feed")]
        if let Some(feed) = Self::from_environment()? {
            return Ok(feed);
        }
        Ok(Self {
            manifest_url: FEED_URL.to_owned(),
            key: PublicKey::decode(EMBEDDED_KEY)
                .map_err(|_| failed("the built-in update key is not valid"))?,
            policy: UpdatePolicy::release(),
        })
    }

    /// Test builds only: `HUSHPEN_UPDATE_FEED` is the manifest URL and `HUSHPEN_UPDATE_PUBKEY`
    /// is a public key file or the base64 key itself.
    #[cfg(feature = "update-test-feed")]
    fn from_environment() -> Result<Option<Self>, CheckError> {
        let Some(url) = std::env::var("HUSHPEN_UPDATE_FEED")
            .ok()
            .filter(|url| !url.trim().is_empty())
        else {
            return Ok(None);
        };
        let key = match std::env::var("HUSHPEN_UPDATE_PUBKEY") {
            Ok(value) if !value.trim().is_empty() => {
                let value = value.trim();
                if std::path::Path::new(value).is_file() {
                    PublicKey::from_file(value)
                } else {
                    PublicKey::from_base64(value)
                }
                .map_err(|_| failed("the test update key is not valid"))?
            }
            _ => return Err(failed("a test feed needs a test update key")),
        };
        Self::with(url.trim(), key).map(Some)
    }

    #[cfg(any(test, feature = "update-test-feed"))]
    fn with(manifest_url: &str, key: PublicKey) -> Result<Self, CheckError> {
        let policy = UpdatePolicy::only(manifest_url).map_err(|blocked| blocked_error(&blocked))?;
        Ok(Self {
            manifest_url: manifest_url.to_owned(),
            key,
            policy,
        })
    }

    /// The anonymous active-user count: one GET to the shared update API with the app, platform,
    /// version, and day or month flags of `hushpen_core::update::Count::query`. It is not an update
    /// source: nothing in the answer is read, and the signed manifest stays the only thing that
    /// offers or verifies a download. True when the server answered 2xx. A test feed never counts.
    pub fn count(&self, query: &str) -> bool {
        self.manifest_url == FEED_URL && send_count(&format!("{COUNT_URL}?{query}"))
    }

    pub fn fetcher(&self) -> UreqFetcher {
        UreqFetcher::with_https_only(self.policy.https_only())
    }

    /// Downloads the release file of the signed manifest into `dest` through `partial`, and
    /// checks its size and sha256 before the rename. The same hosts as the check are allowed.
    pub fn download(
        &self,
        artifact: &Artifact,
        partial: &Path,
        dest: &Path,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<Outcome, CheckError> {
        self.download_with(
            Box::new(self.fetcher()),
            artifact,
            partial,
            dest,
            cancel,
            progress,
        )
    }

    fn download_with(
        &self,
        fetcher: Box<dyn Fetcher>,
        artifact: &Artifact,
        partial: &Path,
        dest: &Path,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<Outcome, CheckError> {
        let spec = Spec {
            url: artifact.url.clone(),
            bytes: artifact.size,
            sha256: artifact.sha256.clone(),
            partial: partial.to_path_buf(),
            dest: dest.to_path_buf(),
        };
        Downloader::for_update(self.policy.clone(), fetcher)
            .with_observer(super::record)
            .run(&spec, cancel, progress)
            .map_err(|failure| CheckError::new(failure.code, failure.detail))
    }

    /// Asks the feed and compares with `current`. At most two requests, plus their redirects.
    pub fn check(&self, fetcher: &dyn Fetcher, current: &Version) -> Result<Offer, CheckError> {
        let manifest = self.get(fetcher, &self.manifest_url, MAX_MANIFEST_BYTES)?;
        let signature = self.get(
            fetcher,
            &format!("{}{SIGNATURE_SUFFIX}", self.manifest_url),
            MAX_SIGNATURE_BYTES,
        )?;
        let signature = std::str::from_utf8(&signature)
            .ok()
            .and_then(|text| Signature::decode(text).ok())
            .ok_or_else(|| signature_error("the signature file is not a signature"))?;
        // A legacy signature signs the bytes themselves, which is what the release signer writes.
        self.key
            .verify(&manifest, &signature, true)
            .map_err(|_| signature_error("the signature does not match the update key"))?;
        let manifest = parse_manifest(&manifest)
            .map_err(|error| failed(format!("the update manifest is not valid: {error:?}")))?;
        Ok(offer(current, manifest))
    }

    /// Follows redirects by hand, so every hop passes the host policy before a connection opens.
    fn get(&self, fetcher: &dyn Fetcher, url: &str, limit: u64) -> Result<Vec<u8>, CheckError> {
        let mut current = self
            .policy
            .check_start(url)
            .map_err(|blocked| blocked_error(&blocked))?;
        let mut hops = 0;
        loop {
            let response = match fetcher.get(&current.url, None) {
                Ok(response) => response,
                Err(FetchError(error)) => {
                    record(Purpose::UpdateCheck, &current.host, "error");
                    return Err(failed(format!("cannot reach {}: {error}", current.host)));
                }
            };
            record(
                Purpose::UpdateCheck,
                &current.host,
                &response.status.to_string(),
            );
            match response.status {
                200 => {
                    let mut body = Vec::new();
                    response
                        .body
                        .take(limit + 1)
                        .read_to_end(&mut body)
                        .map_err(|error| {
                            failed(format!("{} stopped sending: {error}", current.host))
                        })?;
                    if body.len() as u64 > limit {
                        return Err(failed(format!("{} sent more than it should", current.host)));
                    }
                    return Ok(body);
                }
                301 | 302 | 303 | 307 | 308 => {
                    hops += 1;
                    if hops > MAX_REDIRECTS {
                        return Err(blocked_error(&Blocked::new(
                            current.host,
                            format!("more than {MAX_REDIRECTS} redirects"),
                        )));
                    }
                    let target = response
                        .location
                        .as_deref()
                        .and_then(|location| resolve(&current.url, location))
                        .ok_or_else(|| failed("a redirect had no usable location"))?;
                    current = self
                        .policy
                        .check_redirect(&target)
                        .map_err(|blocked| blocked_error(&blocked))?;
                }
                status => {
                    return Err(failed(format!("{} answered {status}", current.host)));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
