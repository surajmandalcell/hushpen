use super::*;
use crate::net::test_server::{Req, TestServer, respond};
use hushpen_core::error::{UPDATE_DOWNLOAD_FAILED, UPDATE_HASH_MISMATCH};
use std::net::TcpStream;
use std::sync::Arc;

// Made with `cargo xtask update-test-keygen` and `cargo xtask update-feed --version 2.1.0`.
const KEY_A: &str = "RWRNyoIuV/8LntIc6od0cNlA8PZaKbAMI9KR235+sNVUPyHDnoRxzul4";
const KEY_B: &str = "RWRc1jaPHj5VZqAo95bedIDtYnW++ZxaC/Y8NxB9mUeJNu2Q8cbJVHkl";
const NOTES: &str = "https://github.com/surajmandalcell/asrpro/releases/tag/v2.1.0";
const MANIFEST: &str = "{\n  \"version\": \"2.1.0\",\n  \"pub_date\": \"2026-01-01T00:00:00Z\",\n  \"notes_url\": \"https://github.com/surajmandalcell/asrpro/releases/tag/v2.1.0\"\n}\n";
const SIGNATURE: &str = "untrusted comment: signature from a hushpen test key\nRURNyoIuV/8LnnjLVwaM72JSEbMuioUHzDeHH0nJsdI6fh2ts+wSk2V7Tj11SXy/QyzlEEF0j3dLBicJxn1yqWOB6gV5OY95qQs=\ntrusted comment: hushpen test feed\n5eY0xjda9BkkV8nug/vgc3Mww+XeTG1qz01MxSqlVoWOSPoC0rg01LNlh499+E97aVytcBjLUAWqonouOp0gCQ==\n";

fn version(text: &str) -> Version {
    Version::parse(text).unwrap()
}

fn feed_at(server: &TestServer, key: &str) -> UpdateFeed {
    UpdateFeed::with(
        &server.url("/latest.json"),
        PublicKey::from_base64(key).unwrap(),
    )
    .unwrap()
}

fn serve(manifest: &'static str, signature: &'static str) -> TestServer {
    TestServer::start(
        move |request: &Req, stream: &mut TcpStream| match request.path.as_str() {
            "/latest.json" => respond(stream, "200 OK", &[], manifest.as_bytes()),
            "/latest.json.minisig" => respond(stream, "200 OK", &[], signature.as_bytes()),
            _ => respond(stream, "404 Not Found", &[], b""),
        },
    )
}

fn check(server: &TestServer, key: &str, current: &str) -> Result<Offer, CheckError> {
    let feed = feed_at(server, key);
    feed.check(&UreqFetcher::with_https_only(false), &version(current))
}

#[test]
fn a_valid_signature_is_accepted_and_a_newer_version_is_offered() {
    let server = serve(MANIFEST, SIGNATURE);
    let Offer::Available(manifest) = check(&server, KEY_A, "2.0.0").unwrap() else {
        panic!("2.1.0 is newer than 2.0.0");
    };
    assert_eq!(manifest.version.as_str(), "2.1.0");
    assert_eq!(manifest.notes_url, NOTES);
    let paths: Vec<String> = server.seen().into_iter().map(|r| r.path).collect();
    assert_eq!(paths, ["/latest.json", "/latest.json.minisig"]);
}

#[test]
fn a_tampered_body_is_rejected_with_the_signature_code() {
    let tampered: &'static str = Box::leak(MANIFEST.replace("2026", "2027").into_boxed_str());
    let server = serve(tampered, SIGNATURE);
    let error = check(&server, KEY_A, "2.0.0").unwrap_err();
    assert_eq!(error.code, UPDATE_SIGNATURE_INVALID);
}

#[test]
fn a_signature_from_another_key_is_rejected() {
    let server = serve(MANIFEST, SIGNATURE);
    let error = check(&server, KEY_B, "2.0.0").unwrap_err();
    assert_eq!(error.code, UPDATE_SIGNATURE_INVALID);
}

