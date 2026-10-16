//! Test feeds for the update check: `cargo xtask update-test-keygen` makes a minisign key pair
//! with no password, and `cargo xtask update-feed` writes a signed `latest.json` for a local
//! feed. Both are for tests. The release manifest is signed by `release.rs` with the secret key
//! from the environment, which never touches the disk.

use minisign::{KeyPair, SecretKeyBox};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

const PUB_DATE: &str = "2026-01-01T00:00:00Z";

/// What `update-feed` writes next to the manifest.
const MANIFEST: &str = "latest.json";
const SIGNATURE: &str = "latest.json.minisig";

/// Where the local feed server answers when `--base-url` is not given.
const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8765";

#[derive(Debug, PartialEq, Eq)]
pub struct FeedOptions {
    out: PathBuf,
    key: PathBuf,
    version: String,
    notes_url: String,
    tamper: bool,
    /// `target=file` pairs: each file is listed under that target of the manifest.
    artifacts: Vec<(String, PathBuf)>,
    base_url: String,
    /// Lists every artifact with a hash the file does not have.
    wrong_sha: bool,
}

/// One entry of `platforms`.
#[derive(Debug, PartialEq, Eq)]
pub struct Platform {
    pub target: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

fn value(iter: &mut std::slice::Iter<'_, String>, flag: &str) -> Result<String, String> {
    iter.next()
        .cloned()
        .ok_or_else(|| format!("{flag} needs a value"))
}

pub fn parse_feed_args(args: &[String]) -> Result<FeedOptions, String> {
    let (mut out, mut key, mut version, mut notes_url, mut tamper) =
        (None, None, None, None, false);
    let (mut artifacts, mut base_url, mut wrong_sha) = (Vec::new(), None, false);
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--out" => out = Some(PathBuf::from(value(&mut iter, "--out")?)),
            "--key" => key = Some(PathBuf::from(value(&mut iter, "--key")?)),
            "--version" => version = Some(value(&mut iter, "--version")?),
            "--notes-url" => notes_url = Some(value(&mut iter, "--notes-url")?),
            "--tamper" => tamper = true,
            "--artifact" => {
                let pair = value(&mut iter, "--artifact")?;
                let (target, file) = pair
                    .split_once('=')
                    .filter(|(target, file)| !target.is_empty() && !file.is_empty())
                    .ok_or("--artifact is <target>=<file>, for example linux-arm64=./a.AppImage")?;
                artifacts.push((target.to_owned(), PathBuf::from(file)));
            }
            "--base-url" => base_url = Some(value(&mut iter, "--base-url")?),
            "--wrong-sha" => wrong_sha = true,
            other => return Err(format!("unknown update-feed argument {other:?}")),
        }
    }
    let version = version.ok_or("update-feed needs --version")?;
    Ok(FeedOptions {
        out: out.ok_or("update-feed needs --out <folder>")?,
        key: key.ok_or("update-feed needs --key <secret key file>")?,
        notes_url: notes_url
            .unwrap_or_else(|| format!("https://example.com/hushpen/releases/{version}")),
        version,
        tamper,
        artifacts,
        base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_owned()),
        wrong_sha,
    })
}

/// The manifest text for `version`. The date is fixed so a test can change one byte of it.
pub fn manifest_text(version: &str, notes_url: &str, platforms: &[Platform]) -> String {
    manifest_text_at(version, PUB_DATE, notes_url, platforms)
}

/// Like [`manifest_text`], with the publication time of a real release.
pub fn manifest_text_at(
    version: &str,
    pub_date: &str,
    notes_url: &str,
    platforms: &[Platform],
) -> String {
    let mut text = format!(
        "{{\n  \"version\": \"{version}\",\n  \"pub_date\": \"{pub_date}\",\n  \"notes_url\": \"{notes_url}\""
    );
    if !platforms.is_empty() {
        let entries: Vec<String> = platforms
            .iter()
            .map(|p| {
                format!(
                    "    \"{}\": {{\"url\": \"{}\", \"sha256\": \"{}\", \"size\": {}}}",
                    p.target, p.url, p.sha256, p.size
                )
            })
            .collect();
        text.push_str(&format!(
            ",\n  \"platforms\": {{\n{}\n  }}",
            entries.join(",\n")
        ));
    }
    text.push_str("\n}\n");
    text
}

