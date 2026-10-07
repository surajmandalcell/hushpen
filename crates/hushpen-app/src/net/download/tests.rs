use super::*;
use crate::net::fetch::UreqFetcher;
use crate::net::test_server::{TestServer, respond, respond_with_range};
use hushpen_store::model_files::{Check, check, stamp_path};
use std::io::Cursor;
use std::sync::{Arc, Mutex};

fn payload(len: usize) -> Vec<u8> {
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 32) as u8
        })
        .collect()
}

fn sha_of(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn https(host: &str, path: &str) -> String {
    format!("https://{host}{path}")
}

fn http(host: &str, path: &str) -> String {
    format!("http://{host}{path}")
}

struct Folders {
    _root: tempfile::TempDir,
    partial: PathBuf,
    dest: PathBuf,
}

impl Folders {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        Self {
            partial: root.path().join("cache/downloads/ggml-t.bin.download"),
            dest: root.path().join("models/whisper/ggml-t.bin"),
            _root: root,
        }
    }

    fn spec(&self, url: String, body: &[u8]) -> Spec {
        self.spec_with_hash(url, body, sha_of(body))
    }

    fn spec_with_hash(&self, url: String, body: &[u8], sha256: String) -> Spec {
        Spec {
            url,
            bytes: body.len() as u64,
            sha256,
            partial: self.partial.clone(),
            dest: self.dest.clone(),
        }
    }

    fn nothing_is_left(&self) -> bool {
        !self.partial.exists() && !self.dest.exists() && !stamp_path(&self.dest).exists()
    }
}

type Script = dyn Fn(&str, Option<u64>) -> Result<Response, FetchError> + Send + Sync;

/// A fetcher that answers from a function and remembers every URL it was asked for.
struct Scripted {
    script: Box<Script>,
    asked: Arc<Mutex<Vec<String>>>,
}

impl Scripted {
    fn boxed(
        script: impl Fn(&str, Option<u64>) -> Result<Response, FetchError> + Send + Sync + 'static,
    ) -> (Box<dyn Fetcher>, Arc<Mutex<Vec<String>>>) {
        let asked = Arc::new(Mutex::new(Vec::new()));
        let fetcher = Self {
            script: Box::new(script),
            asked: Arc::clone(&asked),
        };
        (Box::new(fetcher), asked)
    }
}

impl Fetcher for Scripted {
    fn get(&self, url: &str, range_from: Option<u64>) -> Result<Response, FetchError> {
        self.asked.lock().unwrap().push(url.to_owned());
        (self.script)(url, range_from)
    }
}

fn answer(status: u16, location: Option<String>, body: &[u8]) -> Response {
    Response {
        status,
        location,
        content_length: Some(body.len() as u64),
        range_start: None,
        body: Box::new(Cursor::new(body.to_vec())),
    }
}

fn redirect_to(url: String) -> Response {
    answer(302, Some(url), b"")
}

fn run(downloader: &Downloader, spec: &Spec) -> Result<Outcome, Failure> {
    downloader.run(spec, &AtomicBool::new(false), &mut |_| {})
}

fn hf_downloader(fetcher: Box<dyn Fetcher>) -> Downloader {
    Downloader::new(Policy::model_download(), fetcher)
}

fn local_downloader() -> Downloader {
    let policy = Policy::loopback();
    Downloader::new(policy, Box::new(UreqFetcher::new(&policy)))
}

const START_PATH: &str = "/ggerganov/whisper.cpp/resolve/main/ggml-t.bin";

#[test]
fn blocked_start_urls_are_refused_before_any_request_and_leave_no_file() {
    let body = payload(1000);
    for url in [
        http("huggingface.co", START_PATH),
        https("example.com", START_PATH),
        https("huggingface.co.evil.example", START_PATH),
        https("cdn.hf.co", START_PATH),
    ] {
        let folders = Folders::new();
        let (fetcher, asked) = Scripted::boxed(|_, _| Err(FetchError("must not connect".into())));
        let failure = run(&hf_downloader(fetcher), &folders.spec(url.clone(), &body)).unwrap_err();
        assert_eq!(failure.code, MODEL_HOST_BLOCKED, "{url}");
        assert!(
            asked.lock().unwrap().is_empty(),
            "{url}: no connection opens"
        );
        assert!(folders.nothing_is_left(), "{url}");
    }
}

