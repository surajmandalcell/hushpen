//! The prompt sheet of Settings > Cleanup: what the cleanup model reads before each text, for the
//! chosen profile and the model in use. Qwen and endpoints get an editable prompt; S1-mini shows
//! its fixed prompt and the three values it was trained on. The sheet builds its parts with
//! [`cleanup_prompt`], the function that builds what a cleanup sends, so it shows what is sent.

use super::Settings;
use crate::ai_cleanup::{self, PROFILE_EDITS_SETTING, PROFILE_SETTING};
use crate::theme::{self, Palette, SECONDARY, StyledType, TOOLBAR_TITLE, radius, size, space};
use crate::ui::{self, Kind, Notice};
use crate::{hook, motion};
use gpui_kit::TestSupportExt as _;
use gpui_kit::base::FocusTrapElement as _;
use gpui_kit::component::input::{InputEvent, Textarea, TextareaState};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Role, SharedString, StatefulInteractiveElement,
    Styled, Window, div, px,
};
use hushpen_core::llm_prompt::{
    Context as Layout, PROMPT_MAX_CHARS, Profile, PromptFamily, Structure, Styling, Target, Tuned,
    cleanup_prompt, keep_sentence, valid_prompt,
};
use serde_json::{Value, json};

/// Stands for the dictionary words of a text, which only a dictation has.
const WORDS: &str = "<the dictionary words in the text>";

/// The pickers of the S1-mini values, by the hook id of their buttons.
pub const STYLING: &str = "cleanup.prompt.styling";
pub const STRUCTURE: &str = "cleanup.prompt.structure";
pub const CONTEXT: &str = "cleanup.prompt.context";

fn id(name: &str) -> SharedString {
    hook::id("settings", &format!("cleanup.prompt.{name}"))
}

/// The focus handles of the sheet. `trap` is the sheet itself, which keeps Tab inside it.
pub struct SheetFocus {
    pub trap: FocusHandle,
    pub styling: FocusHandle,
    pub structure: FocusHandle,
    pub context: FocusHandle,
    pub reset: FocusHandle,
    pub done: FocusHandle,
}

/// The open sheet, or `None` in `target` while it is closed.
pub struct PromptSheet {
    /// Where the cleanup runs, read when the sheet opens.
    target: Option<Target>,
    /// The prompt as the user types it. Built the first time the sheet opens.
    text: Option<Entity<TextareaState>>,
    return_focus: Option<FocusHandle>,
    pub focus: SheetFocus,
}

impl PromptSheet {
    pub fn new(cx: &mut App) -> Self {
        let stop = |cx: &mut App| cx.focus_handle().tab_stop(true);
        Self {
            target: None,
            text: None,
            return_focus: None,
            focus: SheetFocus {
                trap: cx.focus_handle(),
                styling: stop(cx),
                structure: stop(cx),
                context: stop(cx),
                reset: stop(cx),
                done: stop(cx),
            },
        }
    }
}

/// Whether S1-mini runs the cleanup there: it has a fixed prompt, so only its values change.
fn fixed(target: Target) -> bool {
    target == Target::Local(PromptFamily::S1Mini)
}

fn target_key(target: Target) -> &'static str {
    match target {
        Target::Local(family) => family.as_str(),
        Target::Endpoint => "endpoint",
    }
}

impl Settings {
    /// The profile cleanup uses now.
    pub(crate) fn cleanup_profile(&self) -> Profile {
        ai_cleanup::profile(&self.storage).profile
    }

    pub fn set_cleanup_profile(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some(profile) = Profile::parse(key) {
            self.set(PROFILE_SETTING, json!(profile.key()), cx);
        }
        self.picker = None;
        cx.notify();
    }

    /// Where cleanup runs with the settings as they are: the endpoint, or the chosen local
    /// model, whose prompt family sets the form of the prompt.
    pub(crate) fn cleanup_target(&self) -> Target {
        if self.ai_provider() == "endpoint" {
            return Target::Endpoint;
        }
        let family = self.ai_model().map_or(PromptFamily::Qwen35, |model| {
            ai_cleanup::family_of(&model.id)
        });
        Target::Local(family)
    }

