//! The tray icon and its menu: Start or Stop dictation, Paste last transcript, Show Hushpen,
//! Settings, and Quit.
//!
//! Each entry is a GPUI action. [`register_actions`] turns the actions into what a click in the
//! app would do: the flow bar's click (so a refused start shows its reason on the bar), the
//! pipeline's `PasteLast` event, and the main window and view changes.

use crate::actions::{
    HideHushpen, PasteLastTranscript, QuitHushpen, ShowAbout, ShowHushpen, ShowSettings,
    ToggleDictation,
};
use crate::controller::Controller;
use crate::flow_bar::FlowBar;
use crate::hook;
use crate::main_window;
use crate::shell::Shell;
use crate::views::View;
use gpui_kit::{
    Action, AnyWindowHandle, App, Entity, Image, ImageFormat, MenuItem, WeakEntity,
    WindowAppearance,
};
use gpui_tray::{Icon, Tray};
use hushpen_core::dictation::{AppEvent, State};
use serde_json::{Value, json};

#[cfg(test)]
mod tests;

const TITLE: &str = "Hushpen";

/// Black glyph, for a light panel.
const GLYPH_BLACK: &[u8] = include_bytes!("../../../assets/asrpro-tray-dark.png");
/// White glyph, for a dark panel.
const GLYPH_WHITE: &[u8] = include_bytes!("../../../assets/asrpro-tray-light.png");

/// One entry of the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Dictation,
    PasteLast,
    Show,
    Settings,
    Quit,
}

impl Item {
    pub const ALL: [Item; 5] = [
        Item::Dictation,
        Item::PasteLast,
        Item::Show,
        Item::Settings,
        Item::Quit,
    ];

    /// The first entry flips while a dictation is listening, so one entry starts and stops.
    pub fn label(self, listening: bool) -> &'static str {
        match self {
            Item::Dictation if listening => "Stop dictation",
            Item::Dictation => "Start dictation",
            Item::PasteLast => "Paste last transcript",
            Item::Show => "Show Hushpen",
            Item::Settings => "Settings",
            Item::Quit => "Quit",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Item::Dictation => "toggle-dictation",
            Item::PasteLast => "paste-last",
            Item::Show => "show",
            Item::Settings => "settings",
            Item::Quit => "quit",
        }
    }

    fn action(self) -> Box<dyn Action> {
        match self {
            Item::Dictation => Box::new(ToggleDictation),
            Item::PasteLast => Box::new(PasteLastTranscript),
            Item::Show => Box::new(ShowHushpen),
            Item::Settings => Box::new(ShowSettings),
            Item::Quit => Box::new(QuitHushpen),
        }
    }
}

/// The menu for the pipeline as it is now. There are no separators, so a menu reader sees the
/// five entries and nothing else.
pub fn menu_items(listening: bool) -> Vec<MenuItem> {
    Item::ALL
        .into_iter()
        .map(|item| MenuItem::Action {
            name: item.label(listening).into(),
            action: item.action(),
            os_action: None,
            checked: false,
            disabled: false,
        })
        .collect()
}

/// The menu as the test hook reports it.
pub fn menu_json(listening: bool, created: bool) -> Value {
    json!({
        "created": created,
        "listening": listening,
        "items": Item::ALL
            .iter()
            .map(|item| json!({"label": item.label(listening), "id": item.name()}))
            .collect::<Vec<_>>(),
    })
}

pub fn listening(controller: &Entity<Controller>, cx: &App) -> bool {
    controller.read(cx).state() == State::Listening
}

/// Which glyph reads on the panel. macOS gets the black one as a template image, which the
/// system tints; Linux panels do not tint, so a dark panel gets the white glyph.
pub fn glyph_for(appearance: WindowAppearance) -> &'static [u8] {
    if cfg!(target_os = "macos") {
        return GLYPH_BLACK;
    }
    match appearance {
        WindowAppearance::Dark | WindowAppearance::VibrantDark => GLYPH_WHITE,
        WindowAppearance::Light | WindowAppearance::VibrantLight => GLYPH_BLACK,
    }
}

fn icon_for(appearance: WindowAppearance, cx: &App) -> Option<Icon> {
    let image = Image::from_bytes(ImageFormat::Png, glyph_for(appearance).to_vec());
    match Icon::from_gpui(&image, cx) {
        Ok(icon) => Some(icon),
        Err(error) => {
            log::warn!("the tray icon could not be read: {error}");
            None
        }
    }
}