#[test]
fn a_signature_file_that_is_not_a_signature_is_rejected() {
    let server = serve(MANIFEST, "not a signature");
    let error = check(&server, KEY_A, "2.0.0").unwrap_err();
    assert_eq!(error.code, UPDATE_SIGNATURE_INVALID);
}

#[test]
fn an_older_or_equal_version_is_not_offered() {
    let server = serve(MANIFEST, SIGNATURE);
    assert_eq!(check(&server, KEY_A, "2.1.0").unwrap(), Offer::UpToDate);
    assert_eq!(check(&server, KEY_A, "3.0.0").unwrap(), Offer::UpToDate);
    assert!(check(&server, KEY_A, "2.1.0-rc.1").unwrap().is_available());
}

#[test]
fn a_missing_manifest_is_a_check_failure_not_a_signature_failure() {
    let server = TestServer::start(|_: &Req, stream: &mut TcpStream| {
        respond(stream, "404 Not Found", &[], b"");
    });
    let error = check(&server, KEY_A, "2.0.0").unwrap_err();
    assert_eq!(error.code, UPDATE_CHECK_FAILED);
    assert!(error.detail.contains("404"));
}

#[test]
fn a_request_carries_no_query_cookie_or_identifying_header() {
    let server = serve(MANIFEST, SIGNATURE);
    check(&server, KEY_A, "2.0.0").unwrap();
    for request in server.seen() {
        assert_eq!(request.method, "GET");
        assert!(!request.path.contains('?'), "{}", request.path);
        let names: Vec<&str> = request
            .headers
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        for banned in ["cookie", "authorization", "referer", "x-request-id"] {
            assert!(!names.contains(&banned), "{banned} in {names:?}");
        }
        let agent = request.header("user-agent").unwrap();
        assert_eq!(agent, format!("Hushpen/{}", hushpen_core::BUILD_VERSION));
    }
}

#[test]
fn a_redirect_off_the_feed_address_is_refused_before_a_connection_opens() {
    let other = serve(MANIFEST, SIGNATURE);
    let target = other.url("/latest.json");
    let server = TestServer::start(move |_: &Req, stream: &mut TcpStream| {
        respond(stream, "302 Found", &[("Location", target.clone())], b"");
    });
    let error = check(&server, KEY_A, "2.0.0").unwrap_err();
    assert_eq!(error.code, UPDATE_CHECK_FAILED);
    assert!(other.seen().is_empty(), "the other server was asked");
}

#[test]
fn a_redirect_to_the_same_address_is_followed() {
    let manifest = Arc::new(MANIFEST);
    let server = TestServer::start(move |request: &Req, stream: &mut TcpStream| {
        match request.path.as_str() {
            "/latest.json" => respond(stream, "302 Found", &[("Location", "/asset".into())], b""),
            "/asset" => respond(stream, "200 OK", &[], manifest.as_bytes()),
            "/latest.json.minisig" => respond(stream, "200 OK", &[], SIGNATURE.as_bytes()),
            _ => respond(stream, "404 Not Found", &[], b""),
        }
    });
    assert!(check(&server, KEY_A, "2.0.0").unwrap().is_available());
}

#[test]
fn an_oversized_answer_is_refused() {
    let server = TestServer::start(|_: &Req, stream: &mut TcpStream| {
        respond(stream, "200 OK", &[], &vec![b'x'; 300 * 1024]);
    });
    let error = check(&server, KEY_A, "2.0.0").unwrap_err();
    assert_eq!(error.code, UPDATE_CHECK_FAILED);
}

fn artifact_for(server: &TestServer, body: &[u8], sha256: Option<String>) -> Artifact {
    use sha2::{Digest, Sha256};
    Artifact {
        url: server.url("/Hushpen.AppImage"),
        sha256: sha256.unwrap_or_else(|| {
            Sha256::digest(body)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect()
        }),
        size: body.len() as u64,
    }
}