#[test]
fn a_redirect_to_a_hugging_face_cdn_host_is_followed_and_the_file_is_verified() {
    let body = payload(5000);
    for cdn in ["cas-bridge.xethub.hf.co", "us.aws.cdn.hf.co"] {
        let folders = Folders::new();
        let target = https(cdn, "/repos/blob?sig=abc");
        let (fetcher, asked) = Scripted::boxed({
            let (body, target) = (body.clone(), target.clone());
            move |url, _| {
                Ok(if url == target {
                    answer(200, None, &body)
                } else {
                    redirect_to(target.clone())
                })
            }
        });
        let spec = folders.spec(https("huggingface.co", START_PATH), &body);
        assert_eq!(
            run(&hf_downloader(fetcher), &spec),
            Ok(Outcome::Done),
            "{cdn}"
        );
        assert_eq!(asked.lock().unwrap().len(), 2);
        assert_eq!(fs::read(&folders.dest).unwrap(), body);
        assert!(!folders.partial.exists());
        let stamp = model_files::read_stamp(&folders.dest).unwrap();
        assert_eq!((stamp.hash, stamp.size), (sha_of(&body), body.len() as u64));
        assert_eq!(check(&folders.dest, spec.bytes, &spec.sha256), Check::Ready);
    }
}

#[test]
fn a_redirect_to_a_forbidden_target_is_refused_before_a_connection_opens() {
    let body = payload(1000);
    for target in [
        http("us.aws.cdn.hf.co", "/blob"),
        https("example.com", "/blob"),
        https("huggingface.co.evil.example", "/blob"),
        https("hf.co", "/blob"),
    ] {
        let folders = Folders::new();
        let (fetcher, asked) = Scripted::boxed({
            let target = target.clone();
            move |_, _| Ok(redirect_to(target.clone()))
        });
        let spec = folders.spec(https("huggingface.co", START_PATH), &body);
        let failure = run(&hf_downloader(fetcher), &spec).unwrap_err();
        assert_eq!(failure.code, MODEL_HOST_BLOCKED, "{target}");
        assert_eq!(
            *asked.lock().unwrap(),
            std::slice::from_ref(&spec.url),
            "{target}: only the start URL was asked for"
        );
        assert!(folders.nothing_is_left(), "{target}");
    }
}

#[test]
fn eight_redirects_are_followed_and_a_ninth_is_refused() {
    let body = payload(1000);
    for (hops, ok) in [(8, true), (9, false)] {
        let folders = Folders::new();
        let (fetcher, asked) = Scripted::boxed({
            let body = body.clone();
            move |url, _| {
                let step = url
                    .split("hop")
                    .nth(1)
                    .and_then(|n| n.parse::<usize>().ok())
                    .unwrap_or(0);
                Ok(if step >= hops {
                    answer(200, None, &body)
                } else {
                    redirect_to(https("cdn.hf.co", &format!("/hop{}", step + 1)))
                })
            }
        });
        let spec = folders.spec(https("huggingface.co", START_PATH), &body);
        let result = run(&hf_downloader(fetcher), &spec);
        if ok {
            assert_eq!(result, Ok(Outcome::Done));
        } else {
            assert_eq!(result.unwrap_err().code, MODEL_HOST_BLOCKED);
            assert_eq!(asked.lock().unwrap().len(), MAX_REDIRECTS + 1);
            assert!(folders.nothing_is_left());
        }
    }
}

#[test]
fn a_relative_redirect_stays_on_the_start_host() {
    let body = payload(1000);
    let folders = Folders::new();
    let (fetcher, asked) = Scripted::boxed({
        let body = body.clone();
        move |url, _| {
            Ok(if url.ends_with("/real.bin") {
                answer(200, None, &body)
            } else {
                answer(302, Some("/real.bin".into()), b"")
            })
        }
    });
    let spec = folders.spec(https("huggingface.co", START_PATH), &body);
    assert_eq!(run(&hf_downloader(fetcher), &spec), Ok(Outcome::Done));
    assert_eq!(
        asked.lock().unwrap()[1],
        https("huggingface.co", "/real.bin")
    );
}

#[test]
fn a_wrong_hash_ends_in_hash_mismatch_with_no_file_and_no_stamp() {
    let body = payload(200_000);
    let server = TestServer::start({
        let body = body.clone();
        move |request, stream| respond_with_range(stream, request, &body)
    });
    let folders = Folders::new();
    let spec = folders.spec_with_hash(server.url("/m.bin"), &body, "0".repeat(64));
    let failure = run(&local_downloader(), &spec).unwrap_err();
    assert_eq!(failure.code, MODEL_HASH_MISMATCH);
    assert!(
        folders.nothing_is_left(),
        "no model, stamp, or partial file"
    );
}

#[test]
fn a_server_file_of_another_size_is_a_mismatch_found_before_the_body() {
    let body = payload(10_000);
    let server = TestServer::start({
        let body = body.clone();
        move |request, stream| respond_with_range(stream, request, &body)
    });
    let folders = Folders::new();
    let mut spec = folders.spec(server.url("/m.bin"), &body);
    spec.bytes += 5;
    let failure = run(&local_downloader(), &spec).unwrap_err();
    assert_eq!(failure.code, MODEL_HASH_MISMATCH);
    assert!(folders.nothing_is_left());
}

