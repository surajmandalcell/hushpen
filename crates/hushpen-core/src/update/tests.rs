use super::*;

fn v(text: &str) -> Version {
    Version::parse(text).unwrap_or_else(|| panic!("{text} should parse"))
}

#[test]
fn a_release_candidate_sorts_before_its_release_and_after_the_earlier_candidate() {
    assert!(v("2.0.0-rc.1") < v("2.0.0-rc.2"));
    assert!(v("2.0.0-rc.2") < v("2.0.0"));
    assert!(v("2.0.0-rc.10") > v("2.0.0-rc.9"));
    assert!(v("2.0.0-dev") < v("2.0.0"));
    assert!(v("1.9.9") < v("2.0.0-rc.1"));
    assert!(v("2.0.1") > v("2.0.0"));
    assert!(v("2.1.0") > v("2.0.99"));
}

#[test]
fn equal_versions_are_equal_whatever_the_build_text_or_prefix() {
    assert_eq!(v("2.0.0"), v("v2.0.0"));
    assert_eq!(v("2.0.0+abc"), v("2.0.0+def"));
    assert_eq!(v("2.0.0-dev").as_str(), "2.0.0-dev");
    assert_eq!(v("v2.0.0").as_str(), "2.0.0");
}

#[test]
fn a_text_part_sorts_after_a_number_part_as_semver_says() {
    assert!(v("1.0.0-1") < v("1.0.0-alpha"));
    assert!(v("1.0.0-alpha") < v("1.0.0-alpha.1"));
    assert!(v("1.0.0-alpha.1") < v("1.0.0-beta"));
}

#[test]
fn text_that_is_not_a_version_is_refused() {
    for bad in [
        "",
        "2",
        "2.0",
        "2.0.0.1",
        "a.b.c",
        "2.0.x",
        "2.0.0-",
        "2.0.0-rc..1",
        "-1.0.0",
        "2.0.0-rc!",
        "2. 0.0",
    ] {
        assert!(Version::parse(bad).is_none(), "{bad:?} parsed");
    }
    assert!(Version::parse(&"9".repeat(80)).is_none());
}

#[test]
fn a_number_too_big_for_u64_is_refused() {
    assert!(Version::parse("99999999999999999999999.0.0").is_none());
}

#[test]
fn an_older_or_equal_manifest_is_not_offered() {
    let current = v("2.0.0-rc.2");
    for version in ["2.0.0-rc.2", "2.0.0-rc.1", "1.0.0"] {
        let manifest = manifest(version);
        assert_eq!(offer(&current, manifest), Offer::UpToDate, "{version}");
    }
    match offer(&current, manifest("2.0.0")) {
        Offer::Available(found) => assert_eq!(found.version.as_str(), "2.0.0"),
        other => panic!("{other:?}"),
    }
}

fn manifest(version: &str) -> Manifest {
    parse_manifest(
        format!(r#"{{"version":"{version}","notes_url":"https://github.com/notes"}}"#).as_bytes(),
    )
    .unwrap()
}

#[test]
fn the_check_is_due_after_24_hours_and_not_before() {
    let hour = 3_600_000u64;
    let t0 = 1_700_000_000_000u64;
    assert!(check_due(0, t0), "never checked");
    assert!(!check_due(t0, t0));
    assert!(!check_due(t0, t0 + 23 * hour + 59 * 60_000));
    assert!(check_due(t0, t0 + 24 * hour));
    assert!(check_due(t0, t0 + 25 * hour));
}

#[test]
fn a_clock_that_went_back_makes_the_check_due() {
    assert!(check_due(2_000_000_000_000, 1_000_000_000_000));
}

const FULL: &str = r#"{
  "version": "2.0.1",
  "pub_date": "2026-10-20T10:00:00Z",
  "notes_url": "https://github.com/surajmandalcell/asrpro/releases/tag/v2.0.1",
  "platforms": {
    "macos-arm64": {
      "url": "https://github.com/surajmandalcell/asrpro/releases/download/v2.0.1/Hushpen-2.0.1-macos-arm64.zip",
      "sha256": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
      "size": 17500000
    }
  }
}"#;

#[test]
fn a_full_manifest_is_read() {
    let manifest = parse_manifest(FULL.as_bytes()).unwrap();
    assert_eq!(manifest.version.as_str(), "2.0.1");
    assert_eq!(
        manifest.notes_url,
        "https://github.com/surajmandalcell/asrpro/releases/tag/v2.0.1"
    );
    assert_eq!(manifest.pub_date.as_deref(), Some("2026-10-20T10:00:00Z"));
    let artifact = &manifest.platforms["macos-arm64"];
    assert_eq!(artifact.size, 17_500_000);
    assert_eq!(artifact.sha256, "a".repeat(64), "the hash is lower-cased");
}

