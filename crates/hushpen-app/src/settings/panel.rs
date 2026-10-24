//! The Settings view: one scrolling list of sections, each a heading above a group, filtered by
//! the search field in the toolbar. Every control has a stable hook id: `settings.<section>.<name>`.

mod audio;
mod cleanup;
mod endpoint;
mod engine;
mod flow_bar;
mod general;
mod shortcuts;
mod storage;
mod updates;

use super::search::{self, ROWS, RowId};
use super::{Section, Settings, slots};
use crate::hook;
use crate::shortcuts::Shortcuts;
use crate::theme::{self, Palette, size, space};
use crate::ui::{self, Kind, Notice};
use gpui_kit::assets::IconName;
use gpui_kit::component::input::InputState;
use gpui_kit::{
    AnyElement, App, Context, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, SharedString, Styled, Window, div, px,
};
use std::rc::Rc;

/// What a row builder reads: the colors, the view, and the focus handles of its controls.
pub(super) struct Page {
    palette: Palette,
    settings: Entity<Settings>,
    shortcuts: Option<Entity<Shortcuts>>,
    focus: Vec<FocusHandle>,
    options: Vec<FocusHandle>,
}

/// A change a control makes to the view.
type Act = fn(&mut Settings, &mut Context<Settings>);

/// One option of a picker: the value it stores, its label, and whether it is the current one.
type Choice = (String, String, bool);

impl Page {
    /// Runs `act` on the view.
    fn act(&self, act: Act) -> impl Fn(&mut App) + 'static {
        let settings = self.settings.clone();
        move |cx| settings.update(cx, act)
    }

    /// A row with a switch, under `settings.<name>`.
    fn toggle(
        &self,
        name: &str,
        title: &'static str,
        detail: &str,
        on: bool,
        slot: usize,
        act: Act,
    ) -> AnyElement {
        ui::toggle_row(
            self.palette,
            settings_id(name),
            title,
            detail,
            on,
            &self.focus[slot],
            self.act(act),
        )
    }

    /// A row with a pop-up button that opens the picker `name`.
    fn menu(
        &self,
        name: &'static str,
        title: &'static str,
        detail: &str,
        current: String,
        slot: usize,
    ) -> AnyElement {
        let settings = self.settings.clone();
        ui::menu_row(
            self.palette,
            settings_id(name),
            title,
            detail,
            current,
            &self.focus[slot],
            move |cx| settings.update(cx, |me, cx| me.toggle_picker(name, cx)),
        )
    }

    fn picker_open(&self, name: &str, cx: &App) -> bool {
        self.settings.read(cx).picker == Some(name)
    }

    /// The options of the open picker `name`, each under `settings.<name>.option.<value>`.
    /// `choose` gets the value of the one picked.
    fn options(
        &self,
        name: &str,
        choices: Vec<Choice>,
        focus: &[FocusHandle],
        choose: impl Fn(&mut Settings, &str, &mut Context<Settings>) + 'static,
    ) -> Vec<AnyElement> {
        let choose = Rc::new(choose);
        choices
            .into_iter()
            .zip(focus)
            .map(|((value, label, selected), focus)| {
                let settings = self.settings.clone();
                let choose = Rc::clone(&choose);
                let id = settings_id(&format!("{name}.option.{value}"));
                ui::option_row(
                    self.palette,
                    id,
                    label,
                    None,
                    selected,
                    focus,
                    move |_, cx| settings.update(cx, |me, cx| choose(me, &value, cx)),
                )
            })
            .collect()
    }
}

/// The hook id of a control of this view: `settings.<name>`.
fn settings_id(name: &str) -> SharedString {
    hook::id("settings", name)
}

/// The lines a row draws: none when it does not apply, such as the endpoint fields while AI
/// cleanup runs on this computer.
fn rows_of(row: RowId, page: &Page, cx: &mut App) -> Vec<AnyElement> {
    match row {
        RowId::LaunchAtLogin => vec![general::launch_at_login(page, cx)],
        RowId::StartHidden => vec![general::start_hidden(page, cx)],
        RowId::HoldKey => shortcuts::slot(page, hushpen_core::shortcut::Slot::Hold, cx),
        RowId::HandsFree => shortcuts::slot(page, hushpen_core::shortcut::Slot::HandsFree, cx),
        RowId::PasteLast => shortcuts::slot(page, hushpen_core::shortcut::Slot::PasteLast, cx),
        RowId::CommandMode => shortcuts::slot(page, hushpen_core::shortcut::Slot::Command, cx),
        RowId::ResetShortcuts => shortcuts::reset(page, cx),
        RowId::Microphone => audio::microphone(page, cx),
        RowId::Sounds => vec![audio::sounds(page, cx)],
        RowId::CueVolume => vec![audio::cue_volume(page, cx)],
        RowId::MaxLength => audio::max_length(page, cx),
        RowId::Rules => vec![cleanup::rules(page, cx)],
        RowId::SpokenPunctuation => vec![cleanup::spoken_punctuation(page, cx)],
        RowId::AiCleanup => vec![cleanup::ai_cleanup(page, cx)],
        RowId::AiSource => cleanup::ai_source(page, cx),
        RowId::AiModel => cleanup::ai_model(page, cx),
        RowId::EndpointPreset => endpoint::preset(page, cx),
        RowId::EndpointUrl => endpoint::url(page, cx),
        RowId::EndpointModel => endpoint::model(page, cx),
        RowId::EndpointKey => endpoint::key(page, cx),
        RowId::EndpointTest => endpoint::test(page, cx),
        RowId::Gpu => vec![engine::gpu(page, cx)],
        RowId::Threads => engine::threads(page, cx),
        RowId::ShowFlowBar => vec![flow_bar::show(page, cx)],
        RowId::FlowBarIdle => vec![flow_bar::idle(page, cx)],
        RowId::FlowBarPosition => flow_bar::position(page, cx),
        RowId::DataFolder => vec![storage::data_folder(page, cx)],
        RowId::Retention => storage::retention(page, cx),
        RowId::UpdateCheck => vec![updates::check(page, cx)],
        RowId::AutoInstall => vec![updates::auto_install(page, cx)],
        RowId::UpdateStatus => updates::status(page, cx),
    }
}

