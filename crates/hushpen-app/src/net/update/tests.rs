use super::*;
use crate::net::test_server::{Req, TestServer, respond};
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
