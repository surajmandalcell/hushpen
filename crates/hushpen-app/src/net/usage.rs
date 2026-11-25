//! The daily counts of IndieCrumb: one ping a day with how often Hushpen was used. "Share
//! anonymous usage" in Settings > Updates turns it off. Off, the client keeps and sends nothing.

use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// The settings key of "Share anonymous usage". On by default.
pub const SHARE: &str = "updates.shareUsage";

static CLIENT: OnceLock<Arc<indiecrumb::Client>> = OnceLock::new();

/// Opens the counts in their own folder, so the crash client keeps its files, and pings at launch
/// and every hour from a background thread. Call once, early.
pub fn start(data: &Path, on: bool) {
    let client = indiecrumb::Client::new(indiecrumb::Config {
        endpoint: super::crash::ENDPOINT.into(),
        key: super::crash::KEY.into(),
        version: env!("CARGO_PKG_VERSION").into(),
        build: String::new(),
        dir: data.join("indiecrumb-usage"),
        enabled: on,
        transport: Box::new(indiecrumb::Ureq::default()),
    });
    let client = match client {
        Ok(client) => Arc::new(client),
        Err(error) => {
            eprintln!("hushpen: usage counts are off: {error}");
            return;
        }
    };
    let _ = CLIENT.set(Arc::clone(&client));
    std::thread::spawn(move || {
        loop {
            client.ping_if_due(false);
            std::thread::sleep(Duration::from_secs(60 * 60));
        }
    });
}

/// Counts one completed dictation, file transcription, or read aloud.
pub fn record_use() {
    if let Some(client) = CLIENT.get() {
        client.record_use();
    }
}

/// Follows the Settings switch. Turning it off deletes the counts that wait to be sent.
pub fn set_enabled(on: bool) {
    if let Some(client) = CLIENT.get() {
        client.set_enabled(on);
    }
}