#[test]
fn a_manifest_needs_only_a_version_and_a_notes_link() {
    let manifest =
        parse_manifest(br#"{"version":"3.0.0","notes_url":"http://127.0.0.1:1/n"}"#).unwrap();
    assert!(manifest.platforms.is_empty());
    assert!(manifest.pub_date.is_none());
}

#[test]
fn a_manifest_that_is_wrong_is_refused_with_the_field_name() {
    let bad = |json: &str| parse_manifest(json.as_bytes()).unwrap_err();
    assert_eq!(bad("not json"), ManifestError::NotJson);
    assert_eq!(bad("[]"), ManifestError::NotJson);
    assert_eq!(
        bad(r#"{"notes_url":"https://github.com/c"}"#),
        ManifestError::Missing("version")
    );
    assert_eq!(
        bad(r#"{"version":"2.0.0"}"#),
        ManifestError::Missing("notes_url")
    );
    assert_eq!(
        bad(r#"{"version":"two","notes_url":"https://github.com/c"}"#),
        ManifestError::Invalid("version")
    );
    assert_eq!(
        bad(r#"{"version":2,"notes_url":"https://github.com/c"}"#),
        ManifestError::Invalid("version")
    );
    for url in [
        "file:///etc/passwd",
        "javascript:alert(1)",
        "https://",
        "https://github.com/a b",
        "ftp://a.b/c",
    ] {
        assert_eq!(
            bad(&format!(r#"{{"version":"2.0.0","notes_url":"{url}"}}"#)),
            ManifestError::Invalid("notes_url"),
            "{url}"
        );
    }
}

#[test]
fn a_platform_entry_with_a_bad_hash_size_or_url_is_refused() {
    let with = |entry: &str| {
        parse_manifest(
            format!(
                r#"{{"version":"2.0.1","notes_url":"https://github.com/c","platforms":{{"linux-x64":{entry}}}}}"#
            )
            .as_bytes(),
        )
        .unwrap_err()
    };
    let hash = "b".repeat(64);
    assert_eq!(
        with(&format!(
            r#"{{"url":"https://github.com/f","sha256":"{}","size":1}}"#,
            "zz".repeat(32)
        )),
        ManifestError::Invalid("platforms")
    );
    assert_eq!(
        with(&format!(
            r#"{{"url":"https://github.com/f","sha256":"{hash}"}}"#
        )),
        ManifestError::Invalid("platforms")
    );
    assert_eq!(
        with(&format!(
            r#"{{"url":"ftp://a.b/f","sha256":"{hash}","size":1}}"#
        )),
        ManifestError::Invalid("platforms")
    );
    assert_eq!(
        with(r#"{"url":"https://github.com/f","sha256":"abc","size":1}"#),
        ManifestError::Invalid("platforms")
    );
}

fn manifest_with(targets: &[&str]) -> Manifest {
    let entries: Vec<String> = targets
        .iter()
        .map(|target| {
            format!(
                r#""{target}":{{"url":"https://github.com/f/{target}","sha256":"{}","size":9}}"#,
                "c".repeat(64)
            )
        })
        .collect();
    let json = format!(
        r#"{{"version":"2.0.1","notes_url":"https://github.com/n","platforms":{{{}}}}}"#,
        entries.join(",")
    );
    parse_manifest(json.as_bytes()).unwrap()
}

#[test]
fn an_install_kind_file_names_one_of_the_package_kinds() {
    for (text, kind) in [
        ("app\n", InstallKind::App),
        ("appimage\n", InstallKind::AppImage),
        ("deb", InstallKind::Deb),
        (" rpm \n", InstallKind::Rpm),
        ("tar\n", InstallKind::Tar),
        ("", InstallKind::Unknown),
        ("flatpak", InstallKind::Unknown),
    ] {
        assert_eq!(InstallKind::parse(text), kind, "{text:?}");
    }
    assert_eq!(InstallKind::parse("appimage").key(), "appimage");
    assert_eq!(InstallKind::Unknown.key(), "unknown");
}

#[test]
fn the_manifest_key_follows_the_system_and_the_cpu() {
    assert_eq!(target_key("macos", "aarch64"), Some("macos-arm64"));
    assert_eq!(target_key("linux", "x86_64"), Some("linux-x64"));
    assert_eq!(target_key("linux", "aarch64"), Some("linux-arm64"));
    assert_eq!(target_key("macos", "x86_64"), None);
    assert_eq!(target_key("windows", "x86_64"), None);
}

#[test]
fn a_zip_or_an_appimage_with_a_file_for_this_target_installs_itself() {
    let manifest = manifest_with(&["linux-arm64", "macos-arm64"]);
    for (kind, target) in [
        (InstallKind::AppImage, "linux-arm64"),
        (InstallKind::App, "macos-arm64"),
    ] {
        let Delivery::Artifact(artifact) = delivery(kind, Some(target), &manifest) else {
            panic!("{kind:?} should install itself");
        };
        assert!(artifact.url.ends_with(target));
    }
}

#[test]
fn deb_rpm_and_tar_only_get_a_notice_even_when_the_manifest_has_their_file() {
    let manifest = manifest_with(&["linux-x64"]);
    for kind in [InstallKind::Deb, InstallKind::Rpm, InstallKind::Tar] {
        assert_eq!(
            delivery(kind, Some("linux-x64"), &manifest),
            Delivery::Notice,
            "{kind:?}"
        );
    }
}

#[test]
fn a_release_without_a_file_for_this_target_is_a_notice() {
    let manifest = manifest_with(&["macos-arm64"]);
    assert_eq!(
        delivery(InstallKind::AppImage, Some("linux-x64"), &manifest),
        Delivery::Notice
    );
    assert_eq!(
        delivery(InstallKind::AppImage, None, &manifest),
        Delivery::Notice
    );
}

#[test]
fn a_source_tree_build_offers_the_install_and_refuses_it_later() {
    let manifest = manifest_with(&[]);
    assert_eq!(
        delivery(InstallKind::Unknown, Some("linux-x64"), &manifest),
        Delivery::Unpackaged
    );
}
