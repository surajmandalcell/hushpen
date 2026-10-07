//! Which hosts a model download may talk to.
//!
//! A download starts on `huggingface.co` over HTTPS. Hugging Face serves the file bytes from
//! its own CDN, so a redirect may go to `huggingface.co` or to a `*.hf.co` host, again over
//! HTTPS. Every other host, plain HTTP, a user name in the URL, and an odd port are refused
//! before any connection opens.

use hushpen_core::error::MODEL_HOST_BLOCKED;

/// Redirects a download follows before it gives up.
pub const MAX_REDIRECTS: usize = 8;

const START_HOST: &str = "huggingface.co";
const CDN_SUFFIX: &str = ".hf.co";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blocked {
    pub code: &'static str,
    /// The refused host, or `-` when the URL did not parse.
    pub host: String,
    /// Host and cause only. A download URL can carry a signed token, so it never goes here.
    pub reason: String,
}

impl Blocked {
    pub fn new(host: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            code: MODEL_HOST_BLOCKED,
            host: host.into(),
            reason: reason.into(),
        }
    }

    fn invalid(what: &str) -> Self {
        Self::new("-", format!("the {what} URL is not valid"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scheme {
    Http,
    Https,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Parsed {
    scheme: Scheme,
    host: String,
    port: Option<u16>,
}

/// A URL the policy accepted. Only the host is safe to show or log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    pub url: String,
    pub host: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    HuggingFace,
    /// A local test server, so the download code runs unchanged against a socket we own.
    #[cfg(test)]
    Loopback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    kind: Kind,
}

impl Policy {
    pub fn model_download() -> Self {
        Self {
            kind: Kind::HuggingFace,
        }
    }

    #[cfg(test)]
    pub fn loopback() -> Self {
        Self {
            kind: Kind::Loopback,
        }
    }

    /// Whether the HTTP client must refuse plain HTTP by itself as a second line of defense.
    pub fn https_only(&self) -> bool {
        matches!(self.kind, Kind::HuggingFace)
    }

    pub fn check_start(&self, url: &str) -> Result<Checked, Blocked> {
        let parsed = parse(url).ok_or_else(|| Blocked::invalid("download"))?;
        let allowed = match self.kind {
            Kind::HuggingFace => parsed.scheme == Scheme::Https && parsed.host == START_HOST,
            #[cfg(test)]
            Kind::Loopback => parsed.host == "127.0.0.1",
        };
        self.finish(url, parsed, allowed)
    }

    pub fn check_redirect(&self, url: &str) -> Result<Checked, Blocked> {
        let parsed = parse(url).ok_or_else(|| Blocked::invalid("redirect"))?;
        let allowed = match self.kind {
            Kind::HuggingFace => {
                parsed.scheme == Scheme::Https
                    && (parsed.host == START_HOST
                        || (parsed.host.len() > CDN_SUFFIX.len()
                            && parsed.host.ends_with(CDN_SUFFIX)))
            }
            #[cfg(test)]
            Kind::Loopback => parsed.host == "127.0.0.1",
        };
        self.finish(url, parsed, allowed)
    }

    fn finish(&self, url: &str, parsed: Parsed, allowed: bool) -> Result<Checked, Blocked> {
        let port_ok = match self.kind {
            Kind::HuggingFace => parsed.port.is_none_or(|port| port == 443),
            #[cfg(test)]
            Kind::Loopback => true,
        };
        if allowed && port_ok {
            return Ok(Checked {
                url: url.to_owned(),
                host: parsed.host,
            });
        }
        let scheme = match parsed.scheme {
            Scheme::Https => "https",
            Scheme::Http => "http",
        };
        Err(Blocked::new(
            parsed.host.clone(),
            format!("{scheme} to host {} is not allowed", parsed.host),
        ))
    }
}

fn parse(url: &str) -> Option<Parsed> {
    if !url.is_ascii()
        || url
            .bytes()
            .any(|b| b.is_ascii_control() || b == b' ' || b == b'\\')
    {
        return None;
    }
    let (scheme, rest) = if let Some(rest) = strip_prefix_ci(url, "https://") {
        (Scheme::Https, rest)
    } else {
        (Scheme::Http, strip_prefix_ci(url, "http://")?)
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.contains('@') {
        return None;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host, Some(port.parse::<u16>().ok()?)),
        None => (authority, None),
    };
    let host = host.to_ascii_lowercase();
    let valid = !host.is_empty()
        && !host.starts_with(['.', '-'])
        && !host.ends_with(['.', '-'])
        && !host.contains("..")
        && host
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-');
    valid.then_some(Parsed { scheme, host, port })
}

fn strip_prefix_ci<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

/// Turns a `Location` header into an absolute URL, relative to the URL that sent it.
pub fn resolve(base: &str, location: &str) -> Option<String> {
    if strip_prefix_ci(location, "https://").is_some()
        || strip_prefix_ci(location, "http://").is_some()
    {
        return Some(location.to_owned());
    }
    let scheme_end = base.find("://")? + 3;
    let authority_end = base[scheme_end..]
        .find(['/', '?', '#'])
        .map_or(base.len(), |at| scheme_end + at);
    if let Some(rest) = location.strip_prefix("//") {
        return Some(format!("{}//{rest}", &base[..scheme_end - 2]));
    }
    if location.starts_with('/') {
        return Some(format!("{}{location}", &base[..authority_end]));
    }
    let path_end = base[authority_end..]
        .find(['?', '#'])
        .map_or(base.len(), |at| authority_end + at);
    let directory_end = base[authority_end..path_end]
        .rfind('/')
        .map_or(authority_end, |at| authority_end + at);
    Some(format!("{}/{location}", &base[..directory_end]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn https(host: &str, path: &str) -> String {
        format!("https://{host}{path}")
    }

    fn http(host: &str, path: &str) -> String {
        format!("http://{host}{path}")
    }

    fn start(url: &str) -> Result<Checked, Blocked> {
        Policy::model_download().check_start(url)
    }

    fn redirect(url: &str) -> Result<Checked, Blocked> {
        Policy::model_download().check_redirect(url)
    }

    const FILE: &str = "/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin";

    #[test]
    fn a_download_starts_on_huggingface_over_https() {
        let ok = start(&https("huggingface.co", FILE)).unwrap();
        assert_eq!(ok.host, "huggingface.co");
        assert!(start(&https("HuggingFace.co", FILE)).is_ok(), "host case");
        assert!(start(&https("huggingface.co:443", FILE)).is_ok());
    }

    #[test]
    fn cdn_hosts_are_allowed_only_as_redirect_targets() {
        for host in [
            "cas-bridge.xethub.hf.co",
            "us.aws.cdn.hf.co",
            "huggingface.co",
        ] {
            assert!(redirect(&https(host, "/blob?token=abc")).is_ok(), "{host}");
        }
        let blocked = start(&https("us.aws.cdn.hf.co", FILE)).unwrap_err();
        assert_eq!(blocked.code, MODEL_HOST_BLOCKED);
    }

    #[test]
    fn plain_http_is_refused_at_the_start_and_in_a_redirect() {
        assert_eq!(
            start(&http("huggingface.co", FILE)).unwrap_err().code,
            MODEL_HOST_BLOCKED
        );
        assert_eq!(
            redirect(&http("cdn.hf.co", "/x")).unwrap_err().code,
            MODEL_HOST_BLOCKED
        );
    }

    #[test]
    fn other_hosts_are_refused_including_look_alikes() {
        for host in [
            "example.com",
            "huggingface.co.evil.example",
            "evil-huggingface.co",
            "xhf.co",
            "hf.co",
            "hf.co.evil.example",
            "github.com",
            "127.0.0.1",
            "localhost",
        ] {
            assert_eq!(
                start(&https(host, FILE)).unwrap_err().code,
                MODEL_HOST_BLOCKED,
                "{host}"
            );
            assert_eq!(
                redirect(&https(host, FILE)).unwrap_err().code,
                MODEL_HOST_BLOCKED,
                "{host}"
            );
        }
    }

    #[test]
    fn a_user_name_a_port_a_trailing_dot_or_an_odd_url_is_refused() {
        for url in [
            https("huggingface.co@evil.example", "/x"),
            https("evil.example@huggingface.co", "/x"),
            https("huggingface.co:8443", "/x"),
            https("huggingface.co.", "/x"),
            https("huggingface.co", "/x y"),
            "ftp://huggingface.co/x".to_string(),
            "huggingface.co/x".to_string(),
            "//huggingface.co/x".to_string(),
            String::new(),
            format!("https://{}/x", "huggingface.co\\@evil.example"),
        ] {
            assert!(start(&url).is_err(), "{url}");
        }
    }

    #[test]
    fn a_blocked_reason_names_the_host_and_never_the_query() {
        let blocked = redirect(&https("example.com", "/f?token=SECRET")).unwrap_err();
        assert!(blocked.reason.contains("example.com"));
        assert!(!blocked.reason.contains("SECRET"));
    }

    #[test]
    fn locations_resolve_against_the_url_that_sent_them() {
        let base = https("huggingface.co", "/a/b/resolve/main/f.bin?x=1");
        assert_eq!(
            resolve(&base, "/other/path").unwrap(),
            https("huggingface.co", "/other/path")
        );
        assert_eq!(
            resolve(&base, "g.bin").unwrap(),
            https("huggingface.co", "/a/b/resolve/main/g.bin")
        );
        assert_eq!(
            resolve(&base, "//cdn.hf.co/z").unwrap(),
            https("cdn.hf.co", "/z")
        );
        let absolute = https("cdn.hf.co", "/z?sig=1");
        assert_eq!(resolve(&base, &absolute).unwrap(), absolute);
    }

    #[test]
    fn a_relative_location_cannot_leave_the_host() {
        let base = https("huggingface.co", "/f");
        for location in ["//example.com/x".to_string(), https("example.com", "/x")] {
            let url = resolve(&base, &location).unwrap();
            assert!(redirect(&url).is_err(), "{location}");
        }
        let url = resolve(&base, "/ok").unwrap();
        assert!(redirect(&url).is_ok());
    }

    #[test]
    fn the_loopback_policy_exists_only_for_tests_and_takes_a_local_http_url() {
        let policy = Policy::loopback();
        assert!(
            policy
                .check_start(&http("127.0.0.1:8080", "/m.bin"))
                .is_ok()
        );
        assert!(policy.check_start(&https("example.com", "/m.bin")).is_err());
        assert!(!policy.https_only());
        assert!(Policy::model_download().https_only());
    }
}
