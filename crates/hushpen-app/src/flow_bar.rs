//! The flow bar: a small always-on-top pill that shows the dictation state, starts, finishes,
//! and cancels a dictation with a click, and can be dragged. It shows only while a dictation
//! runs or a result or notice is up, unless the user asks for the idle pill.
//!
//! [`FlowBar`] is the model and the view. It never owns a window: a [`Host`] opens, moves, and
//! closes the native pop-up, and [`run`] feeds the host from the bar: every frame while the bar
//! moves, twenty times a second while it rests.

mod motion;
pub mod view;
mod window;

use crate::ai_cleanup::Phase;
use crate::controller::{Controller, monotonic_ms};
use crate::hook;
use crate::mic::Mic;
use crate::native::native_handle;
use crate::storage::Storage;
use gpui_kit::{
    AnyWindowHandle, App, Bounds, Context, Entity, IntoElement, Pixels, Render, WeakEntity, Window,
    point, px,
};
use hushpen_core::dictation::{AppEvent, State};
use hushpen_core::flow_bar::{BarState, Display, Drag, Release, Spot, place, state_for, with_undo};
use hushpen_platform::window::{Pointer, WindowHandle};
use motion::{Goal, Motion};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use view::Props;

pub use window::PopUp;

const SPOT_KEY: &str = "overlay.customPos";
/// How often the bar reads the pipeline while it rests, and while it moves or a press is down.
const TICK: Duration = Duration::from_millis(50);
const FRAME: Duration = Duration::from_millis(16);
/// The screen when the display server will not say.
const FALLBACK_SCREEN: Screen = Screen {
    origin: (0.0, 0.0),
    size: (1280.0, 800.0),
};

/// One screen in logical pixels: its top left corner from the top left of the primary screen,
/// and its size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Screen {
    pub origin: (f32, f32),
    pub size: (f32, f32),
}

/// What opens, moves, and closes the native window of the bar.
pub trait Host {
    /// The screen that holds the pointer, in logical pixels.
    fn screen(&self, cx: &App) -> Screen;
    /// Opens the window on first use, and plans the move and resize to `bounds`. The native
    /// call that moves the window is not made here but returned: AppKit reports a move or a
    /// resize while the call runs, and GPUI answers with an update of the App, which fails when
    /// the App is still borrowed (it logs `RefCell already borrowed` and loses the update).
    fn show(&mut self, bounds: Bounds<Pixels>, cx: &mut App) -> Option<Native>;
    fn hide(&mut self, cx: &mut App);
    fn window(&self) -> Option<AnyWindowHandle>;
}

/// Native window work that [`run`] does after the App is free again.
pub type Native = Box<dyn FnOnce()>;

pub type SharedHost = Rc<RefCell<Box<dyn Host>>>;

/// Opens the transcript of a saved row on the main window.
pub type OpenHistory = Rc<dyn Fn(&str, &mut App)>;

type PointerSource = Rc<dyn Fn(WindowHandle) -> Option<Pointer>>;

pub struct FlowBar {
    storage: Rc<Storage>,
    controller: Entity<Controller>,
    mic: Entity<Mic>,
    clock: Rc<dyn Fn() -> u64>,
    pointer: PointerSource,
    open_history: Option<OpenHistory>,
    /// Off in tests and with Reduce Motion: every change shows at once.
    animate: bool,
    motion: Motion,

    display: Display,
    last_failure: Option<&'static str>,
    /// Why a click could not start a dictation. It shows like a failure, with no history row.
    refusal: Option<String>,
    /// The newest refused Command Mode start the bar has shown.
    seen_flash: u64,
    props: Props,
    visible: bool,
    origin: (f32, f32),
    /// The screen the bar is on. It is chosen when the bar appears or a dictation starts, so
    /// the bar does not jump to another screen in the middle of a dictation.
    screen: Screen,

    pressed: bool,
    drag: Drag,
    drag_anchor: (f32, f32),
    drag_origin: Option<(f32, f32)>,
    native: WindowHandle,
    /// Device pixels per logical pixel, for the pointer. X11 only.
    scale: f32,
}