    /// Opens the prompt sheet for the chosen profile and the model in use, with the focus on
    /// Done. The Prompt button gets the focus back when it closes.
    pub fn open_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.cleanup_target();
        let tuned = ai_cleanup::profile(&self.storage);
        let text = match self.prompt.text.clone() {
            Some(text) => text,
            None => {
                let text = cx.new(|cx| TextareaState::new(window, cx));
                cx.subscribe_in(&text, window, |me, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        me.prompt_typed(cx);
                    }
                })
                .detach();
                self.prompt.text = Some(text.clone());
                text
            }
        };
        let shown = cleanup_prompt(target, &tuned, &[]).prompt;
        // Closed while the value is set, so the change it causes saves nothing.
        self.prompt.target = None;
        text.update(cx, |text, cx| text.set_value(shown, window, cx));
        self.prompt.target = Some(target);
        if self.prompt.return_focus.is_none() {
            self.prompt.return_focus = Some(self.control_focus[super::slots::PROMPT].clone());
        }
        self.picker = None;
        window.focus(&self.prompt.focus.done, cx);
        hook::record_event("settings", &format!("prompt sheet {}", target_key(target)));
        cx.notify();
    }

    /// Closes the sheet and gives the focus back to the Prompt button.
    pub fn close_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.prompt.target.is_none() {
            return;
        }
        self.dismiss_prompt(cx);
        if let Some(focus) = self.prompt.return_focus.take() {
            window.focus(&focus, cx);
        }
    }

    /// Closes the sheet without moving the focus, for a view change that hides it.
    pub fn dismiss_prompt(&mut self, cx: &mut Context<Self>) {
        if self.prompt.target.take().is_some() {
            if self
                .picker
                .is_some_and(|open| open.starts_with("cleanup.prompt."))
            {
                self.picker = None;
            }
            hook::record_event("settings", "prompt sheet closed");
            cx.notify();
        }
    }

    /// The text area changed: its text becomes the prompt of the profile. A blank or too long
    /// text keeps nothing, so the built-in prompt is sent, as the sheet says.
    fn prompt_typed(&mut self, cx: &mut Context<Self>) {
        let (Some(target), Some(text)) = (self.prompt.target, self.prompt.text.as_ref()) else {
            return;
        };
        if fixed(target) {
            return;
        }
        let typed = text.read(cx).value().to_string();
        let mut tuned = ai_cleanup::profile(&self.storage);
        tuned.prompt = if valid_prompt(&typed) {
            typed
        } else {
            tuned.profile.prompt().to_owned()
        };
        self.save_profile(&tuned, cx);
    }

    /// Types `text` into the prompt like a user does.
    pub fn type_prompt(&mut self, typed: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.prompt.text.clone() {
            text.update(cx, |text, cx| text.set_value(typed.to_owned(), window, cx));
        }
        self.prompt_typed(cx);
    }

    /// Sets one S1-mini value of the profile: `picker` names which, `key` is the trained word.
    pub fn set_prompt_control(&mut self, picker: &str, key: &str, cx: &mut Context<Self>) {
        let mut tuned = ai_cleanup::profile(&self.storage);
        let controls = &mut tuned.controls;
        match picker {
            STYLING => controls.styling = Styling::parse(key).unwrap_or(controls.styling),
            STRUCTURE => controls.structure = Structure::parse(key).unwrap_or(controls.structure),
            CONTEXT => controls.context = Layout::parse(key).unwrap_or(controls.context),
            _ => {}
        }
        self.save_profile(&tuned, cx);
        self.picker = None;
        cx.notify();
    }

    /// Puts back the built-in values the sheet shows: the three values for S1-mini, the prompt
    /// for the others.
    pub fn reset_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.prompt.target else {
            return;
        };
        let mut tuned = ai_cleanup::profile(&self.storage);
        let built_in = Tuned::built_in(tuned.profile);
        if fixed(target) {
            tuned.controls = built_in.controls;
        } else {
            tuned.prompt = built_in.prompt.clone();
            if let Some(text) = self.prompt.text.clone() {
                self.prompt.target = None;
                text.update(cx, |text, cx| text.set_value(built_in.prompt, window, cx));
                self.prompt.target = Some(target);
            }
        }
        self.save_profile(&tuned, cx);
        self.picker = None;
        hook::record_event("settings", "prompt reset");
    }

    fn save_profile(&mut self, tuned: &Tuned, cx: &mut Context<Self>) {
        let edits = ai_cleanup::edits_with(&self.storage, tuned);
        if self.storage.settings.get(PROFILE_EDITS_SETTING).as_ref() != Some(&edits) {
            self.set(PROFILE_EDITS_SETTING, edits, cx);
        }
    }

    /// `hookctl state` part of the cleanup profile: the profile, whether it differs from its
    /// built-in values, and the open sheet with what it shows.
    pub(crate) fn profile_state_json(&self, cx: &App) -> Value {
        let tuned = ai_cleanup::profile(&self.storage);
        let sheet = self.prompt.target.map(|target| {
            let shown = cleanup_prompt(target, &tuned, &[]);
            json!({
                "target": target_key(target),
                "prompt": shown.prompt,
                "editable": !fixed(target),
                "text": self.prompt.text.as_ref().map(|text| text.read(cx).value().to_string()),
                "control": shown.control.map(|controls| controls.line()),
            })
        });
        json!({
            "key": tuned.profile.key(),
            "edited": tuned.edited(),
            "target": target_key(self.cleanup_target()),
            "sheet": sheet,
        })
    }
}

