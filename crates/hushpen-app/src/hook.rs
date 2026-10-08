//! Element ids for the test hook. Every interactive element gets one, named
//! `<view>.<element>` or `<view>.<element>.<index>` (`sidebar.home`,
//! `history.row.3`). Pair the id with `.test_support()` on the element, which
//! is a no-op outside test builds.

use gpui_kit::SharedString;

#[cfg(feature = "test-automation")]
mod about;
#[cfg(feature = "test-automation")]
mod automation;
#[cfg(feature = "test-automation")]
mod dictation;
#[cfg(feature = "test-automation")]
mod dictionary;
#[cfg(feature = "test-automation")]
mod engine;
#[cfg(feature = "test-automation")]
mod flow_bar;
#[cfg(feature = "test-automation")]
mod history;
#[cfg(feature = "test-automation")]
mod mic;
#[cfg(feature = "test-automation")]
mod models;
#[cfg(feature = "test-automation")]
mod onboarding;
#[cfg(feature = "test-automation")]
mod settings;
#[cfg(feature = "test-automation")]
mod settings_view;
#[cfg(feature = "test-automation")]
mod shortcuts;
#[cfg(feature = "test-automation")]
mod tray;
#[cfg(feature = "test-automation")]
pub use about::attach as attach_about;
#[cfg(feature = "test-automation")]
pub use automation::{
    ExtraWindow, Hooks, Job, Surface, attach, mark_disabled, register_action, register_window,
    set_state_section, set_wav_feeder, start,
};
#[cfg(feature = "test-automation")]
pub use dictation::attach as attach_dictation;
#[cfg(feature = "test-automation")]
pub use dictionary::attach as attach_dictionary;
#[cfg(feature = "test-automation")]
pub use engine::attach as attach_engine;
#[cfg(feature = "test-automation")]
pub use flow_bar::attach as attach_flow_bar;
#[cfg(feature = "test-automation")]
pub use history::attach as attach_history;
#[cfg(feature = "test-automation")]
pub use mic::attach as attach_mic;
#[cfg(feature = "test-automation")]
pub use models::attach as attach_models;
#[cfg(feature = "test-automation")]
pub use onboarding::attach as attach_onboarding;
#[cfg(feature = "test-automation")]
pub use settings::attach as attach_settings;
#[cfg(feature = "test-automation")]
pub use settings_view::attach as attach_settings_view;
#[cfg(feature = "test-automation")]
pub use shortcuts::attach as attach_shortcuts;
#[cfg(feature = "test-automation")]
pub use tray::attach as attach_tray;

/// Records a timestamped event for `hookctl events`. Does nothing in a build
/// without the test hook.
#[cfg(feature = "test-automation")]
pub fn record_event(kind: &str, detail: &str) {
    hushpen_testhook::logs::record_event(kind, detail);
}

#[cfg(not(feature = "test-automation"))]
pub fn record_event(_kind: &str, _detail: &str) {}

/// Tells the hook that an element is drawn disabled. Does nothing without the hook.
#[cfg(not(feature = "test-automation"))]
pub fn mark_disabled(_id: &str, _disabled: bool) {}

/// Records one network request (purpose, host, result) for `hookctl net`.
/// Every network call site calls this once it knows the result. Does nothing
/// in a build without the test hook.
#[cfg(feature = "test-automation")]
pub fn record_net(purpose: &str, host: &str, result: &str) {
    hushpen_testhook::logs::record_net(purpose, host, result);
}

#[cfg(not(feature = "test-automation"))]
pub fn record_net(_purpose: &str, _host: &str, _result: &str) {}

pub fn id(view: &str, element: &str) -> SharedString {
    format!("{view}.{element}").into()
}

pub fn indexed(view: &str, element: &str, index: usize) -> SharedString {
    format!("{view}.{element}.{index}").into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_join_view_element_and_index_with_dots() {
        assert_eq!(id("sidebar", "home"), "sidebar.home");
        assert_eq!(indexed("history", "row", 3), "history.row.3");
    }
}
