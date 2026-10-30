//! The Settings view: one list of grouped sections with a search, the controls that write the
//! settings store, launch at login, and the data folder move.

mod ai;
mod data_folder;
pub mod endpoint;
mod login;
mod panel;
mod registry;
pub mod search;

use crate::enhance::Tuning;
use crate::hook;
use crate::mic::Mic;
use crate::storage::Storage;
use crate::updater::Updater;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{App, AppContext as _, Context, Entity, FocusHandle, Point, ScrollHandle, Window};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

pub use ai::AiParts;
pub(crate) use ai::AiStatus;
pub use data_folder::MoveState;
use login::LoginCache;
pub use login::LoginToggle;
pub(crate) use panel::{render, search_field};

/// Focus slots in [`Settings::control_focus`], shared with the panel.
pub(crate) mod slots {
    pub const LOGIN: usize = 0;
    pub const HIDDEN: usize = 1;
    pub const MIC: usize = 2;
    pub const SOUNDS: usize = 3;
    pub const CUE_VOLUME: usize = 4;
    pub const MAX_MINUTES: usize = 5;
    pub const RULES: usize = 6;
    pub const SPOKEN_PUNCTUATION: usize = 7;
    pub const AI_CLEANUP: usize = 8;
    pub const AI_PROVIDER: usize = 9;
    pub const AI_MODEL: usize = 10;
    pub const AI_DOWNLOAD: usize = 11;
    pub const ENDPOINT_PRESET: usize = 12;
    pub const ENDPOINT_KEY_CLEAR: usize = 13;
    pub const ENDPOINT_TEST: usize = 14;
    pub const GPU: usize = 15;
    pub const THREADS: usize = 16;
    pub const FLOWBAR: usize = 17;
    pub const FLOWBAR_IDLE: usize = 18;
    pub const POSITION: usize = 19;
    pub const CHANGE_FOLDER: usize = 20;
    pub const RETENTION: usize = 21;
    pub const UPDATES_CHECK: usize = 22;
    pub const UPDATES_AUTO: usize = 23;
    pub const UPDATES_NOW: usize = 24;
    pub const UPDATES_NOTES: usize = 25;
    pub const SEARCH_CLEAR: usize = 26;
    pub const INPUT_GAIN: usize = 27;
    pub const ENHANCE: usize = 28;
    pub const ENHANCE_ADJUST: usize = 29;
    pub const COUNT: usize = 30;
    /// The options of the open picker. One picker is open at a time, so they share a pool.
    pub const OPTIONS: usize = 12;
}

/// The sections of the Settings list, in page order. Each has a heading above its group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    General,
    Shortcuts,
    Audio,
    Cleanup,
    Engine,
    FlowBar,
    Storage,
    Updates,
}

impl Section {
    pub const ALL: [Section; 8] = [
        Section::General,
        Section::Shortcuts,
        Section::Audio,
        Section::Cleanup,
        Section::Engine,
        Section::FlowBar,
        Section::Storage,
        Section::Updates,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Section::General => "General",
            Section::Shortcuts => "Shortcuts",
            Section::Audio => "Audio",
            Section::Cleanup => "Cleanup",
            Section::Engine => "Engine",
            Section::FlowBar => "Flow bar",
            Section::Storage => "Storage",
            Section::Updates => "Updates",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Section::General => "general",
            Section::Shortcuts => "shortcuts",
            Section::Audio => "audio",
            Section::Cleanup => "cleanup",
            Section::Engine => "engine",
            Section::FlowBar => "flowbar",
            Section::Storage => "storage",
            Section::Updates => "updates",
        }
    }

    /// The one line of help under the group.
    pub fn footer(self) -> Option<&'static str> {
        match self {
            Section::Shortcuts => Some("Click a shortcut, then press the keys. Esc cancels."),
            Section::Cleanup => Some(
                "AI cleanup stays off until you turn it on. An endpoint is a server you choose, such as Ollama.",
            ),
            Section::Engine => Some("The speech engine loads again with the new choice."),
            Section::FlowBar => {
                Some("Dragging the bar overrides the position until you pick again.")
            }
            _ => None,
        }
    }
}

