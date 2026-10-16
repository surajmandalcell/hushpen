use super::*;
use crate::update_feed::run_keygen;

const REPO: &str = "owner/repo";
const TAG: &str = "v2.0.0-rc.1";
const VERSION: &str = "2.0.0-rc.1";

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("xtask-release-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn text_args(list: &[&str]) -> Vec<String> {
    list.iter().map(|text| (*text).to_owned()).collect()
}

/// A key pair with no password; returns the secret key text and the public key file.
fn key_pair(dir: &Path, name: &str) -> (String, PathBuf) {
    run_keygen(&text_args(&[
        "--out",
        dir.to_str().unwrap(),
        "--name",
        name,
    ]))
    .unwrap();
    (
        fs::read_to_string(dir.join(format!("{name}.key"))).unwrap(),
        dir.join(format!("{name}.pub")),
    )
}

fn args_for(dir: &Path, pubkey: &Path) -> Args {
    Args {
        dir: dir.to_path_buf(),
        tag: TAG.into(),
        repo: REPO.into(),
        pubkey: pubkey.to_path_buf(),
        pub_date: Some("2026-10-17T00:00:00Z".into()),
    }
}

/// A folder with the ten installers, each with its own content.
fn installers_dir(dir: &Path) -> PathBuf {
    let out = dir.join("release");
    fs::create_dir_all(&out).unwrap();
    for name in installers(VERSION) {
        fs::write(out.join(&name), format!("content of {name}")).unwrap();
    }
    out
}

fn verify(dir: &Path, pubkey: &Path) -> Result<(), String> {
    run_verify(&text_args(&[
        "--dir",
        dir.to_str().unwrap(),
        "--tag",
        TAG,
        "--repo",
        REPO,
        "--pubkey",
        pubkey.to_str().unwrap(),
    ]))
}

/// A signed release folder and its public key.
fn signed_release(name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let dir = scratch(name);
    let (secret, pubkey) = key_pair(&dir, "release-key");
    let out = installers_dir(&dir);
    write_release(&args_for(&out, &pubkey), &secret, None).unwrap();
    (dir, out, pubkey)
}

#[test]
fn a_tag_gives_its_version_and_odd_tags_are_refused() {
    assert_eq!(version_of_tag("v2.0.0").unwrap(), "2.0.0");
    assert_eq!(version_of_tag("v2.0.0-rc.12").unwrap(), "2.0.0-rc.12");
    for bad in [
        "2.0.0",
        "v2.0",
        "v2.0.0-beta.1",
        "v2.0.0-rc.",
        "v2.0.0-rc.1x",
        "master",
        "",
    ] {
        assert!(version_of_tag(bad).is_err(), "{bad:?} must be refused");
    }
}

#[test]
fn the_release_holds_the_ten_installers_and_three_metadata_files() {
    let names = release_assets("2.0.0-rc.1");
    assert_eq!(names.len(), 13);
    for expected in [
        "Hushpen-2.0.0-rc.1-macos-arm64.zip",
        "Hushpen-2.0.0-rc.1-macos-arm64.dmg",
        "Hushpen-2.0.0-rc.1-linux-x64.AppImage",
        "Hushpen-2.0.0-rc.1-linux-x64.deb",
        "Hushpen-2.0.0-rc.1-linux-x64.rpm",
        "Hushpen-2.0.0-rc.1-linux-x64.tar.gz",
        "Hushpen-2.0.0-rc.1-linux-arm64.AppImage",
        "Hushpen-2.0.0-rc.1-linux-arm64.deb",
        "Hushpen-2.0.0-rc.1-linux-arm64.rpm",
        "Hushpen-2.0.0-rc.1-linux-arm64.tar.gz",
        "latest.json",
        "latest.json.minisig",
        "SHA256SUMS",
    ] {
        assert!(names.contains(expected), "{expected} is planned");
    }
}

