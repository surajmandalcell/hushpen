//! The opt-in OpenAI-compatible endpoint: a connection test (`GET {url}/models`) and one chat
//! completion for AI cleanup. The URL is checked before a socket opens, redirects are never
//! followed, and every request ends in one [`record`](super::record) line with the host only.
//!
//! The API key goes into one header and nowhere else: not into a log line, an error text, or
//! the `Debug` output of [`Config`].

use super::{Purpose, record};
use hushpen_core::endpoint::{Target, UrlError, chat_body, check_url, host_of_url, reply_text};
use hushpen_core::error::ENDPOINT_FAILED;
use std::fmt;
use std::time::Duration;

/// A reply bigger than this is not a chat completion for a dictation.
const MAX_REPLY_BYTES: u64 = 256 * 1024;

#[derive(Clone)]
pub struct Config {
    pub url: String,
    pub model: String,
    pub key: Option<String>,
    /// The longest a whole request may take, from connect to the last byte.
    pub timeout: Duration,
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("model", &self.model)
            .field("key_set", &self.key.is_some())
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

/// How a request failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The URL was refused before any socket opened.
    Blocked(UrlError),
    Timeout,
    /// Nothing answered: refused, reset, or no route.
    Down,
    Status(u16),
    /// The endpoint answered, but not with a chat completion.
    BadReply,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub kind: Kind,
    /// The host the request was for, or `-` when none could be read.
    pub host: String,
}

impl Failure {
    pub fn code(&self) -> &'static str {
        ENDPOINT_FAILED
    }

    /// The words Settings shows next to the code. No URL, no key.
    pub fn reason(&self) -> String {
        match self.kind {
            Kind::Blocked(error) => error.reason().to_owned(),
            Kind::Timeout => "The endpoint did not answer in time.".to_owned(),
            Kind::Down => "The endpoint could not be reached.".to_owned(),
            Kind::Status(401 | 403) => "The endpoint refused the API key.".to_owned(),
            Kind::Status(status) => format!("The endpoint answered with HTTP {status}."),
            Kind::BadReply => "The endpoint did not send a chat completion.".to_owned(),
        }
    }

    /// The result column of `hookctl net`.
    fn result(&self) -> String {
        match self.kind {
            Kind::Blocked(UrlError::NeedsHttps) => "blocked: https is needed".to_owned(),
            Kind::Blocked(UrlError::Empty) => "blocked: no url".to_owned(),
            Kind::Blocked(UrlError::Invalid) => "blocked: invalid url".to_owned(),
            Kind::Timeout => "failed: timeout".to_owned(),
            Kind::Down => "failed: unreachable".to_owned(),
            Kind::Status(status) => format!("failed: http {status}"),
            Kind::BadReply => "failed: bad reply".to_owned(),
        }
    }
}

/// What talks to the endpoint. The real one is [`UreqTransport`]; tests pass a fake.
pub trait Transport: Send + Sync {
    /// `GET {url}/models`: whether the endpoint answers.
    fn test_connection(&self, config: &Config) -> Result<(), Failure>;

    /// One chat completion with the cleanup prompt. `user` is already wrapped.
    fn complete(
        &self,
        config: &Config,
        system: &str,
        user: &str,
        max_tokens: u32,
    ) -> Result<String, Failure>;
}

pub struct UreqTransport;

impl Transport for UreqTransport {
    fn test_connection(&self, config: &Config) -> Result<(), Failure> {
        exchange(config, |target, agent| {
            let mut request = agent.get(target.models_url());
            if let Some(key) = &config.key {
                request = request.header("Authorization", format!("Bearer {key}"));
            }
            let response = request.call().map_err(|error| kind_of(&error))?;
            ok_status(response.status().as_u16())
        })
    }

    fn complete(
        &self,
        config: &Config,
        system: &str,
        user: &str,
        max_tokens: u32,
    ) -> Result<String, Failure> {
        let body = chat_body(&config.model, system, user, max_tokens);
        exchange(config, |target, agent| {
            let mut request = agent
                .post(target.chat_url())
                .header("Content-Type", "application/json");
            if let Some(key) = &config.key {
                request = request.header("Authorization", format!("Bearer {key}"));
            }
            let mut response = request.send(body).map_err(|error| kind_of(&error))?;
            ok_status(response.status().as_u16())?;
            let text = response
                .body_mut()
                .with_config()
                .limit(MAX_REPLY_BYTES)
                .read_to_string()
                .map_err(|error| kind_of(&error))?;
            reply_text(&text).ok_or(Kind::BadReply)
        })
    }
}

fn ok_status(status: u16) -> Result<(), Kind> {
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(Kind::Status(status))
    }
}