pub fn render(
    settings: &Entity<Settings>,
    shortcuts: Option<&Entity<Shortcuts>>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let palette = theme::palette(cx);
    let (query, notice, page) = {
        let me = settings.read(cx);
        (
            me.query.clone(),
            me.notice.clone(),
            Page {
                palette,
                settings: settings.clone(),
                shortcuts: shortcuts.cloned(),
                focus: me.control_focus.clone(),
                options: me.option_focus.clone(),
            },
        )
    };
    let visible = search::visible(&query);
    let mut sections = Vec::new();
    for section in Section::ALL {
        let mut rows = Vec::new();
        for meta in ROWS.iter().filter(|meta| meta.section == section) {
            if visible.contains(&meta.id) {
                rows.extend(rows_of(meta.id, &page, cx));
            }
        }
        if section == Section::Shortcuts
            && !rows.is_empty()
            && let Some(notice) = shortcuts::keys_off(&page, cx)
        {
            rows.insert(0, notice);
        }
        if !rows.is_empty() {
            sections.push(section_of(palette, section, rows));
        }
    }
    // Esc with an open picker closes it and focuses the trigger again.
    let on_escape = {
        let settings = settings.clone();
        move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            if event.keystroke.key.as_str() == "escape" {
                let focus = settings.update(cx, |me, cx| me.close_picker(cx));
                if let Some(focus) = focus {
                    window.focus(&focus, cx);
                    cx.stop_propagation();
                }
            }
        }
    };
    let mut list = div()
        .id(settings_id("list"))
        .flex()
        .flex_col()
        .gap(px(space::XXL))
        .w_full()
        .on_key_down(on_escape);
    if let Some(message) = notice {
        list = list.child(ui::group(
            settings_id("notice.group"),
            palette,
            vec![ui::notice_row(
                palette,
                settings_id("notice"),
                message,
                Notice::Alert,
                None,
            )],
        ));
    }
    if sections.is_empty() {
        list = list.child(no_match(&page, &query));
    }
    list.children(sections)
}

/// The heading, the group, and the help line of one section.
fn section_of(palette: Palette, section: Section, rows: Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(space::XS + space::XXS))
        .w_full()
        .child(ui::heading(
            palette,
            settings_id(&format!("{}.heading", section.key())),
            section.title(),
        ))
        .child(ui::section(
            palette,
            ui::group(settings_id(section.key()), palette, rows),
            section.footer().map(|text| {
                (
                    settings_id(&format!("{}.help", section.key())),
                    text.to_owned(),
                )
            }),
        ))
        .into_any_element()
}

/// The group that says no row matches, with a button that clears the search.
fn no_match(page: &Page, query: &str) -> AnyElement {
    let settings = page.settings.clone();
    let clear = ui::button(
        page.palette,
        settings_id("search.clear"),
        "Clear",
        Kind::Plain,
        Some(&page.focus[slots::SEARCH_CLEAR]),
        true,
        move |window, cx| {
            settings.update(cx, |me, cx| me.clear_search(window, cx));
        },
    );
    ui::group(
        settings_id("search.empty.group"),
        page.palette,
        vec![ui::notice_row(
            page.palette,
            settings_id("search.empty"),
            format!("No settings match \"{query}\""),
            Notice::Info,
            Some(clear.into_any_element()),
        )],
    )
    .into_any_element()
}

/// The search field for the right end of the toolbar, or nothing before it is attached.
pub fn search_field(settings: &Entity<Settings>, cx: &App) -> Option<impl IntoElement + use<>> {
    let palette = theme::palette(cx);
    let state: Entity<InputState> = settings.read(cx).search.clone()?;
    Some(
        div().w(px(size::SEARCH_FIELD_WIDTH)).flex_none().child(
            ui::field(palette, settings_id("search"), "Search settings", &state)
                .prefix(ui::glyph(palette, IconName::Search)),
        ),
    )
}