#[test]
fn a_signed_release_verifies_and_the_manifest_lists_the_self_update_installers() {
    let (dir, out, pubkey) = signed_release("good");
    verify(&out, &pubkey).unwrap();
    let manifest: Value = serde_json::from_slice(&fs::read(out.join(MANIFEST)).unwrap()).unwrap();
    assert_eq!(manifest["version"], VERSION);
    assert_eq!(manifest["pub_date"], "2026-10-17T00:00:00Z");
    assert_eq!(
        manifest["platforms"]["macos-arm64"]["url"],
        "https://github.com/owner/repo/releases/download/v2.0.0-rc.1/Hushpen-2.0.0-rc.1-macos-arm64.zip"
    );
    assert_eq!(
        manifest["platforms"]["linux-arm64"]["size"],
        "content of Hushpen-2.0.0-rc.1-linux-arm64.AppImage".len()
    );
    let sums = fs::read_to_string(out.join(SUMS)).unwrap();
    assert_eq!(sums.lines().count(), 10);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_changed_installer_fails_the_sums_and_the_manifest_check() {
    let (dir, out, pubkey) = signed_release("changed");
    fs::write(
        out.join("Hushpen-2.0.0-rc.1-linux-x64.AppImage"),
        "other bytes",
    )
    .unwrap();
    let error = verify(&out, &pubkey).unwrap_err();
    assert!(
        error.contains("SHA256SUMS hash of Hushpen-2.0.0-rc.1-linux-x64.AppImage"),
        "{error}"
    );
    assert!(
        error.contains("sha256 of linux-x64 does not match"),
        "{error}"
    );
    assert!(
        error.contains("size of linux-x64 does not match"),
        "{error}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_missing_or_extra_asset_fails_the_name_check() {
    let (dir, out, pubkey) = signed_release("names");
    fs::remove_file(out.join("Hushpen-2.0.0-rc.1-linux-arm64.rpm")).unwrap();
    fs::write(out.join("stray.txt"), "x").unwrap();
    let error = verify(&out, &pubkey).unwrap_err();
    assert!(
        error.contains("asset Hushpen-2.0.0-rc.1-linux-arm64.rpm is missing"),
        "{error}"
    );
    assert!(
        error.contains("asset stray.txt is not in the plan"),
        "{error}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_changed_manifest_fails_the_signature() {
    let (dir, out, pubkey) = signed_release("tamper");
    let body = fs::read_to_string(out.join(MANIFEST)).unwrap();
    fs::write(out.join(MANIFEST), body.replacen("2026", "2027", 1)).unwrap();
    let error = verify(&out, &pubkey).unwrap_err();
    assert!(error.contains("signature does not verify"), "{error}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_release_from_another_tag_fails_the_version_check() {
    let (dir, out, pubkey) = signed_release("version");
    let error = run_verify(&text_args(&[
        "--dir",
        out.to_str().unwrap(),
        "--tag",
        "v2.0.0",
        "--repo",
        REPO,
        "--pubkey",
        pubkey.to_str().unwrap(),
    ]))
    .unwrap_err();
    assert!(error.contains("version is not 2.0.0"), "{error}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_secret_key_of_another_pair_is_refused_before_anything_is_written() {
    let dir = scratch("mismatch");
    let (secret, _) = key_pair(&dir, "other");
    let (_, pubkey) = key_pair(&dir, "embedded");
    let out = installers_dir(&dir);
    let error = write_release(&args_for(&out, &pubkey), &secret, None).unwrap_err();
    assert!(
        error.contains("does not verify with the public key"),
        "{error}"
    );
    assert!(!out.join(MANIFEST).exists());
    assert!(!out.join(SIGNATURE).exists());
    assert!(
        !error.contains(secret.lines().last().unwrap()),
        "the key is never repeated"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn an_unreadable_key_names_the_variable_and_not_its_text() {
    let dir = scratch("garbage");
    let (_, pubkey) = key_pair(&dir, "embedded");
    let out = installers_dir(&dir);
    let error = write_release(&args_for(&out, &pubkey), "not a key at all", None).unwrap_err();
    assert!(error.contains("HUSHPEN_UPDATE_KEY"), "{error}");
    assert!(!error.contains("not a key at all"), "{error}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_manifest_needs_all_ten_installers() {
    let dir = scratch("short");
    let (secret, pubkey) = key_pair(&dir, "k");
    let out = installers_dir(&dir);
    fs::remove_file(out.join("Hushpen-2.0.0-rc.1-macos-arm64.zip")).unwrap();
    assert!(write_release(&args_for(&out, &pubkey), &secret, None).is_err());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn arguments_are_checked() {
    let all = ["--dir", "d", "--tag", "t", "--repo", "r", "--pubkey", "p"];
    assert!(parse_args(&text_args(&all), false).is_ok());
    assert!(
        parse_args(&text_args(&all), true).is_err(),
        "release-manifest needs a date"
    );
    assert!(parse_args(&text_args(&["--dir", "d"]), false).is_err());
    assert!(parse_args(&text_args(&["--bogus"]), false).is_err());
}
