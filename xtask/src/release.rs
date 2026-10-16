//! The release assets: `cargo xtask release-manifest` writes `SHA256SUMS` and the signed
//! `latest.json` next to the packaged artifacts, and `cargo xtask release-verify` checks a
//! downloaded copy of the draft release against the same plan.
//!
//! The secret key comes from the environment (`HUSHPEN_UPDATE_KEY`, plus
//! `HUSHPEN_UPDATE_KEY_PASSWORD` when the key has a password), never from a file or an argument,
//! and no message here repeats a key or a signature.

use std::collections::BTreeSet;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use minisign::{PublicKeyBox, SecretKeyBox, SignatureBox};
use serde_json::Value;

use crate::package::names::{Format, Target, artifact_name, planned_artifacts};
use crate::package::sha256_hex;
use crate::update_feed::{Platform, manifest_text_at};

const MANIFEST: &str = "latest.json";
const SIGNATURE: &str = "latest.json.minisig";
const SUMS: &str = "SHA256SUMS";
const KEY_VAR: &str = "HUSHPEN_UPDATE_KEY";
const PASSWORD_VAR: &str = "HUSHPEN_UPDATE_KEY_PASSWORD";

const TARGETS: [Target; 3] = [Target::MacosArm64, Target::LinuxX64, Target::LinuxArm64];

/// The format each target updates itself from.
fn update_format(target: Target) -> Format {
    match target {
        Target::MacosArm64 => Format::Zip,
        _ => Format::AppImage,
    }
}

/// `v2.0.0` or `v2.0.0-rc.1` gives `2.0.0` or `2.0.0-rc.1`.
pub fn version_of_tag(tag: &str) -> Result<String, String> {
    let bad = || format!("tag {tag:?} is not vMAJOR.MINOR.PATCH or vMAJOR.MINOR.PATCH-rc.N");
    let version = tag.strip_prefix('v').ok_or_else(bad)?;
    let (core, rc) = match version.split_once("-rc.") {
        Some((core, rc)) => (core, Some(rc)),
        None => (version, None),
    };
    let number = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 || !parts.iter().all(|part| number(part)) || rc.is_some_and(|r| !number(r))
    {
        return Err(bad());
    }
    Ok(version.to_owned())
}

/// The ten installers of the release, in plan order.
pub fn installers(version: &str) -> Vec<String> {
    TARGETS
        .iter()
        .flat_map(|target| planned_artifacts(version, *target))
        .collect()
}

/// Everything the release holds: the installers, `latest.json`, its signature, and `SHA256SUMS`.
pub fn release_assets(version: &str) -> BTreeSet<String> {
    let mut assets: BTreeSet<String> = installers(version).into_iter().collect();
    assets.extend([MANIFEST, SIGNATURE, SUMS].map(str::to_owned));
    assets
}

fn download_url(repo: &str, tag: &str, name: &str) -> String {
    format!("https://github.com/{repo}/releases/download/{tag}/{name}")
}

fn notes_url(repo: &str, tag: &str) -> String {
    format!("https://github.com/{repo}/releases/tag/{tag}")
}

fn read_public_key(path: &Path) -> Result<minisign::PublicKey, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    PublicKeyBox::from_string(&text)
        .and_then(PublicKeyBox::into_public_key)
        .map_err(|error| format!("{} is not a minisign public key: {error}", path.display()))
}

/// Signs `manifest`, then checks the signature against `public`, so a secret key that belongs to
/// another key pair fails here and not in the app of every user.
pub fn sign_manifest(
    key_text: &str,
    password: Option<String>,
    public: &minisign::PublicKey,
    manifest: &str,
    tag: &str,
) -> Result<String, String> {
    let secret = SecretKeyBox::from_string(key_text)
        .and_then(|boxed| match password {
            Some(password) => boxed.into_secret_key(Some(password)),
            None => boxed.into_unencrypted_secret_key(),
        })
        .map_err(|error| {
            format!("cannot open {KEY_VAR} as a minisign secret key (is the password set in {PASSWORD_VAR}?): {error}")
        })?;
    let signature = minisign::sign(
        None,
        &secret,
        Cursor::new(manifest.as_bytes()),
        Some(&format!("hushpen release {tag}")),
        Some("signature from the hushpen release key"),
    )
    .map_err(|error| format!("cannot sign the manifest: {error}"))?;
    minisign::verify(
        public,
        &signature,
        Cursor::new(manifest.as_bytes()),
        true,
        false,
        false,
    )
    .map_err(|_| {
        format!("the signature of {KEY_VAR} does not verify with the public key in assets/update-key.pub")
    })?;
    Ok(signature.into_string())
}