pub struct Settings {
    pub(crate) storage: Rc<Storage>,
    pub(crate) mic: Entity<Mic>,
    login: Option<Arc<dyn LoginToggle>>,
    login_cache: LoginCache,
    default_data_dir: PathBuf,
    /// The open picker, if any, named by the hook id of its trigger, such as `audio.mic`.
    pub(crate) picker: Option<&'static str>,
    pub(crate) move_state: MoveState,
    pub(crate) notice: Option<String>,
    /// What a finished move does. Tests record it instead of restarting.
    pub(crate) restart: Restart,
    pub(crate) control_focus: Vec<FocusHandle>,
    pub(crate) option_focus: Vec<FocusHandle>,
    /// What the search field holds. It is never saved.
    pub(crate) query: String,
    pub(crate) search: Option<Entity<InputState>>,
    /// The scroll position of the content pane, which a new query puts back at the top.
    scroll: Option<ScrollHandle>,
    ai: Option<AiParts>,
    ai_loading: bool,
    pub(crate) endpoint: Option<Entity<endpoint::EndpointForm>>,
    pub(crate) updater: Option<Entity<Updater>>,
    /// The audio enhance sheet, which the Audio enhance row opens.
    pub(crate) tuning: Option<Entity<Tuning>>,
    in_flight: Option<InFlight>,
}

/// How the app hands over to a fresh copy after the move.
pub type Restart = Rc<dyn Fn(&mut App)>;

/// Whether a transcription is running: a dictation after the microphone stopped, or a file
/// import. A data folder move waits for it, so no history row lands after the old
/// `history.db` is gone.
pub type InFlight = Rc<dyn Fn(&App) -> bool>;

pub struct Parts {
    pub storage: Rc<Storage>,
    pub mic: Entity<Mic>,
    pub login: Option<Arc<dyn LoginToggle>>,
    /// Where the pointer file lives. Defaults to the system's default data
    /// folder; tests pass a folder of their own.
    pub default_data_dir: Option<PathBuf>,
    /// Defaults to [`restart_into`]; tests inject a recorder.
    pub restart: Option<Restart>,
}

impl Settings {
    pub fn new(parts: Parts, cx: &mut Context<Self>) -> Self {
        let Parts {
            storage,
            mic,
            login,
            default_data_dir,
            restart,
        } = parts;
        cx.observe(&mic, |_, _, cx| cx.notify()).detach();
        let mut me = Self {
            storage,
            mic,
            login,
            login_cache: LoginCache::default(),
            default_data_dir: default_data_dir
                .unwrap_or_else(hushpen_store::data_dir::resolve_from_env),
            picker: None,
            move_state: MoveState::Idle,
            notice: None,
            restart: restart.unwrap_or_else(|| Rc::new(data_folder::restart_into)),
            control_focus: (0..slots::COUNT)
                .map(|_| cx.focus_handle().tab_stop(true))
                .collect(),
            option_focus: (0..slots::OPTIONS)
                .map(|_| cx.focus_handle().tab_stop(true))
                .collect(),
            query: String::new(),
            search: None,
            scroll: None,
            ai: None,
            ai_loading: false,
            endpoint: None,
            updater: None,
            tuning: None,
            in_flight: None,
        };
        me.refresh_login(cx);
        me
    }

    /// Shows the update check in Settings > Updates and repaints when it changes.
    pub fn attach_updater(&mut self, updater: Entity<Updater>, cx: &mut Context<Self>) {
        cx.observe(&updater, |_, _, cx| cx.notify()).detach();
        self.updater = Some(updater);
    }

    pub fn toggle_update_check(&mut self, cx: &mut Context<Self>) {
        self.flip("updates.check", cx);
        self.updates_changed(cx);
    }

    pub fn toggle_auto_install(&mut self, cx: &mut Context<Self>) {
        self.flip("updates.autoInstall", cx);
        self.updates_changed(cx);
    }

    fn updates_changed(&mut self, cx: &mut Context<Self>) {
        if let Some(updater) = self.updater.clone() {
            updater.update(cx, |updater, cx| updater.settings_changed(cx));
        }
    }

    /// Connects the audio enhance sheet to the Audio enhance row.
    pub fn attach_tuning(&mut self, tuning: Entity<Tuning>, cx: &mut Context<Self>) {
        cx.observe(&tuning, |_, _, cx| cx.notify()).detach();
        self.tuning = Some(tuning);
    }

    /// Tells Settings how to see a running transcription.
    pub fn attach_in_flight(&mut self, in_flight: InFlight) {
        self.in_flight = Some(in_flight);
    }