/// The `platforms` entries for the artifact files: each is served at `<base_url>/<file name>`.
fn platforms(options: &FeedOptions) -> Result<Vec<Platform>, String> {
    options
        .artifacts
        .iter()
        .map(|(target, file)| {
            let bytes = fs::read(file)
                .map_err(|error| format!("cannot read {}: {error}", file.display()))?;
            let name = file
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| format!("{} has no usable name", file.display()))?;
            let sha256 = if options.wrong_sha {
                "0".repeat(64)
            } else {
                Sha256::digest(&bytes)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect()
            };
            Ok(Platform {
                target: target.clone(),
                url: format!("{}/{name}", options.base_url.trim_end_matches('/')),
                sha256,
                size: bytes.len() as u64,
            })
        })
        .collect()
}

fn read_secret(path: &Path) -> Result<minisign::SecretKey, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    SecretKeyBox::from_string(&text)
        .and_then(SecretKeyBox::into_unencrypted_secret_key)
        .map_err(|error| format!("cannot open {} as a test key: {error}", path.display()))
}

/// Writes `latest.json` and `latest.json.minisig` into `options.out`. With `tamper`, one byte of
/// the manifest changes after it was signed.
pub fn write_feed(options: &FeedOptions) -> Result<(), String> {
    let secret = read_secret(&options.key)?;
    let manifest = manifest_text(&options.version, &options.notes_url, &platforms(options)?);
    let signature = minisign::sign(
        None,
        &secret,
        Cursor::new(manifest.as_bytes()),
        Some("hushpen test feed"),
        Some("signature from a hushpen test key"),
    )
    .map_err(|error| format!("cannot sign the manifest: {error}"))?;
    let shipped = if options.tamper {
        manifest.replacen(&PUB_DATE[..4], "2027", 1)
    } else {
        manifest
    };
    fs::create_dir_all(&options.out)
        .map_err(|error| format!("cannot create {}: {error}", options.out.display()))?;
    fs::write(options.out.join(MANIFEST), shipped)
        .map_err(|error| format!("cannot write the manifest: {error}"))?;
    fs::write(options.out.join(SIGNATURE), signature.into_string())
        .map_err(|error| format!("cannot write the signature: {error}"))?;
    println!("wrote {}", options.out.join(MANIFEST).display());
    Ok(())
}

pub fn run_feed(args: &[String]) -> Result<(), String> {
    write_feed(&parse_feed_args(args)?)
}