/// The sheet, or nothing while it is closed. It comes in like every sheet: the scrim fades in,
/// and the sheet fades in and rises a few pixels into place.
pub fn render(
    settings: &Entity<Settings>,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let target = settings.read(cx).prompt.target?;
    let arrival = motion::enter(id("sheet.arrival"), motion::BASE, window, cx);
    let palette = theme::palette(cx);
    let close = {
        let settings = settings.clone();
        move |window: &mut Window, cx: &mut App| {
            settings.update(cx, |me, cx| me.close_prompt(window, cx));
        }
    };
    let on_key = {
        let close = close.clone();
        move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            if event.keystroke.key.as_str() == "escape" {
                close(window, cx);
                cx.stop_propagation();
            }
        }
    };
    let me = settings.read(cx);
    let tuned = ai_cleanup::profile(&me.storage);
    let built_in = Tuned::built_in(tuned.profile);
    let sent = cleanup_prompt(target, &tuned, &[WORDS.to_owned()]);
    let title = format!("{} prompt", tuned.profile.label());
    let line = match target {
        Target::Local(PromptFamily::S1Mini) => {
            "S1-mini works only with this exact prompt. Pick how the text reads.".to_owned()
        }
        Target::Local(_) => {
            let model = me
                .ai_model()
                .map_or_else(|| "The model".to_owned(), |m| m.name);
            format!("{model} reads this before each text.")
        }
        Target::Endpoint => "The endpoint reads this before each text.".to_owned(),
    };
    let mut body = vec![
        header(palette, title.clone(), line),
        text_group(palette, me, target, cx),
    ];
    if let Some(controls) = sent.control {
        body.push(
            ui::group(
                id("controls"),
                palette,
                vec![
                    control_row(
                        palette,
                        settings,
                        me,
                        STYLING,
                        "Styling",
                        "How formal the text reads.",
                        controls.styling.label(),
                        Styling::ALL.iter().map(|v| (v.key(), v.label())).collect(),
                        controls.styling.key(),
                        &me.prompt.focus.styling,
                    ),
                    control_row(
                        palette,
                        settings,
                        me,
                        STRUCTURE,
                        "Structure",
                        "Lists put three or more items on their own lines.",
                        controls.structure.label(),
                        Structure::ALL
                            .iter()
                            .map(|v| (v.key(), v.label()))
                            .collect(),
                        controls.structure.key(),
                        &me.prompt.focus.structure,
                    ),
                    control_row(
                        palette,
                        settings,
                        me,
                        CONTEXT,
                        "Context",
                        "Email keeps a greeting and a sign-off on their own lines.",
                        controls.context.label(),
                        Layout::ALL.iter().map(|v| (v.key(), v.label())).collect(),
                        controls.context.key(),
                        &me.prompt.focus.context,
                    ),
                ],
            )
            .into_any_element(),
        );
    }
    body.push(also_sent(palette, &sent));
    let changed = if fixed(target) {
        tuned.controls != built_in.controls
    } else {
        tuned.prompt != built_in.prompt
    };
    let reset = {
        let settings = settings.clone();
        move |window: &mut Window, cx: &mut App| {
            settings.update(cx, |me, cx| me.reset_prompt(window, cx));
        }
    };
    let footer = div()
        .flex()
        .items_center()
        .justify_end()
        .gap(px(space::SM))
        .child(ui::button(
            palette,
            id("reset"),
            "Reset to Default",
            Kind::Plain,
            Some(&me.prompt.focus.reset),
            changed,
            reset,
        ))
        .child(ui::button(
            palette,
            id("done"),
            "Done",
            Kind::Default,
            Some(&me.prompt.focus.done),
            true,
            close,
        ));
    let sheet = div()
        .id(id("sheet"))
        .test_support()
        .role(Role::Dialog)
        .aria_label(title)
        .on_key_down(on_key)
        .flex()
        .flex_col()
        .gap(px(space::LG))
        .w(px(space::SHEET_WIDTH))
        .p(px(space::XL))
        .bg(palette.content)
        .rounded(px(radius::WINDOW))
        .border(theme::hairline())
        .border_color(palette.separator)
        .children(body)
        .child(footer);
    let trap = me.prompt.focus.trap.clone();
    Some(
        div()
            .focus_trap(id("sheet.scrim"), &trap)
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::faded(palette.scrim, arrival))
            .child(motion::arrive(sheet, arrival, motion::TRAVEL))
            .into_any_element(),
    )
}

/// The title of the sheet and one line under it.
fn header(palette: Palette, title: String, line: String) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(space::XS))
        .child(
            div()
                .id(id("sheet.title"))
                .test_support()
                .role(Role::Heading)
                .aria_label(title.clone())
                .text_token(TOOLBAR_TITLE)
                .text_color(palette.label)
                .truncate()
                .child(title),
        )
        .child(
            div()
                .id(id("sheet.line"))
                .test_support()
                .aria_label(line.clone())
                .text_token(SECONDARY)
                .text_color(palette.label_secondary)
                .truncate()
                .child(line),
        )
        .into_any_element()
}

