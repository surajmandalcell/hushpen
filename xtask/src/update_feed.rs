//! Test feeds for the update check: `cargo xtask update-test-keygen` makes a minisign key pair
//! with no password, and `cargo xtask update-feed` writes a signed `latest.json` for a local
//! feed. Both are for tests. The release key has a password and never touches the disk; CI
//! signs the real manifest with it.

use minisign::{KeyPair, SecretKeyBox};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

const PUB_DATE: &str = "2026-01-01T00:00:00Z";

/// What `update-feed` writes next to the manifest.
const MANIFEST: &str = "latest.json";
const SIGNATURE: &str = "latest.json.minisig";

#[derive(Debug, PartialEq, Eq)]
pub struct FeedOptions {
    out: PathBuf,
    key: PathBuf,
    version: String,
    notes_url: String,
    tamper: bool,
}

fn value(iter: &mut std::slice::Iter<'_, String>, flag: &str) -> Result<String, String> {
    iter.next()
        .cloned()
        .ok_or_else(|| format!("{flag} needs a value"))
}

pub fn parse_feed_args(args: &[String]) -> Result<FeedOptions, String> {
    let (mut out, mut key, mut version, mut notes_url, mut tamper) =
        (None, None, None, None, false);
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--out" => out = Some(PathBuf::from(value(&mut iter, "--out")?)),
            "--key" => key = Some(PathBuf::from(value(&mut iter, "--key")?)),
            "--version" => version = Some(value(&mut iter, "--version")?),
            "--notes-url" => notes_url = Some(value(&mut iter, "--notes-url")?),
            "--tamper" => tamper = true,
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
    })
}

/// The manifest text for `version`. The date is fixed so a test can change one byte of it.
pub fn manifest_text(version: &str, notes_url: &str) -> String {
    format!(
        "{{\n  \"version\": \"{version}\",\n  \"pub_date\": \"{PUB_DATE}\",\n  \"notes_url\": \"{notes_url}\"\n}}\n"
    )
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
    let manifest = manifest_text(&options.version, &options.notes_url);
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
