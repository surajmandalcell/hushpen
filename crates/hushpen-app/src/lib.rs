//! Hushpen desktop app.

#[cfg(all(feature = "test-automation", not(debug_assertions)))]
compile_error!(
    "the test hook is for debug builds only: build without the test-automation feature, \
     or without --release"
);

pub mod actions;
pub mod app;
pub mod app_menu;
pub mod assets;
pub mod cli;
pub mod controller;
pub mod dictation;
pub mod dictionary;
pub mod engine_host;
pub mod flow_bar;
pub mod history;
pub mod hook;
pub mod instance;
pub mod main_window;
pub mod mic;
pub mod models;
pub mod native;
pub mod net;
pub mod onboarding;
pub mod shell;
pub mod shortcuts;
pub mod storage;
pub mod theme;
pub mod tray;
pub mod views;
