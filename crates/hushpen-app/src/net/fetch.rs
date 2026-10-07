//! One HTTP GET, with no redirect following. The download loop follows redirects itself so it can
//! check every hop against the host policy before a connection opens.

use super::policy::Policy;
use std::io::Read;
use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);

pub struct Response {
    pub status: u16,
    pub location: Option<String>,
    pub content_length: Option<u64>,
    /// First byte of a `206` answer, from `Content-Range: bytes <start>-<end>/<total>`.
    pub range_start: Option<u64>,
    pub body: Box<dyn Read + Send>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchError(pub String);

pub trait Fetcher: Send + Sync {
    /// GET `url`, from byte `range_from` on when it is set. Never follows a redirect.
    fn get(&self, url: &str, range_from: Option<u64>) -> Result<Response, FetchError>;
}

pub struct UreqFetcher {
    agent: ureq::Agent,
}

impl UreqFetcher {
    pub fn new(policy: &Policy) -> Self {
        // Purpose::ModelDownload is the only caller. Hosts: huggingface.co, then *.hf.co.
        let config = ureq::Agent::config_builder()
            .max_redirects(0)
            .http_status_as_error(false)
            .https_only(policy.https_only())
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_recv_response(Some(RESPONSE_TIMEOUT))
            .user_agent(format!("Hushpen/{}", hushpen_core::BUILD_VERSION))
            .build();
        Self {
            agent: ureq::Agent::new_with_config(config),
        }
    }
}

impl Fetcher for UreqFetcher {
    fn get(&self, url: &str, range_from: Option<u64>) -> Result<Response, FetchError> {
        let mut request = self
            .agent
            .get(url)
            // A compressed answer would break byte ranges and the size check.
            .header("Accept-Encoding", "identity");
        if let Some(from) = range_from {
            request = request.header("Range", format!("bytes={from}-"));
        }
        let response = request
            .call()
            .map_err(|error| FetchError(error.to_string()))?;
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        };
        let status = response.status().as_u16();
        let location = header("location");
        let content_length = header("content-length").and_then(|value| value.trim().parse().ok());
        let range_start = header("content-range").and_then(|value| {
            let range = value.trim().strip_prefix("bytes ")?;
            range.split(['-', '/']).next()?.trim().parse().ok()
        });
        Ok(Response {
            status,
            location,
            content_length,
            range_start,
            body: Box::new(response.into_body().into_reader()),
        })
    }
}