#[test]
fn a_full_download_reports_rising_progress_and_stamps_the_file() {
    let body = payload(3 * 1024 * 1024);
    let server = TestServer::start({
        let body = body.clone();
        move |request, stream| respond_with_range(stream, request, &body)
    });
    let folders = Folders::new();
    let spec = folders.spec(server.url("/m.bin"), &body);
    let mut seen = Vec::new();
    let result =
        local_downloader().run(&spec, &AtomicBool::new(false), &mut |step| seen.push(step));
    assert_eq!(result, Ok(Outcome::Done));

    let bytes: Vec<u64> = seen
        .iter()
        .filter_map(|step| match step {
            Progress::Bytes { done, .. } => Some(*done),
            Progress::Verifying => None,
        })
        .collect();
    assert!(bytes.windows(2).all(|pair| pair[0] <= pair[1]), "{bytes:?}");
    assert_eq!(bytes.first(), Some(&0));
    assert_eq!(bytes.last(), Some(&(body.len() as u64)));
    assert_eq!(seen.last(), Some(&Progress::Verifying));
    assert_eq!(fs::read(&folders.dest).unwrap(), body);
    assert!(!folders.partial.exists());
    assert_eq!(check(&folders.dest, spec.bytes, &spec.sha256), Check::Ready);
}

