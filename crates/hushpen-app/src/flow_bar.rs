//! The flow bar: a small always-on-top pill that shows the dictation state, starts and stops a
//! dictation with a click, picks the language, and can be dragged.
//!
//! [`FlowBar`] is the model and the view. It never owns a window: a [`Host`] opens, moves, and
//! closes the native pop-up, and [`run`] feeds the host from the bar twenty times a second.

pub mod view;
mod window;

use crate::controller::{Controller, monotonic_ms};
use crate::dictation::Dictation;
use crate::hook;
use crate::mic::Mic;
use crate::native::native_handle;
use crate::storage::Storage;
use gpui_kit::{
    AnyWindowHandle, App, Bounds, Context, Entity, IntoElement, Pixels, Render, Window, point, px,
};
use hushpen_core::dictation::AppEvent;
use hushpen_core::flow_bar::{BarState, Display, Drag, Release, Spot, place, state_for};
use hushpen_platform::window::{Pointer, WindowHandle};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use view::{Language, Picker, Props};

pub use window::PopUp;

const SPOT_KEY: &str = "overlay.customPos";
/// How often the bar reads the pipeline, and how often while a press is down.
const TICK: Duration = Duration::from_millis(50);
const DRAG_TICK: Duration = Duration::from_millis(16);
/// An open language list closes by itself: the bar takes no keys, so it cannot close on Esc.
const PICKER_HOLD_MS: u64 = 10_000;
const DOT_STEP_MS: u64 = 280;
/// The size of the screen when the display server will not say.
const FALLBACK_SCREEN: (f32, f32) = (1280.0, 800.0);

/// What opens, moves, and closes the native window of the bar.
pub trait Host {
    /// The size of the primary screen in logical pixels.
    fn screen(&self, cx: &App) -> (f32, f32);
    /// Opens the window on first use, then moves and sizes it to `bounds`.
    fn show(&mut self, bounds: Bounds<Pixels>, cx: &mut App);
    fn hide(&mut self, cx: &mut App);
    fn window(&self) -> Option<AnyWindowHandle>;
}

pub type SharedHost = Rc<RefCell<Box<dyn Host>>>;

/// Opens the transcript of a saved row on the main window.
pub type OpenHistory = Rc<dyn Fn(&str, &mut App)>;

type PointerSource = Rc<dyn Fn(WindowHandle) -> Option<Pointer>>;

pub struct FlowBar {
    storage: Rc<Storage>,
    controller: Entity<Controller>,
    dictation: Entity<Dictation>,
    mic: Entity<Mic>,
    clock: Rc<dyn Fn() -> u64>,
    pointer: PointerSource,
    open_history: Option<OpenHistory>,

    display: Display,
    last_failure: Option<&'static str>,
    /// Why a click could not start a dictation. It shows like a failure, with no history row.
    refusal: Option<String>,
    /// When the language list opened.
    picker: Option<u64>,
    props: Props,
    visible: bool,
    origin: (f32, f32),
    screen: (f32, f32),

    pressed: bool,
    drag: Drag,
    drag_anchor: (f32, f32),
    drag_origin: Option<(f32, f32)>,
    native: WindowHandle,
    /// Device pixels per logical pixel, for the pointer. X11 only.
    scale: f32,
}

impl FlowBar {
    pub fn new(
        storage: Rc<Storage>,
        controller: Entity<Controller>,
        dictation: Entity<Dictation>,
        mic: Entity<Mic>,
    ) -> Self {
        Self {
            storage,
            controller,
            dictation,
            mic,
            clock: Rc::new(monotonic_ms),
            pointer: Rc::new(hushpen_platform::window::pointer),
            open_history: None,
            display: Display::default(),
            last_failure: None,
            refusal: None,
            picker: None,
            props: Props::new(BarState::Idle),
            visible: false,
            origin: (0.0, 0.0),
            screen: FALLBACK_SCREEN,
            pressed: false,
            drag: Drag::default(),
            drag_anchor: (0.0, 0.0),
            drag_origin: None,
            native: WindowHandle::Other,
            scale: 1.0,
        }
    }

    /// What "Open history" does: it gets the id of the newest saved row.
    pub fn on_open_history(&mut self, open: OpenHistory) {
        self.open_history = Some(open);
    }

