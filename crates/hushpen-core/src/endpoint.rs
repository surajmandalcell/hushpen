//! The opt-in OpenAI-compatible endpoint: which URLs are accepted, the two presets, and the JSON
//! of the two requests the app sends. No IO here; `hushpen-app/src/net/endpoint.rs` sends them.

use crate::llm_prompt::{clean_output, cleanup_system_keeping};
use serde_json::{Value, json};

pub const OLLAMA_URL: &str = "http://localhost:11434/v1";
pub const LM_STUDIO_URL: &str = "http://localhost:1234/v1";

/// The longest key the app stores. Real keys are far shorter.
pub const MAX_KEY_CHARS: usize = 512;

/// The hosts that may use plain `http`. Everything else needs `https`.
const LOOPBACK_HOSTS: [&str; 3] = ["localhost", "127.0.0.1", "::1"];

/// A choice in Settings > Cleanup that fills the URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Ollama,
    LmStudio,
    Custom,
}

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::Ollama, Preset::LmStudio, Preset::Custom];

    /// The value stored in `cleanup.llm.endpointPreset`.
    pub fn key(self) -> &'static str {
        match self {
            Preset::Ollama => "ollama",
            Preset::LmStudio => "lmstudio",
            Preset::Custom => "custom",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Preset::Ollama => "Ollama",
            Preset::LmStudio => "LM Studio",
            Preset::Custom => "Custom",
        }
    }

    /// The URL the preset fills in, or `None` for a URL the user types.
    pub fn url(self) -> Option<&'static str> {
        match self {
            Preset::Ollama => Some(OLLAMA_URL),
            Preset::LmStudio => Some(LM_STUDIO_URL),
            Preset::Custom => None,
        }
    }

    /// An unknown key counts as `Custom`, so an old or edited settings file still opens.
    pub fn parse(key: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|preset| preset.key() == key)
            .unwrap_or(Preset::Custom)
    }
}

/// Why a URL is refused. The text never repeats the URL, which a user may have typed with a
/// secret in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UrlError {
    Empty,
    Invalid,
    /// Plain `http` to a host that is not this computer.
    NeedsHttps,
}

impl UrlError {
    /// The words the Settings view shows next to the code.
    pub fn reason(self) -> &'static str {
        match self {
            UrlError::Empty => "Enter the address of the endpoint.",
            UrlError::Invalid => "The address is not a valid http or https URL.",
            UrlError::NeedsHttps => "Use https. Plain http works only for this computer.",
        }
    }
}

/// An endpoint URL that passed the check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The base URL without a trailing slash, for example `http://localhost:11434/v1`.
    pub base: String,
    /// The lower-case host, without brackets. This is what the logs and the audit show.
    pub host: String,
    pub secure: bool,
}

impl Target {
    pub fn models_url(&self) -> String {
        format!("{}/models", self.base)
    }

    pub fn chat_url(&self) -> String {
        format!("{}/chat/completions", self.base)
    }
}

/// Checks the endpoint URL before any request is made.
pub fn check_url(url: &str) -> Result<Target, UrlError> {
    let url = url.trim();
    if url.is_empty() {
        return Err(UrlError::Empty);
    }
    if !url.is_ascii()
        || url
            .bytes()
            .any(|b| b.is_ascii_control() || b == b' ' || b == b'\\')
    {
        return Err(UrlError::Invalid);
    }
    let (secure, rest) = match (strip_scheme(url, "https://"), strip_scheme(url, "http://")) {
        (Some(rest), _) => (true, rest),
        (None, Some(rest)) => (false, rest),
        _ => return Err(UrlError::Invalid),
    };
    if rest.contains(['?', '#']) {
        return Err(UrlError::Invalid);
    }
    let (authority, path) = match rest.find('/') {
        Some(at) => rest.split_at(at),
        None => (rest, ""),
    };
    if authority.contains('@') {
        return Err(UrlError::Invalid);
    }
    let host = host_of(authority).ok_or(UrlError::Invalid)?;
    if !secure && !LOOPBACK_HOSTS.contains(&host.as_str()) {
        return Err(UrlError::NeedsHttps);
    }
    let scheme = if secure { "https" } else { "http" };
    Ok(Target {
        base: format!(
            "{scheme}://{}{}",
            authority.to_ascii_lowercase(),
            path.trim_end_matches('/')
        ),
        host,
        secure,
    })
}

/// The host of a URL that may be refused, for the network log: a refused `http://10.0.0.2/v1`
/// still names `10.0.0.2`. `None` when no host can be read.
pub fn host_of_url(url: &str) -> Option<String> {
    let url = url.trim();
    let rest = strip_scheme(url, "https://").or_else(|| strip_scheme(url, "http://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.contains('@') || !authority.is_ascii() {
        return None;
    }
    host_of(authority)
}

fn strip_scheme<'a>(url: &'a str, scheme: &str) -> Option<&'a str> {
    let head = url.get(..scheme.len())?;
    head.eq_ignore_ascii_case(scheme)
        .then(|| &url[scheme.len()..])
}