#[test]
fn a_cancel_settles_fast_and_leaves_no_partial_file() {
    let body = payload(50_000);
    let server = TestServer::start({
        let body = body.clone();
        move |_, stream| {
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                body.len() * 100
            );
            let _ = std::io::Write::write_all(stream, head.as_bytes());
            for _ in 0..100 {
                if std::io::Write::write_all(stream, &body).is_err() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    });
    let folders = Folders::new();
    let mut spec = folders.spec(server.url("/m.bin"), &body);
    spec.bytes = (body.len() * 100) as u64;
    let cancel = Arc::new(AtomicBool::new(false));
    let worker = {
        let (cancel, spec) = (Arc::clone(&cancel), spec.clone());
        std::thread::spawn(move || local_downloader().run(&spec, &cancel, &mut |_| {}))
    };
    let wait = Instant::now();
    while !folders.partial.exists() || fs::metadata(&folders.partial).unwrap().len() == 0 {
        assert!(
            wait.elapsed() < Duration::from_secs(5),
            "the download never started"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let asked = Instant::now();
    cancel.store(true, Ordering::Relaxed);
    assert_eq!(worker.join().unwrap(), Ok(Outcome::Cancelled));
    assert!(
        asked.elapsed() < Duration::from_secs(2),
        "{:?}",
        asked.elapsed()
    );
    assert!(folders.nothing_is_left());
}

#[test]
fn a_dropped_connection_keeps_the_partial_file_and_the_next_run_resumes_with_a_range() {
    let body = payload(400_000);
    let cut = 150_000;
    let first = Arc::new(AtomicBool::new(true));
    let server = TestServer::start({
        let (body, first) = (body.clone(), Arc::clone(&first));
        move |request, stream| {
            if first.swap(false, Ordering::SeqCst) {
                let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
                let _ = std::io::Write::write_all(stream, head.as_bytes());
                let _ = std::io::Write::write_all(stream, &body[..cut]);
                return;
            }
            respond_with_range(stream, request, &body);
        }
    });
    let folders = Folders::new();
    let spec = folders.spec(server.url("/m.bin"), &body);
    let failure = run(&local_downloader(), &spec).unwrap_err();
    assert_eq!(failure.code, DOWNLOAD_FAILED);
    let kept = fs::metadata(&folders.partial).unwrap().len();
    assert!(kept > 0 && kept <= cut as u64);
    assert!(!folders.dest.exists());

    assert_eq!(run(&local_downloader(), &spec), Ok(Outcome::Done));
    let requests = server.seen();
    assert_eq!(requests.last().unwrap().range_from, Some(kept));
    assert_eq!(fs::read(&folders.dest).unwrap(), body);
    assert!(!folders.partial.exists());
}

#[test]
fn a_partial_file_left_by_a_killed_app_is_resumed() {
    let body = payload(300_000);
    let server = TestServer::start({
        let body = body.clone();
        move |request, stream| respond_with_range(stream, request, &body)
    });
    let folders = Folders::new();
    fs::create_dir_all(folders.partial.parent().unwrap()).unwrap();
    fs::write(&folders.partial, &body[..120_000]).unwrap();
    let spec = folders.spec(server.url("/m.bin"), &body);
    assert_eq!(run(&local_downloader(), &spec), Ok(Outcome::Done));
    assert_eq!(server.seen()[0].range_from, Some(120_000));
    assert_eq!(server.seen()[0].path, "/m.bin");
    assert_eq!(fs::read(&folders.dest).unwrap(), body);
}

#[test]
fn a_server_that_ignores_the_range_restarts_the_file_clean() {
    let body = payload(300_000);
    let server = TestServer::start({
        let body = body.clone();
        move |_, stream| respond(stream, "200 OK", &[], &body)
    });
    let folders = Folders::new();
    fs::create_dir_all(folders.partial.parent().unwrap()).unwrap();
    fs::write(&folders.partial, &body[..100_000]).unwrap();
    let spec = folders.spec(server.url("/m.bin"), &body);
    assert_eq!(run(&local_downloader(), &spec), Ok(Outcome::Done));
    assert_eq!(fs::read(&folders.dest).unwrap(), body);
}

#[test]
fn a_server_that_answers_416_gets_a_clean_second_request() {
    let body = payload(200_000);
    let server = TestServer::start({
        let body = body.clone();
        move |request, stream| {
            if request.range_from.is_some() {
                respond(stream, "416 Range Not Satisfiable", &[], b"");
            } else {
                respond(stream, "200 OK", &[], &body);
            }
        }
    });
    let folders = Folders::new();
    fs::create_dir_all(folders.partial.parent().unwrap()).unwrap();
    fs::write(&folders.partial, &body[..50_000]).unwrap();
    let spec = folders.spec(server.url("/m.bin"), &body);
    assert_eq!(run(&local_downloader(), &spec), Ok(Outcome::Done));
    let ranges: Vec<_> = server.seen().iter().map(|r| r.range_from).collect();
    assert_eq!(ranges, [Some(50_000), None]);
}

#[test]
fn a_corrupt_partial_file_fails_the_hash_and_is_removed_so_the_next_run_starts_clean() {
    let body = payload(250_000);
    let server = TestServer::start({
        let body = body.clone();
        move |request, stream| respond_with_range(stream, request, &body)
    });
    let folders = Folders::new();
    fs::create_dir_all(folders.partial.parent().unwrap()).unwrap();
    let mut bad = body[..100_000].to_vec();
    bad[500] ^= 0xFF;
    fs::write(&folders.partial, &bad).unwrap();
    let spec = folders.spec(server.url("/m.bin"), &body);

    assert_eq!(
        run(&local_downloader(), &spec).unwrap_err().code,
        MODEL_HASH_MISMATCH
    );
    assert!(folders.nothing_is_left());
    assert_eq!(run(&local_downloader(), &spec), Ok(Outcome::Done));
    assert_eq!(fs::read(&folders.dest).unwrap(), body);
}

#[test]
fn a_full_size_partial_file_is_hashed_without_a_request() {
    let body = payload(80_000);
    let (fetcher, asked) = Scripted::boxed(|_, _| Err(FetchError("must not connect".into())));
    let folders = Folders::new();
    fs::create_dir_all(folders.partial.parent().unwrap()).unwrap();
    fs::write(&folders.partial, &body).unwrap();
    let spec = folders.spec(https("huggingface.co", START_PATH), &body);
    assert_eq!(run(&hf_downloader(fetcher), &spec), Ok(Outcome::Done));
    assert!(asked.lock().unwrap().is_empty());
    assert_eq!(fs::read(&folders.dest).unwrap(), body);
}

#[test]
fn a_stalled_server_fails_after_the_idle_time_and_keeps_the_partial_file() {
    let body = payload(60_000);
    let server = TestServer::start({
        let body = body.clone();
        move |_, stream| {
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                body.len() * 2
            );
            let _ = std::io::Write::write_all(stream, head.as_bytes());
            let _ = std::io::Write::write_all(stream, &body);
            std::thread::sleep(Duration::from_secs(3));
        }
    });
    let folders = Folders::new();
    let mut spec = folders.spec(server.url("/m.bin"), &body);
    spec.bytes = (body.len() * 2) as u64;
    let downloader = local_downloader().with_idle(Duration::from_millis(400));
    let started = Instant::now();
    let failure = run(&downloader, &spec).unwrap_err();
    assert_eq!(failure.code, DOWNLOAD_FAILED);
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(folders.partial.exists());
}

#[test]
fn an_http_error_status_is_a_download_failure_and_leaves_no_file() {
    let body = payload(1000);
    let server = TestServer::start(|_, stream| respond(stream, "404 Not Found", &[], b"no"));
    let folders = Folders::new();
    let failure = run(
        &local_downloader(),
        &folders.spec(server.url("/m.bin"), &body),
    )
    .unwrap_err();
    assert_eq!(failure.code, DOWNLOAD_FAILED);
    assert!(folders.nothing_is_left());
}
