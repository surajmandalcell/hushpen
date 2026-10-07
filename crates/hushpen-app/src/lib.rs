//! Hushpen desktop app.

#[cfg(all(feature = "test-automation", not(debug_assertions)))]
compile_error!(
    "the test hook is for debug builds only: build without the test-automation feature, \
     or without --release"
);

pub mod app;
pub mod assets;
pub mod cli;
pub mod dictation;
pub mod engine_host;
pub mod hook;
pub mod instance;
pub mod mic;
pub mod models;
pub mod net;
pub mod shell;
pub mod storage;
pub mod theme;
pub mod views;