impl FlowBar {
    pub fn new(storage: Rc<Storage>, controller: Entity<Controller>, mic: Entity<Mic>) -> Self {
        Self {
            storage,
            controller,
            mic,
            clock: Rc::new(monotonic_ms),
            pointer: Rc::new(hushpen_platform::window::pointer),
            open_history: None,
            animate: true,
            motion: Motion::default(),
            display: Display::default(),
            last_failure: None,
            refusal: None,
            seen_flash: 0,
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
        (self.origin, self.motion.size())
    }

    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// The bar moves on its own: it changes size, fades, or draws a live waveform or dots.
    fn needs_frames(&self) -> bool {
        self.animate
            && self.visible
            && (!self.motion.is_settled()
                || matches!(
                    self.props.state,
                    BarState::Listening | BarState::Transcribing
                ))
    }

    fn flag(&self, key: &str, default: bool) -> bool {
        self.storage
            .settings
            .get(key)
            .and_then(|value| value.as_bool())
            .unwrap_or(default)
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

    /// Reads the pipeline, the settings, and the pointer, and works out what to draw and where.
    /// `None` means the window should not exist.
    pub fn tick(&mut self, screen: Screen, cx: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        let now = (self.clock)();
        let was_idle = self.props.state == BarState::Idle;
        if !self.visible {
            self.screen = screen;
        }
        self.follow_pointer(cx);

        let (derived, code, copied, offered, preparing, flash) = {
            let controller = self.controller.read(cx);
            (
                state_for(controller.state(), controller.failure_code()),
                controller.failure_code(),
                controller.copied_only(),
                controller.undo_offered(),
                controller.ai_phase() == Some(Phase::Preparing),
                controller
                    .flash()
                    .map(|flash| (flash.seq, flash.message.clone())),
            )
        };
        if derived.is_failure() {
            self.last_failure = code;
        }
        if derived != BarState::Idle {
            self.refusal = None;
        }
        if let Some((seq, message)) = flash
            && seq != self.seen_flash
        {
            self.seen_flash = seq;
            self.refusal = Some(message);
            self.display.update(BarState::Blocked, now);
        }
        let shown = with_undo(self.display.update(derived, now), offered);
        if !shown.is_failure() {
            self.refusal = None;
        }

        let wanted = self.flag("overlay.enabled", true)
            && (shown != BarState::Idle || self.pressed || self.flag("overlay.idleVisible", false));
        if wanted && was_idle && shown != BarState::Idle && !self.pressed {
            self.screen = screen;
        }
        // A bar on its way out keeps what it showed last: the idle pill must not flash up as
        // a result fades.
        let mut props = if wanted {
            let message = match shown {
                BarState::Result | BarState::Undo => view::result_text(copied).to_owned(),
                BarState::Error | BarState::Blocked => self
                    .refusal
                    .clone()
                    .unwrap_or_else(|| view::failure_text(self.last_failure).to_owned()),
                _ => String::new(),
            };
            Props {
                state: shown,
                message,
                open_history: shown.is_failure()
                    && self.refusal.is_none()
                    && self.controller.read(cx).last_row_id().is_some(),
                preparing: shown == BarState::Transcribing && preparing,
                ..Props::new(shown)
            }
        } else {
            self.props.clone()
        };
        let wave = if props.state == BarState::Listening {
            view::wave_goal(&self.mic.read(cx).meter())
        } else {
            Vec::new()
        };
        let goal_size = view::size_of(&props);
        self.motion.update(
            now,
            Goal {
                size: goal_size,
                shown: wanted,
                new_state: wanted && props.state != self.props.state,
                wave: &wave,
            },
            self.animate && !cx.reduce_motion(),
        );
        let size = self.motion.size();
        props.levels = self.motion.wave().to_vec();
        props.phase = (now % view::DOT_CYCLE_MS) as f32 / view::DOT_CYCLE_MS as f32;
        props.frame = Some(size);
        props.presence = self.motion.presence();
        props.fade = self.motion.fade();
        if props != self.props {
            self.props = props;
            cx.notify();
        }

        self.visible = wanted || !self.motion.is_gone();
        // The last look is only kept while the bar fades: a gone bar is idle, so the state
        // section and its change event do not keep reporting a result or an expired offer.
        if !self.visible && self.props.state != BarState::Idle {
            self.props = Props::new(BarState::Idle);
            cx.notify();
        }
        // The spot comes from the goal size, so the bar grows and shrinks around a still
        // center and lands on the same pixels every time.
        let at = match self.drag_origin {
            Some(origin) => origin,
            None => {
                let (x, y) = place(self.screen.size, goal_size, self.spot());
                (
                    self.screen.origin.0 + x + (goal_size.0 - size.0) / 2.0,
                    self.screen.origin.1 + y + (goal_size.1 - size.1) / 2.0,
                )
            }
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
            let (width, height) = self.motion.size();
            let Screen { origin, size } = self.screen;
            let x = (self.drag_anchor.0 + delta.0)
                .clamp(origin.0, origin.0 + (size.0 - width).max(0.0));
            let y = (self.drag_anchor.1 + delta.1)
                .clamp(origin.1, origin.1 + (size.1 - height).max(0.0));
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
        // The spot is kept from the corner of the screen, so it holds on every screen.
        let local = (
            origin.0 - self.screen.origin.0,
            origin.1 - self.screen.origin.1,
        );
        let Spot::Custom { center_x, edge_y } = Spot::dragged(local, size, self.screen.size) else {
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
        self.send(AppEvent::FlowBarClick, cx);
    }

    /// The tick: the words go in, as when the hold key comes up.
    pub fn finish(&mut self, cx: &mut Context<Self>) {
        hook::record_event("flowbar", "finish");
        if self.controller.read(cx).state() == State::Listening {
            self.send(AppEvent::FlowBarClick, cx);
        }
    }

    /// The cross: the recording is thrown away, as with Esc.
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        hook::record_event("flowbar", "cancel");
        self.send(AppEvent::Esc, cx);
    }

    fn send(&mut self, event: AppEvent, cx: &mut Context<Self>) {
        let result = self
            .controller
            .update(cx, |controller, cx| controller.dispatch(event, cx));
        if let Err(reason) = result {
            self.refusal = Some(reason);
            self.display.update(BarState::Error, (self.clock)());
        }
        cx.notify();
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

    /// The "Undo cleanup" button: the app's undo key, then the rule text. A refusal shows on
    /// the bar like a refused start.
    pub fn press_undo(&mut self, cx: &mut Context<Self>) {
        if let Err(reason) = self.undo_cleanup(cx) {
            self.refusal = Some(reason);
            self.display.update(BarState::Error, (self.clock)());
        }
        cx.notify();
    }

    /// The same without a notice on the bar, for the test hook.
    pub fn undo_cleanup(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        hook::record_event("flowbar", "undo cleanup");
        self.controller
            .update(cx, |controller, cx| controller.undo_cleanup(cx))
    }

    /// `hookctl state` section `overlay`.
    pub fn state_json(&self) -> Value {
        let ((x, y), (width, height)) = self.frame();
        json!({
            "state": self.props.state.key(),
            "visible": self.visible,
            "bounds": {"x": x, "y": y, "width": width, "height": height},
            "settled": self.motion.is_settled(),
            "message": self.props.message,
            "open_history": self.props.open_history,
            "dragging": self.drag.is_dragging(),
            "position": self.storage.settings.get("overlay.position"),
            "custom_position": self.storage.settings.get(SPOT_KEY),
        })
    }
}

impl Render for FlowBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let composited = crate::theme::chrome::compositor(cx);
        view::content(&self.props, Some(&cx.entity()), composited)
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
            let step = cx.update(|cx| drive(cx, &driver, &bar));
            let Some((wait, native)) = step else { break };
            if let Some(native) = native {
                native();
            }
            cx.background_executor().timer(wait).await;
        }
    })
    .detach();
    host
}

/// One tick: reads the bar and opens, plans, or closes the window. Returns how long to wait and
/// the native work to do once the App is no longer borrowed, or `None` when the bar is gone.
fn drive(
    cx: &mut App,
    driver: &SharedHost,
    bar: &WeakEntity<FlowBar>,
) -> Option<(Duration, Option<Native>)> {
    let screen = driver.borrow().screen(cx);
    let Ok((plan, fast)) = bar.update(cx, |bar, cx| {
        let plan = bar.tick(screen, cx);
        (plan, bar.is_pressed() || bar.needs_frames())
    }) else {
        return None;
    };
    let native = match plan {
        Some(bounds) => driver.borrow_mut().show(bounds, cx),
        None => {
            driver.borrow_mut().hide(cx);
            None
        }
    };
    Some((if fast { FRAME } else { TICK }, native))
}

#[cfg(test)]
mod tests;