fn kind_of(error: &ureq::Error) -> Kind {
    match error {
        ureq::Error::Timeout(_) => Kind::Timeout,
        _ => Kind::Down,
    }
}

/// Checks the URL, runs `call` on a fresh agent, and records the result.
fn exchange<T>(
    config: &Config,
    call: impl FnOnce(&Target, &ureq::Agent) -> Result<T, Kind>,
) -> Result<T, Failure> {
    let target = match check_url(&config.url) {
        Ok(target) => target,
        Err(error) => {
            let failure = Failure {
                kind: Kind::Blocked(error),
                host: host_of_url(&config.url).unwrap_or_else(|| "-".to_owned()),
            };
            settle(&failure);
            return Err(failure);
        }
    };
    let agent = agent(&target, config.timeout);
    match call(&target, &agent) {
        Ok(value) => {
            record(Purpose::Endpoint, &target.host, "ok");
            Ok(value)
        }
        Err(kind) => {
            let failure = Failure {
                kind,
                host: target.host,
            };
            settle(&failure);
            Err(failure)
        }
    }
}

fn settle(failure: &Failure) {
    let result = failure.result();
    record(Purpose::Endpoint, &failure.host, &result);
    log::warn!("{ENDPOINT_FAILED} host={} {result}", failure.host);
}

fn agent(target: &Target, timeout: Duration) -> ureq::Agent {
    // Purpose::Endpoint. Host: the one the user typed (user-configured), plain http only for
    // loopback. A proxy from the environment would make the connect go to another host.
    let config = ureq::Agent::config_builder()
        .max_redirects(0)
        .http_status_as_error(false)
        .https_only(target.secure)
        .proxy(None)
        .timeout_global(Some(timeout))
        .user_agent(format!("Hushpen/{}", hushpen_core::BUILD_VERSION))
        .build();
    ureq::Agent::new_with_config(config)
}

#[cfg(test)]
mod tests {
    use super::super::test_server::{Req, TestServer, respond};
    use super::*;
    use std::net::TcpStream;
    use std::time::Instant;

    /// Built from parts so the network audit does not read a test host as a call site.
    fn plain(host: &str) -> String {
        format!("http://{host}/v1")
    }

    fn config(url: String) -> Config {
        Config {
            url,
            model: "stub-model".to_owned(),
            key: Some("sk-test-123".to_owned()),
            timeout: Duration::from_secs(2),
        }
    }

    fn chat_reply(text: &str) -> String {
        serde_json::json!({"choices": [{"message": {"role": "assistant", "content": text}}]})
            .to_string()
    }

