//! The update check: one signed `latest.json` and its `.minisig`, and nothing else.
//!
//! Each request is a plain GET with no query string, no cookie, and no header that names this
//! machine or this install. The manifest counts only after its signature matches the public
//! key that ships inside the app (`assets/update-key.pub`).

use super::fetch::{FetchError, Fetcher, UreqFetcher};
use super::policy::{Blocked, MAX_REDIRECTS, UpdatePolicy, resolve};
use super::{Purpose, record};
use hushpen_core::error::{UPDATE_CHECK_FAILED, UPDATE_SIGNATURE_INVALID};
use hushpen_core::update::{Offer, Version, offer, parse_manifest};
use minisign_verify::{PublicKey, Signature};
use std::io::Read;

/// The release feed. GitHub answers it with a redirect to the newest release asset.
pub const FEED_URL: &str =
    "https://github.com/surajmandalcell/asrpro/releases/latest/download/latest.json";

const SIGNATURE_SUFFIX: &str = ".minisig";
const EMBEDDED_KEY: &str = include_str!("../../../../assets/update-key.pub");

const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
const MAX_SIGNATURE_BYTES: u64 = 4 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckError {
    pub code: &'static str,
    /// Names a host or a field. It never holds a URL query or a manifest value.
    pub detail: String,
}

impl CheckError {
    fn failed(detail: impl Into<String>) -> Self {
        Self {
            code: UPDATE_CHECK_FAILED,
            detail: detail.into(),
        }
    }

    fn signature(detail: impl Into<String>) -> Self {
        Self {
            code: UPDATE_SIGNATURE_INVALID,
            detail: detail.into(),
        }
    }

    fn blocked(blocked: &Blocked) -> Self {
        record(Purpose::UpdateCheck, &blocked.host, "blocked");
        Self::failed(blocked.reason.clone())
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
                .map_err(|_| CheckError::failed("the built-in update key is not valid"))?,
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
                .map_err(|_| CheckError::failed("the test update key is not valid"))?
            }
            _ => return Err(CheckError::failed("a test feed needs a test update key")),
        };
        Self::with(url.trim(), key).map(Some)
    }

    #[cfg(any(test, feature = "update-test-feed"))]
    fn with(manifest_url: &str, key: PublicKey) -> Result<Self, CheckError> {
        let policy =
            UpdatePolicy::only(manifest_url).map_err(|blocked| CheckError::blocked(&blocked))?;
        Ok(Self {
            manifest_url: manifest_url.to_owned(),
            key,
            policy,
        })
    }

    pub fn fetcher(&self) -> UreqFetcher {
        UreqFetcher::with_https_only(self.policy.https_only())
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
            .ok_or_else(|| CheckError::signature("the signature file is not a signature"))?;
        // A legacy signature signs the bytes themselves, which is what the release signer writes.
        self.key
            .verify(&manifest, &signature, true)
            .map_err(|_| CheckError::signature("the signature does not match the update key"))?;
        let manifest = parse_manifest(&manifest).map_err(|error| {
            CheckError::failed(format!("the update manifest is not valid: {error:?}"))
        })?;
        Ok(offer(current, manifest))
    }

    /// Follows redirects by hand, so every hop passes the host policy before a connection opens.
    fn get(&self, fetcher: &dyn Fetcher, url: &str, limit: u64) -> Result<Vec<u8>, CheckError> {
        let mut current = self
            .policy
            .check_start(url)
            .map_err(|blocked| CheckError::blocked(&blocked))?;
        let mut hops = 0;
        loop {
            let response = match fetcher.get(&current.url, None) {
                Ok(response) => response,
                Err(FetchError(error)) => {
                    record(Purpose::UpdateCheck, &current.host, "error");
                    return Err(CheckError::failed(format!(
                        "cannot reach {}: {error}",
                        current.host
                    )));
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
                            CheckError::failed(format!("{} stopped sending: {error}", current.host))
                        })?;
                    if body.len() as u64 > limit {
                        return Err(CheckError::failed(format!(
                            "{} sent more than it should",
                            current.host
                        )));
                    }
                    return Ok(body);
                }
                301 | 302 | 303 | 307 | 308 => {
                    hops += 1;
                    if hops > MAX_REDIRECTS {
                        return Err(CheckError::blocked(&Blocked::new(
                            current.host,
                            format!("more than {MAX_REDIRECTS} redirects"),
                        )));
                    }
                    let target = response
                        .location
                        .as_deref()
                        .and_then(|location| resolve(&current.url, location))
                        .ok_or_else(|| CheckError::failed("a redirect had no usable location"))?;
                    current = self
                        .policy
                        .check_redirect(&target)
                        .map_err(|blocked| CheckError::blocked(&blocked))?;
                }
                status => {
                    return Err(CheckError::failed(format!(
                        "{} answered {status}",
                        current.host
                    )));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