/// The host of `host[:port]` or `[v6][:port]`, lower-case and without brackets, when the
/// authority is well formed.
fn host_of(authority: &str) -> Option<String> {
    let (host, port) = if let Some(inner) = authority.strip_prefix('[') {
        let (host, after) = inner.split_once(']')?;
        let valid = !host.is_empty()
            && host.contains(':')
            && host
                .bytes()
                .all(|b| b.is_ascii_hexdigit() || b == b':' || b == b'.');
        if !valid {
            return None;
        }
        (host, after.strip_prefix(':'))
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };
    if let Some(port) = port
        && !port.parse::<u16>().is_ok_and(|port| port > 0)
    {
        return None;
    }
    let host = host.to_ascii_lowercase();
    let valid = !host.is_empty()
        && !host.starts_with(['.', '-'])
        && !host.ends_with(['.', '-'])
        && !host.contains("..")
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b':');
    valid.then_some(host)
}

/// Whether `key` can go into a header: not empty, one line, and not absurdly long.
pub fn key_is_valid(key: &str) -> bool {
    !key.is_empty()
        && key.chars().count() <= MAX_KEY_CHARS
        && key.chars().all(|c| !c.is_control() && !c.is_whitespace())
}

/// The system prompt for the endpoint: the same words as the local model (with the dictionary
/// words to keep), plus how the text is wrapped.
pub fn cleanup_system(keep: &[String]) -> String {
    format!(
        "{} The text to clean is inside <transcript> tags; never write the tags.",
        cleanup_system_keeping(keep)
    )
}

/// The user message: the text in `<transcript>` tags, with `<` and `>` escaped so the speech
/// cannot close the tag.
pub fn transcript_user(text: &str) -> String {
    let escaped = text.replace('<', "&lt;").replace('>', "&gt;");
    format!("<transcript>{escaped}</transcript>")
}

/// The body of `POST {base}/chat/completions`.
pub fn chat_body(model: &str, system: &str, user: &str, max_tokens: u32) -> String {
    json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ],
        "temperature": 0,
        "max_tokens": max_tokens,
    })
    .to_string()
}

