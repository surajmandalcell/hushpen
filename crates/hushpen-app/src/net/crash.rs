//! Crash reports, ask first. A panic or a fatal signal writes one file under `<data>/indiecrumb/crashes/`;
//! the next launch asks "Hushpen quit unexpectedly." (Report…, Ignore). Report… shows the Problem
//! Report window with the exact text; nothing is sent unless the person presses Send to Developer.
//! Ignore, Don't Send, or closing the window deletes the report.

use gpui_kit::App;
use std::path::Path;
use std::sync::Arc;

/// The IndieCrumb host.
pub(super) const ENDPOINT: &str = "https://crumb.samhq.app";

/// The public IndieCrumb ingest key: it ships in the app and can only submit reports.
pub(super) const KEY: &str = "ic_pub_e02ef20e5bf29972c8ce2a943c407ddf";

/// Opens the client and installs the panic hook. Call once, early, before the window opens.
pub fn start(data: &Path) -> Option<Arc<indiecrumb::Client>> {
    let client = indiecrumb::Client::new(indiecrumb::Config {
        endpoint: ENDPOINT.into(),
        key: KEY.into(),
        version: env!("CARGO_PKG_VERSION").into(),
        build: String::new(),
        dir: data.join("indiecrumb"),
        // The hook only writes a local file; sending always waits for Send.
        enabled: true,
        transport: Box::new(indiecrumb::Ureq::default()),
    });
    match client {
        Ok(client) => {
            client.install_panic_hook();
            Some(Arc::new(client))
        }
        Err(error) => {
            eprintln!("hushpen: crash reports are off: {error}");
            None
        }
    }
}

/// Shows the waiting crash report, if there is one, without activating the app.
pub fn ask(client: &Arc<indiecrumb::Client>, cx: &mut App) {
    indiecrumb::ui::ask_crash(Arc::clone(client), "Hushpen", cx);
}
