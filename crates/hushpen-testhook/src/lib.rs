//! Test hook for debug builds with the `test-automation` feature.
//!
//! The app starts a [`server::HookServer`] on a Unix socket in the data
//! folder (or loopback TCP inside a container). `hookctl` talks to it. The
//! hook reports and drives the real app; it never fakes engine results,
//! permissions, clocks, or receipts.

#[cfg(all(feature = "guard", not(debug_assertions)))]
compile_error!(
    "the test hook is for debug builds only: build without the test-automation feature, \
     or without --release"
);

pub mod cli;
pub mod client;
pub mod dialogs;
pub mod endpoint;
pub mod logs;
pub mod protocol;
pub mod registry;
pub mod router;
pub mod server;

pub use endpoint::Endpoint;
pub use protocol::{ActionInfo, Bounds, ElementInfo, Request, Response};
pub use registry::{ActionRegistry, StateRegistry};
pub use router::{Backend, Router};
pub use server::{Handler, HookServer};