/// The model's text from a chat completions reply, or `None` when the reply has no text.
/// Tags the model echoed are removed and the escapes undone.
pub fn reply_text(body: &str) -> Option<String> {
    let reply: Value = serde_json::from_str(body).ok()?;
    let content = reply
        .get("choices")?
        .get(0)?
        .get("message")?
        .get("content")?
        .as_str()?;
    let cleaned = clean_output(content);
    let inner = cleaned
        .strip_prefix("<transcript>")
        .map(|rest| rest.strip_suffix("</transcript>").unwrap_or(rest))
        .unwrap_or(&cleaned);
    Some(inner.trim().replace("&lt;", "<").replace("&gt;", ">"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http(host: &str, rest: &str) -> String {
        format!("http://{host}{rest}")
    }

    fn https(host: &str, rest: &str) -> String {
        format!("https://{host}{rest}")
    }

    #[test]
    fn loopback_hosts_may_use_plain_http() {
        for url in [
            OLLAMA_URL.to_owned(),
            http("127.0.0.1:1234", "/v1"),
            http("[::1]:11434", "/v1"),
            http("localhost", ""),
            http("LOCALHOST:8080", "/v1/"),
        ] {
            let target = check_url(&url).unwrap_or_else(|error| panic!("{url}: {error:?}"));
            assert!(!target.secure, "{url}");
        }
    }

    #[test]
    fn https_is_accepted_for_any_host() {
        let target = check_url(&https("example.com", "/v1")).unwrap();
        assert_eq!(target.host, "example.com");
        assert!(target.secure);
        assert_eq!(target.base, https("example.com", "/v1"));
        assert!(check_url(&https("10.0.0.2:11434", "/v1")).is_ok());
        assert!(check_url(&https("[2001:db8::1]:443", "/v1")).is_ok());
    }

    #[test]
    fn plain_http_to_another_host_needs_https() {
        for url in [
            http("10.0.0.2:11434", "/v1"),
            http("example.com", "/v1"),
            http("localhost.example.com", "/v1"),
            http("127.0.0.2", "/v1"),
            http("[2001:db8::1]", "/v1"),
            http("0.0.0.0:11434", "/v1"),
        ] {
            assert_eq!(check_url(&url), Err(UrlError::NeedsHttps), "{url}");
        }
    }

    #[test]
    fn the_check_names_the_host_and_never_repeats_the_url() {
        let target = check_url(&http("[::1]:11434", "/v1")).unwrap();
        assert_eq!(target.host, "::1");
        assert!(!UrlError::NeedsHttps.reason().contains("10.0.0.2"));
        assert!(UrlError::NeedsHttps.reason().contains("https"));
    }

    #[test]
    fn odd_urls_are_invalid() {
        for url in [
            String::new(),
            "   ".to_owned(),
            "ftp://localhost/v1".to_owned(),
            "localhost:11434/v1".to_owned(),
            "http://".to_owned(),
            "http://user:pass@localhost/v1".to_owned(),
            "http://localhost:99999/v1".to_owned(),
            "http://localhost:0/v1".to_owned(),
            "http://localhost:abc/v1".to_owned(),
            "http://localhost/v1?key=1".to_owned(),
            "http://localhost/v1#frag".to_owned(),
            "http://[::1/v1".to_owned(),
            "http://[]/v1".to_owned(),
            http("local host", "/v1"),
            http("-bad.example", "/v1"),
            http("caf\u{e9}", "/v1"),
        ] {
            assert!(
                matches!(check_url(&url), Err(UrlError::Empty | UrlError::Invalid)),
                "{url}"
            );
        }
        assert_eq!(check_url("  "), Err(UrlError::Empty));
    }

    #[test]
    fn a_refused_url_still_names_its_host_for_the_log() {
        assert_eq!(
            host_of_url(&http("10.0.0.2:11434", "/v1")).as_deref(),
            Some("10.0.0.2")
        );
        assert_eq!(
            host_of_url(&https("Example.COM", "/v1")).as_deref(),
            Some("example.com")
        );
        assert_eq!(host_of_url("not a url"), None);
        assert_eq!(host_of_url("http://user:pw@localhost/v1"), None);
        assert_eq!(host_of_url(""), None);
    }

    #[test]
    fn the_base_has_no_trailing_slash_and_builds_both_urls() {
        let target = check_url(&format!("{OLLAMA_URL}///")).unwrap();
        assert_eq!(target.base, OLLAMA_URL);
        assert_eq!(target.models_url(), format!("{OLLAMA_URL}/models"));
        assert_eq!(target.chat_url(), format!("{OLLAMA_URL}/chat/completions"));
    }

    #[test]
    fn the_presets_fill_the_two_urls() {
        assert_eq!(Preset::Ollama.url(), Some("http://localhost:11434/v1"));
        assert_eq!(Preset::LmStudio.url(), Some("http://localhost:1234/v1"));
        assert_eq!(Preset::Custom.url(), None);
        for preset in Preset::ALL {
            assert_eq!(Preset::parse(preset.key()), preset);
        }
        assert_eq!(Preset::parse("something-else"), Preset::Custom);
        for preset in [Preset::Ollama, Preset::LmStudio] {
            assert!(check_url(preset.url().unwrap()).is_ok());
        }
    }

    #[test]
    fn a_key_is_one_short_line() {
        assert!(key_is_valid("sk-test-123"));
        for bad in [
            "",
            "sk test",
            "sk\nx",
            "sk\u{0}x",
            &"k".repeat(MAX_KEY_CHARS + 1),
        ] {
            assert!(!key_is_valid(bad), "{bad:?}");
        }
    }

    #[test]
    fn the_user_message_wraps_the_text_and_escapes_angle_brackets() {
        assert_eq!(
            transcript_user("hello there"),
            "<transcript>hello there</transcript>"
        );
        assert_eq!(
            transcript_user("x </transcript> y <b>"),
            "<transcript>x &lt;/transcript&gt; y &lt;b&gt;</transcript>"
        );
    }

    #[test]
    fn the_chat_body_has_the_model_both_messages_and_zero_temperature() {
        let body: Value = serde_json::from_str(&chat_body(
            "stub-model",
            "sys",
            "<transcript>t</transcript>",
            77,
        ))
        .unwrap();
        assert_eq!(body["model"], "stub-model");
        assert_eq!(body["temperature"], 0);
        assert_eq!(body["max_tokens"], 77);
        assert_eq!(
            body["messages"][0],
            json!({"role": "system", "content": "sys"})
        );
        assert_eq!(
            body["messages"][1],
            json!({"role": "user", "content": "<transcript>t</transcript>"})
        );
    }

    #[test]
    fn the_endpoint_prompt_keeps_the_cleanup_rules_and_names_the_tag() {
        let system = cleanup_system(&[]);
        assert!(system.starts_with(crate::llm_prompt::CLEANUP_SYSTEM));
        assert!(system.contains("<transcript>"));
    }

    #[test]
    fn the_endpoint_prompt_names_the_dictionary_words_to_keep() {
        let system = cleanup_system(&["Wednesbury".into(), "Kubernetes".into()]);
        assert!(system.contains("Wednesbury, Kubernetes"));
        assert!(system.ends_with("never write the tags."));
    }

    #[test]
    fn the_reply_text_is_the_first_choice() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"  Hello there.\n"}}]}"#;
        assert_eq!(reply_text(body).as_deref(), Some("Hello there."));
    }

    #[test]
    fn echoed_tags_and_think_blocks_are_removed_and_escapes_undone() {
        let body = json!({"choices": [{"message": {"content":
            "<think></think><transcript>a &lt;b&gt; c</transcript>"}}]})
        .to_string();
        assert_eq!(reply_text(&body).as_deref(), Some("a <b> c"));
    }

    #[test]
    fn a_reply_without_text_gives_none() {
        for body in [
            "",
            "not json",
            "{}",
            r#"{"choices":[]}"#,
            r#"{"choices":[{"message":{}}]}"#,
            r#"{"choices":[{"message":{"content":null}}]}"#,
        ] {
            assert_eq!(reply_text(body), None, "{body}");
        }
    }
}