fn download(
    server: &TestServer,
    artifact: &Artifact,
    folder: &Path,
) -> Result<Outcome, CheckError> {
    let feed = feed_at(server, KEY_A);
    feed.download(
        artifact,
        &folder.join("Hushpen.part"),
        &folder.join("Hushpen.staged"),
        &AtomicBool::new(false),
        &mut |_| {},
    )
}

fn serve_file(body: &'static [u8]) -> TestServer {
    TestServer::start(
        move |request: &Req, stream: &mut TcpStream| match request.path.as_str() {
            "/Hushpen.AppImage" => respond(stream, "200 OK", &[], body),
            _ => respond(stream, "404 Not Found", &[], b""),
        },
    )
}

#[test]
fn a_release_file_is_downloaded_checked_and_renamed_into_place() {
    let body: &'static [u8] = b"an appimage, more or less";
    let server = serve_file(body);
    let folder = tempfile::tempdir().unwrap();
    let artifact = artifact_for(&server, body, None);
    assert_eq!(
        download(&server, &artifact, folder.path()).unwrap(),
        Outcome::Done
    );
    assert_eq!(
        std::fs::read(folder.path().join("Hushpen.staged")).unwrap(),
        body
    );
    assert!(!folder.path().join("Hushpen.part").exists());
    assert!(
        !folder.path().join("Hushpen.staged.verified").exists(),
        "only models get a stamp"
    );
    let asked: Vec<String> = server.seen().into_iter().map(|r| r.path).collect();
    assert_eq!(asked, ["/Hushpen.AppImage"]);
}

#[test]
fn a_file_that_does_not_match_the_manifest_hash_ends_in_the_update_hash_code() {
    let body: &'static [u8] = b"the served bytes";
    let server = serve_file(body);
    let folder = tempfile::tempdir().unwrap();
    let wrong = artifact_for(&server, body, Some("0".repeat(64)));
    let error = download(&server, &wrong, folder.path()).unwrap_err();
    assert_eq!(error.code, UPDATE_HASH_MISMATCH);
    assert!(!folder.path().join("Hushpen.staged").exists());
    assert!(
        !folder.path().join("Hushpen.part").exists(),
        "the bad file is not kept"
    );
}

#[test]
fn a_file_of_another_size_than_the_manifest_says_is_a_hash_mismatch() {
    let body: &'static [u8] = b"the served bytes";
    let server = serve_file(body);
    let folder = tempfile::tempdir().unwrap();
    let mut artifact = artifact_for(&server, body, None);
    artifact.size += 5;
    let error = download(&server, &artifact, folder.path()).unwrap_err();
    assert_eq!(error.code, UPDATE_HASH_MISMATCH);
}

#[test]
fn a_download_from_a_host_off_the_feed_address_is_refused_with_the_download_code() {
    let body: &'static [u8] = b"x";
    let server = serve_file(body);
    let folder = tempfile::tempdir().unwrap();
    let mut artifact = artifact_for(&server, body, None);
    artifact.url = "https://github.com/other/Hushpen.AppImage".into();
    let error = download(&server, &artifact, folder.path()).unwrap_err();
    assert_eq!(error.code, UPDATE_DOWNLOAD_FAILED);
    assert!(server.seen().is_empty());
}

#[test]
fn a_server_error_is_a_download_failure() {
    let server = TestServer::start(|_: &Req, stream: &mut TcpStream| {
        respond(stream, "500 Internal Server Error", &[], b"");
    });
    let folder = tempfile::tempdir().unwrap();
    let artifact = artifact_for(&server, b"abc", None);
    let error = download(&server, &artifact, folder.path()).unwrap_err();
    assert_eq!(error.code, UPDATE_DOWNLOAD_FAILED);
}

#[test]
fn the_embedded_key_is_a_valid_minisign_key() {
    assert!(PublicKey::decode(EMBEDDED_KEY).is_ok());
}

#[test]
fn the_release_feed_is_the_github_latest_asset_over_https() {
    let policy = UpdatePolicy::release();
    let start = policy.check_start(FEED_URL).unwrap();
    assert_eq!(start.host, "github.com");
    assert!(policy.https_only());
}