/// What the actions need to reach.
#[derive(Clone)]
pub struct Surfaces {
    pub controller: Entity<Controller>,
    pub flow_bar: Entity<FlowBar>,
    pub shell: Entity<Shell>,
}

/// Wires every action of the tray and the app menu to what it does.
///
/// The listeners live in the app until the process ends, so they hold the surfaces weakly: a
/// strong handle there outlives the entity map and fails the leak check at exit.
pub fn register_actions(cx: &mut App, surfaces: &Surfaces) {
    let flow_bar = surfaces.flow_bar.downgrade();
    cx.on_action(move |_: &ToggleDictation, cx| {
        hook::record_event("tray", "toggle-dictation");
        let _ = flow_bar.update(cx, |bar, cx| bar.toggle(cx));
    });
    let controller = surfaces.controller.downgrade();
    cx.on_action(move |_: &PasteLastTranscript, cx| {
        hook::record_event("tray", "paste-last");
        let _ = controller.update(cx, |controller, cx| {
            if let Err(reason) = controller.dispatch(AppEvent::PasteLast, cx) {
                log::warn!("paste last from the tray was refused: {reason}");
            }
        });
    });
    cx.on_action(|_: &ShowHushpen, cx| {
        hook::record_event("tray", "show");
        main_window::show(cx);
    });
    let shell = surfaces.shell.downgrade();
    cx.on_action(move |_: &ShowSettings, cx| {
        hook::record_event("tray", "settings");
        open_view(&shell, View::Settings, cx);
    });
    let shell = surfaces.shell.downgrade();
    cx.on_action(move |_: &ShowAbout, cx| open_view(&shell, View::About, cx));
    cx.on_action(|_: &HideHushpen, cx| cx.hide());
    cx.on_action(|_: &QuitHushpen, cx| {
        hook::record_event("tray", "quit");
        cx.quit();
    });
}

fn open_view(shell: &WeakEntity<Shell>, view: View, cx: &mut App) {
    let _ = shell.update(cx, |shell, cx| shell.select(view, cx));
    main_window::show(cx);
}

/// Keeps the tray handle alive for the whole run. Dropping it removes the icon.
struct Held {
    _tray: Tray,
}

impl gpui_kit::Global for Held {}

/// Shows the tray icon. `None` when the platform refuses it; the app then runs with no tray and
/// the main window minimizes on close.
pub fn install(cx: &mut App, controller: &Entity<Controller>) -> Option<Tray> {
    let appearance = cx.window_appearance();
    let mut builder = Tray::builder().title(TITLE).tooltip(TITLE).menu({
        let controller = controller.downgrade();
        move |cx| {
            let listening = controller
                .upgrade()
                .is_some_and(|controller| listening(&controller, cx));
            menu_items(listening)
        }
    });
    if let Some(icon) = icon_for(appearance, cx) {
        builder = builder.icon(icon);
    }
    let tray = match builder.build(cx) {
        Ok(tray) => tray,
        Err(error) => {
            log::warn!("the tray icon could not be shown: {error}");
            return None;
        }
    };
    cx.set_global(Held {
        _tray: tray.clone(),
    });

    let mut last = listening(controller, cx);
    let live = tray.clone();
    cx.observe(controller, move |controller, cx| {
        let now = controller.read(cx).state() == State::Listening;
        if now != last {
            last = now;
            if let Err(error) = live.refresh_menu(cx) {
                log::warn!("the tray menu could not be updated: {error}");
            }
        }
    })
    .detach();

    #[cfg(target_os = "macos")]
    cx.spawn(async move |cx| {
        // The status item gets its button a moment after it is made.
        cx.background_executor()
            .timer(std::time::Duration::from_millis(300))
            .await;
        cx.update(|_| {
            let styled = hushpen_platform::tray_host::style_status_icon();
            if styled == 0 {
                log::warn!("the menu bar icon was not found, so it is not a template image");
            }
        });
    })
    .detach();
    Some(tray)
}

/// Swaps the Linux glyph when the panel goes from light to dark. The window reports the
/// system appearance.
pub fn follow_appearance(cx: &mut App, window: AnyWindowHandle, tray: &Tray) {
    let tray = tray.clone();
    let _ = window.update(cx, |_, window, _| {
        window
            .observe_window_appearance(move |window, cx| {
                if let Some(icon) = icon_for(window.appearance(), cx)
                    && let Err(error) = tray.set_icon(Some(icon), cx)
                {
                    log::warn!("the tray icon could not be swapped: {error}");
                }
            })
            .detach();
    });
}
