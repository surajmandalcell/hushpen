//! The only module that opens network connections (architecture section 9). `xtask net-audit`
//! lists every call site here with its purpose and host.

pub mod download;
pub mod endpoint;
pub mod fetch;
pub mod policy;
#[cfg(test)]
mod test_server;
pub mod update;

use crate::hook;

/// Why a request exists. Each purpose has a gate that runs before the socket opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// A model file that the user asked for. Hosts: `huggingface.co`, then `*.hf.co`.
    ModelDownload,
    /// The OpenAI-compatible endpoint that the user turned on. The host is whatever the user
    /// typed: `https` for any host, `http` only for localhost, 127.0.0.1 and ::1.
    Endpoint,
    /// The signed `latest.json` of the opt-in update check. Hosts: `github.com`, then
    /// `*.githubusercontent.com`.
    UpdateCheck,
    /// The release file that the signed manifest names, after the user asked for it or turned
    /// on automatic installs. Same hosts as the check.
    UpdateDownload,
}

impl Purpose {
    pub fn as_str(self) -> &'static str {
        match self {
            Purpose::ModelDownload => "model_download",
            Purpose::Endpoint => "endpoint",
            Purpose::UpdateCheck => "update_check",
            Purpose::UpdateDownload => "update_download",
        }
    }
}

/// Tells `hookctl net` about one request. Every call site calls this once it knows the result.
pub fn record(purpose: Purpose, host: &str, result: &str) {
    hook::record_net(purpose.as_str(), host, result);
}