#[derive(Debug, PartialEq, Eq)]
struct Args {
    dir: PathBuf,
    tag: String,
    repo: String,
    pubkey: PathBuf,
    pub_date: Option<String>,
}

fn parse_args(args: &[String], need_date: bool) -> Result<Args, String> {
    let (mut dir, mut tag, mut repo, mut pubkey, mut pub_date) = (None, None, None, None, None);
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = || {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value"))
        };
        match arg.as_str() {
            "--dir" => dir = Some(PathBuf::from(value()?)),
            "--tag" => tag = Some(value()?),
            "--repo" => repo = Some(value()?),
            "--pubkey" => pubkey = Some(PathBuf::from(value()?)),
            "--pub-date" => pub_date = Some(value()?),
            other => return Err(format!("unknown release argument {other:?}")),
        }
    }
    if need_date && pub_date.is_none() {
        return Err("release-manifest needs --pub-date <RFC 3339 time>".into());
    }
    Ok(Args {
        dir: dir.ok_or("needs --dir <folder>")?,
        tag: tag.ok_or("needs --tag <vX.Y.Z[-rc.N]>")?,
        repo: repo.ok_or("needs --repo <owner/name>")?,
        pubkey: pubkey.ok_or("needs --pubkey <file>")?,
        pub_date,
    })
}