/// The prompt in its text area, editable unless the model has a fixed one, with its length or
/// the reason it is not sent under it.
fn text_group(palette: Palette, me: &Settings, target: Target, cx: &App) -> AnyElement {
    let mut rows = Vec::new();
    if let Some(text) = &me.prompt.text {
        rows.push(
            div()
                .id(id("text"))
                .test_support()
                .p(px(size::ROW_PADDING))
                .child(
                    Textarea::new(text)
                        .aria_label("Prompt")
                        .readonly(fixed(target))
                        .h(px(size::TEXT_AREA_HEIGHT))
                        .bg(palette.control)
                        .rounded(px(radius::BUTTON))
                        .border_color(gpui_kit::transparent_black()),
                )
                .into_any_element(),
        );
        if !fixed(target) {
            let typed = text.read(cx).value().to_string();
            let chars = typed.chars().count();
            rows.push(if typed.trim().is_empty() {
                ui::notice_row(
                    palette,
                    id("notice"),
                    "The prompt is empty, so the default one is sent.".into(),
                    Notice::Failure,
                    None,
                )
            } else if chars > PROMPT_MAX_CHARS {
                ui::notice_row(
                    palette,
                    id("notice"),
                    format!("Over {PROMPT_MAX_CHARS} characters, so the default one is sent."),
                    Notice::Failure,
                    None,
                )
            } else {
                ui::labeled_row(
                    palette,
                    id("count"),
                    format!("{chars} of {PROMPT_MAX_CHARS} characters"),
                    None,
                    div(),
                )
            });
        }
    }
    ui::group(id("prompt"), palette, rows).into_any_element()
}

/// What goes with the prompt: the S1-mini line of values, the dictionary words, the worked
/// examples, and how an endpoint gets the text.
fn also_sent(palette: Palette, sent: &hushpen_core::llm_prompt::CleanupPrompt) -> AnyElement {
    let fact = |key: &str, title: &str, value: String| {
        ui::fact_row(
            palette,
            id(&format!("row.{key}")),
            id(key),
            title,
            Some(value),
            None,
        )
    };
    let mut rows = Vec::new();
    match sent.control {
        Some(controls) => {
            rows.push(fact("controlLine", "Control line", controls.line()));
            rows.push(fact(
                "words",
                "Dictionary words",
                "Not sent. The check after cleanup keeps them.".into(),
            ));
        }
        None => {
            let words = keep_sentence(&[WORDS.to_owned()]).unwrap_or_default();
            let mut added = sent.added.iter();
            if let Some(sentence) = added.next().filter(|sentence| **sentence == words) {
                rows.push(fact("words", "Dictionary words", sentence.clone()));
            }
            for sentence in added {
                rows.push(fact("format", "Text format", sentence.clone()));
            }
            if !sent.examples.is_empty() {
                rows.push(fact(
                    "examples",
                    "Examples",
                    format!("{} cleanups, sent as earlier turns", sent.examples.len()),
                ));
            }
        }
    }
    ui::group(id("added"), palette, rows).into_any_element()
}

/// A row with a pop-up of the trained values of one S1-mini control.
#[allow(clippy::too_many_arguments)]
fn control_row(
    palette: Palette,
    settings: &Entity<Settings>,
    me: &Settings,
    picker: &'static str,
    title: &'static str,
    detail: &str,
    current: &str,
    choices: Vec<(&'static str, &'static str)>,
    chosen: &str,
    focus: &FocusHandle,
) -> AnyElement {
    let menu = (me.picker == Some(picker)).then(|| {
        let close = settings.clone();
        let menu = ui::Menu::new(hook::id("settings", &format!("{picker}.list")), move |cx| {
            close.update(cx, |me, cx| me.close_picker(cx))
        });
        choices
            .into_iter()
            .zip(&me.option_focus)
            .fold(menu, |menu, ((key, label), focus)| {
                let settings = settings.clone();
                menu.option(
                    hook::id("settings", &format!("{picker}.option.{key}")),
                    label,
                    None,
                    key == chosen,
                    focus,
                    move |_, cx| {
                        settings.update(cx, |me, cx| me.set_prompt_control(picker, key, cx))
                    },
                )
            })
    });
    let toggle = settings.clone();
    ui::menu_row(
        palette,
        detail,
        ui::PopUp {
            id: hook::id("settings", picker),
            label: title,
            current: current.to_owned(),
            enabled: true,
            focus,
            menu,
        },
        move |cx| toggle.update(cx, |me, cx| me.toggle_picker(picker, cx)),
    )
}