/// Writes `<name>.key` (0600) and `<name>.pub` into the folder after `--out`.
pub fn run_keygen(args: &[String]) -> Result<(), String> {
    let (mut out, mut name) = (None, "update-test".to_owned());
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--out" => out = Some(PathBuf::from(value(&mut iter, "--out")?)),
            "--name" => name = value(&mut iter, "--name")?,
            other => return Err(format!("unknown update-test-keygen argument {other:?}")),
        }
    }
    let out = out.ok_or("update-test-keygen needs --out <folder>")?;
    fs::create_dir_all(&out)
        .map_err(|error| format!("cannot create {}: {error}", out.display()))?;
    let pair = KeyPair::generate_unencrypted_keypair()
        .map_err(|error| format!("cannot make a key pair: {error}"))?;
    let secret = pair
        .sk
        .to_box(Some("hushpen test key, never for a release"))
        .map_err(|error| format!("cannot write the secret key: {error}"))?
        .into_string();
    let public = pair
        .pk
        .to_box()
        .map_err(|error| format!("cannot write the public key: {error}"))?
        .into_string();
    let secret_path = out.join(format!("{name}.key"));
    fs::write(&secret_path, secret).map_err(|error| format!("cannot write the key: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&secret_path, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("cannot protect the key: {error}"))?;
    }
    let public_path = out.join(format!("{name}.pub"));
    fs::write(&public_path, public).map_err(|error| format!("cannot write the key: {error}"))?;
    println!(
        "wrote {} and {}",
        secret_path.display(),
        public_path.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xtask-feed-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn arguments_are_parsed_and_the_required_ones_checked() {
        let options = parse_feed_args(&args(&[
            "--out",
            "o",
            "--key",
            "k",
            "--version",
            "9.9.9",
            "--tamper",
        ]))
        .unwrap();
        assert_eq!(options.version, "9.9.9");
        assert!(options.tamper);
        assert!(options.notes_url.contains("9.9.9"));
        assert!(parse_feed_args(&args(&["--out", "o", "--key", "k"])).is_err());
        assert!(parse_feed_args(&args(&["--bogus"])).is_err());
    }

    #[test]
    fn artifacts_are_listed_with_their_real_hash_size_and_a_url_under_the_base() {
        let dir = scratch("artifacts");
        let file = dir.join("Hushpen.AppImage");
        fs::write(&file, b"abc").unwrap();
        let spec = format!("linux-arm64={}", file.display());
        let mut list = vec![
            "--out",
            "o",
            "--key",
            "k",
            "--version",
            "9.9.9",
            "--artifact",
            &spec,
            "--base-url",
            "http://127.0.0.1:9/",
        ];
        let options = parse_feed_args(&args(&list)).unwrap();
        let listed = platforms(&options).unwrap();
        assert_eq!(
            listed,
            [Platform {
                target: "linux-arm64".into(),
                url: "http://127.0.0.1:9/Hushpen.AppImage".into(),
                sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
                size: 3,
            }]
        );
        let text = manifest_text("9.9.9", "https://example.com/n", &listed);
        assert!(text.contains(r#""linux-arm64": {"url": "http://127.0.0.1:9/Hushpen.AppImage""#));
        assert!(text.ends_with("}\n}\n"));

        list.push("--wrong-sha");
        let wrong = platforms(&parse_feed_args(&args(&list)).unwrap()).unwrap();
        assert_eq!(wrong[0].sha256, "0".repeat(64));
        assert_eq!(wrong[0].size, 3);

        assert!(parse_feed_args(&args(&["--artifact", "nofile"])).is_err());
        assert!(parse_feed_args(&args(&["--artifact", "=x"])).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_manifest_without_artifacts_has_no_platforms_key() {
        let text = manifest_text("9.9.9", "https://example.com/n", &[]);
        assert!(!text.contains("platforms"));
    }

    #[test]
    fn a_feed_verifies_with_its_own_key_and_not_after_a_tamper_or_with_another_key() {
        let dir = scratch("verify");
        run_keygen(&args(&["--out", dir.to_str().unwrap(), "--name", "a"])).unwrap();
        run_keygen(&args(&["--out", dir.to_str().unwrap(), "--name", "b"])).unwrap();
        let (a_key, b_key) = (dir.join("a.key"), dir.join("b.key"));
        let public = |name: &str| {
            minisign_verify::PublicKey::decode(
                &fs::read_to_string(dir.join(format!("{name}.pub"))).unwrap(),
            )
            .unwrap()
        };
        let feed = |sub: &str, key: &Path, tamper: bool| {
            let out = dir.join(sub);
            let mut list = vec![
                "--out",
                out.to_str().unwrap(),
                "--key",
                key.to_str().unwrap(),
                "--version",
                "9.9.9",
            ];
            if tamper {
                list.push("--tamper");
            }
            run_feed(&args(&list)).unwrap();
            (
                fs::read(out.join(MANIFEST)).unwrap(),
                minisign_verify::Signature::from_file(out.join(SIGNATURE)).unwrap(),
            )
        };

        let (body, signature) = feed("good", &a_key, false);
        assert!(public("a").verify(&body, &signature, true).is_ok());
        assert!(
            public("b").verify(&body, &signature, true).is_err(),
            "another key"
        );

        let (body, signature) = feed("tampered", &a_key, true);
        assert!(
            public("a").verify(&body, &signature, true).is_err(),
            "one byte changed"
        );

        let (body, signature) = feed("wrong", &b_key, false);
        assert!(
            public("a").verify(&body, &signature, true).is_err(),
            "signed by b"
        );
        let _ = fs::remove_dir_all(dir);
    }
}