    fn answer(request: &Req, stream: &mut TcpStream) {
        if request.path.ends_with("/models") {
            respond(stream, "200 OK", &[], br#"{"data":[]}"#);
        } else {
            respond(
                stream,
                "200 OK",
                &[("Content-Type", "application/json".to_owned())],
                chat_reply("Meet on Wednesday.").as_bytes(),
            );
        }
    }

    #[test]
    fn a_connection_test_gets_models_with_the_bearer_key() {
        let server = TestServer::start(answer);
        let transport = UreqTransport;
        transport
            .test_connection(&config(server.url("/v1")))
            .unwrap();
        let seen = server.seen();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].method, "GET");
        assert_eq!(seen[0].path, "/v1/models");
        assert_eq!(seen[0].header("authorization"), Some("Bearer sk-test-123"));
        assert_eq!(
            seen[0].header("user-agent"),
            Some(format!("Hushpen/{}", hushpen_core::BUILD_VERSION).as_str())
        );
    }

    #[test]
    fn a_cleanup_posts_the_model_the_wrapped_text_and_the_key() {
        let server = TestServer::start(answer);
        let text = UreqTransport
            .complete(
                &config(server.url("/v1")),
                "system words",
                "<transcript>um hello</transcript>",
                64,
            )
            .unwrap();
        assert_eq!(text, "Meet on Wednesday.");
        let seen = server.seen();
        assert_eq!(seen[0].method, "POST");
        assert_eq!(seen[0].path, "/v1/chat/completions");
        assert_eq!(seen[0].header("authorization"), Some("Bearer sk-test-123"));
        let body: serde_json::Value = serde_json::from_str(&seen[0].body).unwrap();
        assert_eq!(body["model"], "stub-model");
        assert_eq!(body["temperature"], 0);
        assert_eq!(
            body["messages"][1]["content"],
            "<transcript>um hello</transcript>"
        );
    }

    #[test]
    fn no_key_means_no_authorization_header() {
        let server = TestServer::start(answer);
        let mut config = config(server.url("/v1"));
        config.key = None;
        UreqTransport.test_connection(&config).unwrap();
        assert_eq!(server.seen()[0].header("authorization"), None);
    }

    #[test]
    fn a_url_that_needs_https_is_refused_before_any_request() {
        let server = TestServer::start(answer);
        let failure = UreqTransport
            .test_connection(&config(plain("10.0.0.2:11434")))
            .unwrap_err();
        assert_eq!(failure.kind, Kind::Blocked(UrlError::NeedsHttps));
        assert_eq!(failure.host, "10.0.0.2");
        assert_eq!(failure.code(), ENDPOINT_FAILED);
        assert!(failure.reason().contains("https"));
        assert!(!failure.reason().contains("10.0.0.2"));
        assert!(server.seen().is_empty());
    }

    #[test]
    fn an_empty_or_odd_url_is_refused_too() {
        for url in ["", "localhost:11434"] {
            let failure = UreqTransport
                .complete(&config(url.to_owned()), "s", "u", 8)
                .unwrap_err();
            assert!(matches!(failure.kind, Kind::Blocked(_)), "{url}");
        }
    }

    #[test]
    fn a_port_with_nothing_behind_it_is_down() {
        let port = {
            let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
            listener.local_addr().unwrap().port()
        };
        let failure = UreqTransport
            .test_connection(&config(format!("http://127.0.0.1:{port}/v1")))
            .unwrap_err();
        assert_eq!(failure.kind, Kind::Down);
        assert_eq!(failure.host, "127.0.0.1");
    }

    #[test]
    fn a_slow_endpoint_times_out_at_the_configured_time() {
        let server = TestServer::start(|_, stream| {
            std::thread::sleep(Duration::from_secs(5));
            respond(stream, "200 OK", &[], b"{}");
        });
        let mut config = config(server.url("/v1"));
        config.timeout = Duration::from_millis(400);
        let began = Instant::now();
        let failure = UreqTransport.complete(&config, "s", "u", 8).unwrap_err();
        assert_eq!(failure.kind, Kind::Timeout);
        assert!(
            began.elapsed() < Duration::from_secs(2),
            "{:?}",
            began.elapsed()
        );
    }

    #[test]
    fn a_bad_status_a_bad_reply_and_a_redirect_are_failures() {
        let server = TestServer::start(|request, stream| match request.path.as_str() {
            "/v1/models" => respond(stream, "401 Unauthorized", &[], b"{}"),
            "/v1/chat/completions" => respond(stream, "200 OK", &[], b"not json"),
            _ => respond(
                stream,
                "302 Found",
                &[("Location", "http://127.0.0.1:1/".to_owned())],
                b"",
            ),
        });
        let config = config(server.url("/v1"));
        assert_eq!(
            UreqTransport.test_connection(&config).unwrap_err().kind,
            Kind::Status(401)
        );
        assert_eq!(
            UreqTransport
                .complete(&config, "s", "u", 8)
                .unwrap_err()
                .kind,
            Kind::BadReply
        );
        let mut elsewhere = config.clone();
        elsewhere.url = server.url("/other");
        assert_eq!(
            UreqTransport.test_connection(&elsewhere).unwrap_err().kind,
            Kind::Status(302)
        );
    }

    #[test]
    fn the_key_never_shows_in_a_debug_print_or_a_reason() {
        let config = config("http://localhost:1/v1".to_owned());
        assert!(!format!("{config:?}").contains("sk-test-123"));
        for kind in [
            Kind::Timeout,
            Kind::Down,
            Kind::Status(401),
            Kind::BadReply,
            Kind::Blocked(UrlError::NeedsHttps),
        ] {
            let failure = Failure {
                kind,
                host: "localhost".to_owned(),
            };
            assert!(!failure.reason().contains("sk-test"));
            assert!(!failure.result().contains("sk-test"));
        }
    }

    #[cfg(feature = "test-automation")]
    #[test]
    fn every_request_lands_in_the_network_log_with_purpose_host_and_result() {
        use hushpen_testhook::logs::net_entries;
        let server = TestServer::start(answer);
        UreqTransport
            .test_connection(&config(server.url("/v1")))
            .unwrap();
        let _ = UreqTransport.test_connection(&config(plain("10.0.0.7")));
        let entries = net_entries();
        assert!(
            entries
                .iter()
                .any(|e| e.purpose == "endpoint" && e.host == "127.0.0.1" && e.result == "ok")
        );
        assert!(entries.iter().any(|e| e.purpose == "endpoint"
            && e.host == "10.0.0.7"
            && e.result == "blocked: https is needed"));
    }
}
