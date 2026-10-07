//! The only module that opens network connections (architecture section 9). `xtask net-audit`
//! lists every call site here with its purpose and host.

pub mod download;
pub mod fetch;
pub mod policy;
#[cfg(test)]
mod test_server;

use crate::hook;

/// Why a request exists. Each purpose has a gate that runs before the socket opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// A model file that the user asked for. Hosts: `huggingface.co`, then `*.hf.co`.
    ModelDownload,
}

impl Purpose {
    pub fn as_str(self) -> &'static str {
        match self {
            Purpose::ModelDownload => "model_download",
        }
    }
}

/// Tells `hookctl net` about one request. Every call site calls this once it knows the result.
pub fn record(purpose: Purpose, host: &str, result: &str) {
    hook::record_net(purpose.as_str(), host, result);
}
