//! Desktop network services and the shared download service.
//! `xtask net-audit` lists every call site with its purpose and host.

pub mod devices;
pub mod endpoint;
#[cfg(test)]
use hushpen_store::net::test_server;
pub use hushpen_store::net::{Purpose, download, fetch, policy};
pub mod update;

use crate::hook;

/// Tells `hookctl net` about one request. Every call site calls this once it knows the result.
pub fn record(purpose: Purpose, host: &str, result: &str) {
    hook::record_net(purpose.as_str(), host, result);
}