    /// Connects the endpoint rows of Cleanup. Their fields need a window, so the form is built
    /// by the caller.
    pub fn attach_endpoint(
        &mut self,
        form: Entity<endpoint::EndpointForm>,
        cx: &mut Context<Self>,
    ) {
        cx.observe(&form, |_, _, cx| cx.notify()).detach();
        self.endpoint = Some(form);
    }

    pub fn close_picker(&mut self, cx: &mut Context<Self>) {
        if self.picker.take().is_some() {
            cx.notify();
        }
    }

    /// The Settings view opened: re-read launch at login from the system, so
    /// a login item removed by hand shows as off.
    pub fn opened(&mut self, cx: &mut Context<Self>) {
        self.refresh_login(cx);
        cx.notify();
    }

    /// The user came back to Hushpen, often from System Settings > Login Items: read launch at
    /// login again.
    pub fn app_activated(&mut self, cx: &mut Context<Self>) {
        self.refresh_login(cx);
    }

    /// Builds the search field of the toolbar. It needs a window, so the caller attaches it.
    pub fn attach_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        cx.subscribe_in(&search, window, |this, input, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                let value = input.read(cx).value().to_string();
                this.set_query(value, cx);
            }
        })
        .detach();
        self.search = Some(search);
    }

    /// Shares the scroll position of the content pane, so a new query starts at the top.
    pub fn use_scroll(&mut self, scroll: ScrollHandle) {
        self.scroll = Some(scroll);
    }

    /// Shows only the rows that match `query`; an empty one shows every row.
    pub fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        if self.query == query {
            return;
        }
        self.query = query;
        self.picker = None;
        if let Some(scroll) = &self.scroll {
            scroll.set_offset(Point::default());
        }
        hook::record_event("settings", "search");
        cx.notify();
    }

    /// Types `query` into the search field like a user does.
    pub fn search_for(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(search) = &self.search {
            search.update(cx, |input, cx| {
                input.set_value(query.to_owned(), window, cx)
            });
        }
        self.set_query(query.to_owned(), cx);
    }

    /// Esc in the search field: clears a query and says so. With nothing typed it returns false,
    /// and the caller moves the focus away.
    pub fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.query.is_empty() {
            return false;
        }
        self.search_for("", window, cx);
        true
    }

    pub fn toggle_picker(&mut self, picker: &'static str, cx: &mut Context<Self>) {
        self.picker = match self.picker {
            Some(open) if open == picker => None,
            _ => Some(picker),
        };
        cx.notify();
    }

    /// `hookctl state` section `settings_view`: the query, the registry rows it matches (a row
    /// that does not apply, such as an endpoint field while AI cleanup runs here, draws nothing)
    /// and their sections in page order, the move, and launch at login as the system has it. The
    /// hook module wires it once the hook is attached.
    pub fn hook_state_json(&self, cx: &gpui_kit::App) -> Value {
        let shown: Vec<&search::RowMeta> = search::visible(&self.query)
            .into_iter()
            .filter_map(|id| search::ROWS.iter().find(|meta| meta.id == id))
            .collect();
        let mut sections: Vec<&str> = shown.iter().map(|meta| meta.section.key()).collect();
        sections.dedup();
        json!({
            "query": self.query,
            "rows": shown.iter().map(|meta| meta.key).collect::<Vec<_>>(),
            "sections": sections,
            "ai": {
                "enabled": self.stored_bool("cleanup.llm.enabled"),
                "provider": self.ai_provider(),
                "endpoint": self.endpoint.as_ref().map(|form| form.read(cx).hook_state_json(cx)),
                "model": self.ai_model().map(|model| model.id),
                "offered": self.ai_offered().into_iter().map(|model| model.id).collect::<Vec<_>>(),
                "status": self.ai_status(cx).map(AiStatus::key),
            },
            "launch_at_login": self.launch_at_login(),
            "data_folder": self.storage.data.root().display().to_string(),
            "move": match &self.move_state {
                MoveState::Idle => json!({"state": "idle"}),
                MoveState::Choosing => json!({"state": "choosing"}),
                MoveState::Failed(code) => json!({"state": "failed", "error": code}),
                MoveState::Restarting => json!({"state": "restarting"}),
            },
        })
    }
}

#[cfg(test)]
mod endpoint_tests;
#[cfg(test)]
mod list_tests;
#[cfg(test)]
mod tests;