    pub fn props(&self) -> &Props {
        &self.props
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Where the bar is: the top left corner and the size, in logical pixels.
    pub fn frame(&self) -> ((f32, f32), (f32, f32)) {
        (self.origin, view::size_of(&self.props))
    }

    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    fn flag(&self, key: &str) -> bool {
        self.storage
            .settings
            .get(key)
            .and_then(|value| value.as_bool())
            .unwrap_or(true)
    }

    fn spot(&self) -> Spot {
        let custom = self.storage.settings.get(SPOT_KEY).and_then(|value| {
            Some((
                value.get("x")?.as_f64()? as f32,
                value.get("y")?.as_f64()? as f32,
            ))
        });
        let position = self
            .storage
            .settings
            .get("overlay.position")
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default();
        Spot::from_settings(&position, custom)
    }

    fn language(&self, cx: &App) -> Language {
        let code = self.dictation.read(cx).language(cx);
        let reason = self.dictation.read(cx).picker_off_reason(cx);
        Language {
            label: code.to_uppercase(),
            enabled: reason.is_none(),
            reason: reason.map(str::to_owned),
        }
    }

    /// Reads the pipeline, the settings, and the pointer, and works out what to draw and where.
    /// `None` means the window should not exist.
    pub fn tick(&mut self, screen: (f32, f32), cx: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        let now = (self.clock)();
        self.screen = screen;
        self.follow_pointer(cx);

        let (derived, code, copied) = {
            let controller = self.controller.read(cx);
            (
                state_for(controller.state(), controller.failure_code()),
                controller.failure_code(),
                controller.copied_only(),
            )
        };
        if derived.is_failure() {
            self.last_failure = code;
        }
        if derived != BarState::Idle {
            self.refusal = None;
        }
        let shown = self.display.update(derived, now);
        if !shown.is_failure() {
            self.refusal = None;
        }
        if shown != BarState::Idle
            || self
                .picker
                .is_some_and(|since| now.saturating_sub(since) > PICKER_HOLD_MS)
        {
            self.picker = None;
        }

        let spot = self.spot();
        let below = spot.opens_downward(screen);
        let language = self.language(cx);
        let picker = match (self.picker, &language.reason) {
            (None, _) => Picker::Closed,
            (Some(_), Some(reason)) => Picker::Reason {
                text: reason.clone(),
                below,
            },
            (Some(_), None) => {
                let dictation = self.dictation.read(cx);
                Picker::List {
                    codes: dictation.picker_codes(),
                    selected: dictation.language(cx),
                    below,
                }
            }
        };
        let message = match shown {
            BarState::Result => view::result_text(copied).to_owned(),
            BarState::Error | BarState::Blocked => self
                .refusal
                .clone()
                .unwrap_or_else(|| view::failure_text(self.last_failure).to_owned()),
            _ => String::new(),
        };
        let props = Props {
            state: shown,
            levels: if shown == BarState::Listening {
                self.mic.read(cx).meter()
            } else {
                Vec::new()
            },
            dot: ((now / DOT_STEP_MS) % 3) as usize,
            language,
            picker,
            open_history: shown.is_failure()
                && self.refusal.is_none()
                && self.controller.read(cx).last_row_id().is_some(),
            message,
        };
        let size = view::size_of(&props);
        if props != self.props {
            self.props = props;
            cx.notify();
        }

        self.visible = self.flag("overlay.enabled")
            && (self.props.state != BarState::Idle
                || self.picker.is_some()
                || self.pressed
                || self.flag("overlay.idleVisible"));
        let at = match self.drag_origin {
            Some(origin) => origin,
            None => place(screen, size, spot),
        };
        self.origin = at;
        self.visible.then(|| {
            Bounds::new(
                point(px(at.0), px(at.1)),
                gpui_kit::size(px(size.0), px(size.1)),
            )
        })
    }

    fn sample(&self) -> Option<(f32, f32, bool)> {
        let pointer = (self.pointer)(self.native)?;
        Some((pointer.x / self.scale, pointer.y / self.scale, pointer.left))
    }

    fn apply(&mut self, at: (f32, f32)) {
        if let Some(delta) = self.drag.moved(at) {
            let (width, height) = view::size_of(&self.props);
            let x = (self.drag_anchor.0 + delta.0).clamp(0.0, (self.screen.0 - width).max(0.0));
            let y = (self.drag_anchor.1 + delta.1).clamp(0.0, (self.screen.1 - height).max(0.0));
            self.drag_origin = Some((x, y));
        }
    }

    /// Moves the bar with the pointer while the button is down, and ends the press when the
    /// button came up without the window hearing it.
    fn follow_pointer(&mut self, cx: &mut Context<Self>) {
        if !self.pressed {
            return;
        }
        match self.sample() {
            Some((_, _, false)) => self.release(cx),
            Some((x, y, true)) => self.apply((x, y)),
            None => {}
        }
    }

    /// The left button went down on the bar. It becomes a click or a drag when it comes up.
    pub fn press(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.native = native_handle(window);
        self.scale = if cfg!(target_os = "linux") {
            window.scale_factor()
        } else {
            1.0
        };
        self.press_at(cx);
    }

    fn press_at(&mut self, cx: &mut Context<Self>) {
        self.pressed = true;
        self.drag = Drag::default();
        self.drag_origin = None;
        if let Some((x, y, _)) = self.sample() {
            self.drag.press((x, y));
            self.drag_anchor = self.origin;
        }
        cx.notify();
    }

    /// The left button came up. A press that moved the bar saves the new position; any other
    /// press is a click.
    pub fn release(&mut self, cx: &mut Context<Self>) {
        if !self.pressed {
            return;
        }
        if let Some((x, y, _)) = self.sample() {
            self.apply((x, y));
        }
        self.pressed = false;
        let release = self.drag.release();
        self.drag = Drag::default();
        match release {
            Some(Release::Dragged) => self.finish_drag(cx),
            _ => self.click(cx),
        }
    }

    fn finish_drag(&mut self, cx: &mut Context<Self>) {
        let Some(origin) = self.drag_origin.take() else {
            return;
        };
        let size = view::size_of(&self.props);
        let Spot::Custom { center_x, edge_y } = Spot::dragged(origin, size, self.screen) else {
            return;
        };
        self.origin = origin;
        if let Err(error) = self
            .storage
            .settings
            .set_internal(SPOT_KEY, json!({"x": center_x, "y": edge_y}))
        {
            log::warn!("the flow bar position could not be saved: {error}");
        }
        hook::record_event("flowbar", &format!("moved {center_x:.0} {edge_y:.0}"));
        cx.notify();
    }

    fn click(&mut self, cx: &mut Context<Self>) {
        hook::record_event("flowbar", "click");
        self.toggle(cx);
    }

    /// Starts a dictation, or stops the one that runs: a click on the bar, or the tray entry. A
    /// start that preflight refuses shows its reason on the bar.
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.picker = None;
        let result = self.controller.update(cx, |controller, cx| {
            controller.dispatch(AppEvent::FlowBarClick, cx)
        });
        if let Err(reason) = result {
            self.refusal = Some(reason);
            self.display.update(BarState::Error, (self.clock)());
        }
        cx.notify();
    }