/// The `platforms` entries: one installer for each target, served from the release itself.
fn platforms(dir: &Path, version: &str, tag: &str, repo: &str) -> Result<Vec<Platform>, String> {
    TARGETS
        .iter()
        .map(|target| {
            let name = artifact_name(version, *target, update_format(*target))
                .expect("an update format is a shipped format");
            let path = dir.join(&name);
            let size = fs::metadata(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?
                .len();
            Ok(Platform {
                target: target.name().to_owned(),
                url: download_url(repo, tag, &name),
                sha256: sha256_hex(&path)?,
                size,
            })
        })
        .collect()
}

fn write_sums(dir: &Path, version: &str) -> Result<(), String> {
    let mut names = installers(version);
    names.sort();
    let mut text = String::new();
    for name in names {
        text.push_str(&format!("{}  {name}\n", sha256_hex(&dir.join(&name))?));
    }
    fs::write(dir.join(SUMS), text).map_err(|error| format!("cannot write {SUMS}: {error}"))
}

/// Writes `SHA256SUMS`, `latest.json`, and `latest.json.minisig` into `--dir`, which already
/// holds the ten installers.
pub fn run_manifest(args: &[String]) -> Result<(), String> {
    let args = parse_args(args, true)?;
    let key_text = std::env::var(KEY_VAR).map_err(|_| format!("{KEY_VAR} is not set"))?;
    let password = std::env::var(PASSWORD_VAR)
        .ok()
        .filter(|text| !text.is_empty());
    write_release(&args, &key_text, password)
}

fn write_release(args: &Args, key_text: &str, password: Option<String>) -> Result<(), String> {
    let version = version_of_tag(&args.tag)?;
    let public = read_public_key(&args.pubkey)?;

    write_sums(&args.dir, &version)?;
    let manifest = manifest_text_at(
        &version,
        args.pub_date.as_deref().unwrap_or_default(),
        &notes_url(&args.repo, &args.tag),
        &platforms(&args.dir, &version, &args.tag, &args.repo)?,
    );
    let signature = sign_manifest(key_text, password, &public, &manifest, &args.tag)?;
    fs::write(args.dir.join(MANIFEST), &manifest)
        .map_err(|error| format!("cannot write {MANIFEST}: {error}"))?;
    fs::write(args.dir.join(SIGNATURE), signature)
        .map_err(|error| format!("cannot write {SIGNATURE}: {error}"))?;
    println!("wrote {SUMS}, {MANIFEST}, and {SIGNATURE} for {version}");
    Ok(())
}

fn file_names(dir: &Path) -> Result<BTreeSet<String>, String> {
    let entries =
        fs::read_dir(dir).map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
    Ok(entries
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect())
}

fn check_names(dir: &Path, version: &str, problems: &mut Vec<String>) {
    let planned = release_assets(version);
    let found = match file_names(dir) {
        Ok(found) => found,
        Err(error) => return problems.push(error),
    };
    for name in planned.difference(&found) {
        problems.push(format!("asset {name} is missing"));
    }
    for name in found.difference(&planned) {
        problems.push(format!("asset {name} is not in the plan"));
    }
    if planned == found {
        println!(
            "ok: asset names: {} files, the plan and nothing else",
            found.len()
        );
    }
}

fn check_sums(dir: &Path, version: &str, problems: &mut Vec<String>) {
    let Ok(text) = fs::read_to_string(dir.join(SUMS)) else {
        return problems.push(format!("{SUMS} cannot be read"));
    };
    let mut listed = std::collections::BTreeMap::new();
    for line in text.lines() {
        if let Some((hash, name)) = line.split_once("  ") {
            listed.insert(name.to_owned(), hash.to_owned());
        } else {
            problems.push(format!("{SUMS} has a malformed line"));
        }
    }
    let before = problems.len();
    let planned = installers(version);
    for name in &planned {
        match (listed.get(name), sha256_hex(&dir.join(name))) {
            (Some(listed), Ok(actual)) if *listed == actual => {}
            (Some(_), Ok(_)) => problems.push(format!("{SUMS} hash of {name} is wrong")),
            (None, _) => problems.push(format!("{SUMS} has no line for {name}")),
            (_, Err(_)) => problems.push(format!("{name} cannot be read for its hash")),
        }
    }
    for name in listed.keys().filter(|name| !planned.contains(name)) {
        problems.push(format!("{SUMS} lists {name}, which is not an installer"));
    }
    if problems.len() == before {
        println!(
            "ok: {SUMS}: sha256 of all {} installers match their files",
            planned.len()
        );
    }
}

fn check_manifest(
    dir: &Path,
    version: &str,
    tag: &str,
    repo: &str,
    public: &minisign::PublicKey,
    problems: &mut Vec<String>,
) {
    let (Ok(body), Ok(signature_text)) = (
        fs::read(dir.join(MANIFEST)),
        fs::read_to_string(dir.join(SIGNATURE)),
    ) else {
        return problems.push(format!("{MANIFEST} or {SIGNATURE} cannot be read"));
    };
    match SignatureBox::from_string(&signature_text)
        .map(|boxed| minisign::verify(public, &boxed, Cursor::new(&body), true, false, false))
    {
        Ok(Ok(())) => println!("ok: {MANIFEST} signature verifies with assets/update-key.pub"),
        _ => problems.push(format!(
            "{MANIFEST} signature does not verify with assets/update-key.pub"
        )),
    }

    let Ok(Value::Object(manifest)) = serde_json::from_slice::<Value>(&body) else {
        return problems.push(format!("{MANIFEST} is not a JSON object"));
    };
    let before = problems.len();
    if manifest.get("version").and_then(Value::as_str) != Some(version) {
        problems.push(format!("{MANIFEST} version is not {version}"));
    }
    if manifest.get("notes_url").and_then(Value::as_str) != Some(&notes_url(repo, tag)) {
        problems.push(format!(
            "{MANIFEST} notes_url is not the release page of {tag}"
        ));
    }
    let platforms = manifest.get("platforms").and_then(Value::as_object);
    let listed: BTreeSet<&str> = platforms
        .map(|targets| targets.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let wanted: BTreeSet<&str> = TARGETS.iter().map(|target| target.name()).collect();
    if listed != wanted {
        problems.push(format!("{MANIFEST} platforms are not exactly {wanted:?}"));
    }
    for target in TARGETS {
        let name = artifact_name(version, target, update_format(target))
            .expect("an update format is a shipped format");
        let Some(entry) = platforms.and_then(|targets| targets.get(target.name())) else {
            continue;
        };
        let path = dir.join(&name);
        let same = |field: &str, expected: Option<Value>| {
            expected.is_some_and(|expected| entry.get(field) == Some(&expected))
        };
        if !same("url", Some(Value::from(download_url(repo, tag, &name)))) {
            problems.push(format!(
                "{MANIFEST} url of {} is not the release asset {name}",
                target.name()
            ));
        }
        if !same("sha256", sha256_hex(&path).ok().map(Value::from)) {
            problems.push(format!(
                "{MANIFEST} sha256 of {} does not match {name}",
                target.name()
            ));
        }
        if !same(
            "size",
            fs::metadata(&path).ok().map(|meta| Value::from(meta.len())),
        ) {
            problems.push(format!(
                "{MANIFEST} size of {} does not match {name}",
                target.name()
            ));
        }
    }
    if problems.len() == before {
        println!(
            "ok: {MANIFEST}: version {version}, 3 targets, sha256 and size match the downloaded assets"
        );
    }
}

/// Checks a downloaded copy of the release in `--dir`; every problem is listed, not only the first.
pub fn run_verify(args: &[String]) -> Result<(), String> {
    let args = parse_args(args, false)?;
    let version = version_of_tag(&args.tag)?;
    let public = read_public_key(&args.pubkey)?;
    let mut problems = Vec::new();
    check_names(&args.dir, &version, &mut problems);
    check_sums(&args.dir, &version, &mut problems);
    check_manifest(
        &args.dir,
        &version,
        &args.tag,
        &args.repo,
        &public,
        &mut problems,
    );
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "release verification failed:\n  {}",
            problems.join("\n  ")
        ))
    }
}

#[cfg(test)]
mod tests;