    /// The language chip: opens the list, or the reason the list is off, and closes it again.
    pub fn toggle_picker(&mut self, cx: &mut Context<Self>) {
        self.picker = match self.picker {
            Some(_) => None,
            None => Some((self.clock)()),
        };
        hook::record_event(
            "flowbar",
            if self.picker.is_some() {
                "picker open"
            } else {
                "picker closed"
            },
        );
        cx.notify();
    }

    /// An option of the list: `auto` or a whisper code.
    pub fn choose_language(&mut self, code: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let result = self
            .dictation
            .update(cx, |dictation, cx| dictation.set_language(code, cx));
        self.picker = None;
        cx.notify();
        result
    }

    /// The "Open history" button of a failure.
    pub fn open_history(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.controller.read(cx).last_row_id().map(str::to_owned) else {
            return;
        };
        hook::record_event("flowbar", "open history");
        self.display.clear();
        self.last_failure = None;
        if let Some(open) = self.open_history.clone() {
            // The main window reads state that this update holds, so it runs after the update.
            cx.defer(move |cx| open(&id, cx));
        }
        cx.notify();
    }

    /// `hookctl state` section `overlay`.
    pub fn state_json(&self) -> Value {
        let ((x, y), (width, height)) = self.frame();
        let picker = match &self.props.picker {
            Picker::Closed => "closed",
            Picker::List { .. } => "list",
            Picker::Reason { .. } => "reason",
        };
        json!({
            "state": self.props.state.key(),
            "visible": self.visible,
            "bounds": {"x": x, "y": y, "width": width, "height": height},
            "message": self.props.message,
            "language": {
                "label": self.props.language.label,
                "enabled": self.props.language.enabled,
                "reason": self.props.language.reason,
            },
            "picker": picker,
            "open_history": self.props.open_history,
            "dragging": self.drag.is_dragging(),
            "position": self.storage.settings.get("overlay.position"),
            "custom_position": self.storage.settings.get(SPOT_KEY),
        })
    }
}

impl Render for FlowBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        view::content(&self.props, Some(&cx.entity()))
    }
}

/// Keeps `host` in step with `bar` for as long as the app runs. Returns the host, which the
/// test hook reads the window from.
pub fn run(cx: &mut App, bar: Entity<FlowBar>, host: Box<dyn Host>) -> SharedHost {
    let host: SharedHost = Rc::new(RefCell::new(host));
    let driver = Rc::clone(&host);
    // The task lives until the app is dropped, so it holds the bar weakly: a strong handle would
    // outlive the entity map and fail the leak check when the app quits.
    let bar = bar.downgrade();
    cx.spawn(async move |cx| {
        loop {
            let wait = cx.update(|cx| {
                let screen = driver.borrow().screen(cx);
                let Ok((plan, pressed)) =
                    bar.update(cx, |bar, cx| (bar.tick(screen, cx), bar.is_pressed()))
                else {
                    return None;
                };
                match plan {
                    Some(bounds) => driver.borrow_mut().show(bounds, cx),
                    None => driver.borrow_mut().hide(cx),
                }
                Some(if pressed { DRAG_TICK } else { TICK })
            });
            let Some(wait) = wait else { break };
            cx.background_executor().timer(wait).await;
        }
    })
    .detach();
    host
}

#[cfg(test)]
mod tests;
