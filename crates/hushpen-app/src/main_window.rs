//! Where the main window goes when it is not in front: hidden to the tray, or minimized when no
//! tray can show it again.
//!
//! The close button, the window manager's close request, and History's Re-paste call
//! [`close_requested`] or [`step_aside`]. The tray menu, a second start of the app, and the
//! flow bar's "Open history" call [`show`].

use gpui_kit::{AnyWindowHandle, App, Global, Window};
use hushpen_platform::window::{self, WindowHandle};
use std::rc::Rc;

/// The native side of the window. The platform crate does it; tests use a recording stand-in.
pub trait Native {
    fn hide(&self) -> Result<(), String>;
    fn show(&self) -> Result<(), String>;
    fn minimize(&self, window: &mut Window);
}

/// How the window is away from the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Away {
    /// Off the screen; only the tray brings it back.
    Hidden,
    /// In the task bar or Dock; the window manager brings it back.
    Minimized,
}

impl Away {
    pub fn key(self) -> &'static str {
        match self {
            Away::Hidden => "hidden",
            Away::Minimized => "minimized",
        }
    }
}

pub struct MainWindow {
    handle: AnyWindowHandle,
    native: Rc<dyn Native>,
    /// Asked at each close, because a tray host can start or stop while the app runs.
    tray_host: Rc<dyn Fn() -> bool>,
    away: Option<Away>,
}

impl Global for MainWindow {}

struct Platform(WindowHandle);

impl Native for Platform {
    fn hide(&self) -> Result<(), String> {
        window::hide(self.0).map_err(|error| error.to_string())
    }

    fn show(&self) -> Result<(), String> {
        window::show(self.0).map_err(|error| error.to_string())
    }

    fn minimize(&self, window: &mut Window) {
        window.minimize_window();
    }
}

/// Registers the main window. `tray_host` says whether a tray can bring it back.
pub fn install(
    cx: &mut App,
    handle: AnyWindowHandle,
    native: WindowHandle,
    tray_host: Rc<dyn Fn() -> bool>,
) {
    install_with(cx, handle, Rc::new(Platform(native)), tray_host);
}

pub fn install_with(
    cx: &mut App,
    handle: AnyWindowHandle,
    native: Rc<dyn Native>,
    tray_host: Rc<dyn Fn() -> bool>,
) {
    cx.set_global(MainWindow {
        handle,
        native,
        tray_host,
        away: None,
    });
}

/// How the window is away from the screen, or `None` while it shows.
pub fn away(cx: &App) -> Option<Away> {
    cx.try_global::<MainWindow>().and_then(|main| main.away)
}

/// Picks hide or minimize, and hides when it can. A hide that fails (a backend that cannot
/// hide) becomes a minimize, so the window never vanishes with no way back. `None` when no main
/// window is registered.
fn pick(cx: &mut App) -> Option<(Away, Rc<dyn Native>, AnyWindowHandle)> {
    let main = cx.try_global::<MainWindow>()?;
    let hidden = (main.tray_host)()
        && match main.native.hide() {
            Ok(()) => true,
            Err(reason) => {
                log::warn!("the window could not be hidden, so it is minimized: {reason}");
                false
            }
        };
    let away = if hidden {
        Away::Hidden
    } else {
        Away::Minimized
    };
    let (native, handle) = (Rc::clone(&main.native), main.handle);
    cx.global_mut::<MainWindow>().away = Some(away);
    Some((away, native, handle))
}

/// The window was asked to close, by its button, a key, or the window manager. It hides to the
/// tray, or minimizes when there is no tray. With no main window registered the app quits.
pub fn close_requested(window: &mut Window, cx: &mut App) {
    match pick(cx) {
        None => cx.quit(),
        Some((Away::Hidden, ..)) => {}
        Some((Away::Minimized, native, _)) => native.minimize(window),
    }
}

/// Like [`close_requested`], from code that runs outside the window's own update: a menu action
/// or a deferred call. Does nothing when no main window is registered.
pub fn step_aside(cx: &mut App) {
    if let Some((Away::Minimized, native, handle)) = pick(cx) {
        let _ = handle.update(cx, |_, window, _| native.minimize(window));
    }
}

/// Puts the window back on the screen and in front, from the tray, a second start of the app,
/// or the flow bar.
pub fn show(cx: &mut App) {
    let Some(main) = cx.try_global::<MainWindow>() else {
        return;
    };
    let (native, handle) = (Rc::clone(&main.native), main.handle);
    if let Err(reason) = native.show() {
        log::warn!("the window could not be shown by the platform: {reason}");
    }
    cx.global_mut::<MainWindow>().away = None;
    cx.activate(true);
    let _ = handle.update(cx, |_, window, _| window.activate_window());
}

#[cfg(test)]
pub(crate) mod testkit {
    use super::*;
    use std::cell::{Cell, RefCell};

    /// Counts what the window was asked to do.
    #[derive(Default)]
    pub struct Calls {
        pub hidden: Cell<u32>,
        pub shown: Cell<u32>,
        pub minimized: Cell<u32>,
        pub hide_fails: Cell<bool>,
        pub log: RefCell<Vec<&'static str>>,
    }

    impl Native for Calls {
        fn hide(&self) -> Result<(), String> {
            if self.hide_fails.get() {
                return Err("no such backend".into());
            }
            self.hidden.set(self.hidden.get() + 1);
            self.log.borrow_mut().push("hide");
            Ok(())
        }

        fn show(&self) -> Result<(), String> {
            self.shown.set(self.shown.get() + 1);
            self.log.borrow_mut().push("show");
            Ok(())
        }

        fn minimize(&self, _: &mut Window) {
            self.minimized.set(self.minimized.get() + 1);
            self.log.borrow_mut().push("minimize");
        }
    }

    /// Registers `handle` with a recording window and a tray host that answers `host`.
    pub fn install_fake(cx: &mut App, handle: AnyWindowHandle, host: bool) -> Rc<Calls> {
        let calls = Rc::new(Calls::default());
        install_with(cx, handle, calls.clone(), Rc::new(move || host));
        calls
    }
}

#[cfg(test)]
mod tests {
    use super::testkit::install_fake;
    use super::*;
    use gpui_kit::{
        AppContext as _, Context, IntoElement, Render, TestAppContext, WindowOptions, div,
    };

    struct Blank;

    impl Render for Blank {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    fn window(cx: &mut TestAppContext) -> AnyWindowHandle {
        cx.update(gpui_kit::init);
        cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| cx.new(|_| Blank))
                .expect("open test window")
                .0
        })
    }

    #[gpui_kit::test]
    fn close_with_a_tray_hides_the_window_and_keeps_the_app(cx: &mut TestAppContext) {
        let handle = window(cx);
        let calls = cx.update(|cx| install_fake(cx, handle, true));
        cx.update_window(handle, |_, window, cx| close_requested(window, cx))
            .unwrap();
        assert_eq!(calls.hidden.get(), 1);
        assert_eq!(calls.minimized.get(), 0);
        assert_eq!(cx.update(|cx| away(cx)), Some(Away::Hidden));
    }

    #[gpui_kit::test]
    fn close_with_no_tray_minimizes_and_never_hides(cx: &mut TestAppContext) {
        let handle = window(cx);
        let calls = cx.update(|cx| install_fake(cx, handle, false));
        cx.update_window(handle, |_, window, cx| close_requested(window, cx))
            .unwrap();
        assert_eq!(calls.hidden.get(), 0, "a withdrawn window has no way back");
        assert_eq!(calls.minimized.get(), 1);
        assert_eq!(cx.update(|cx| away(cx)), Some(Away::Minimized));
    }

    #[gpui_kit::test]
    fn a_hide_that_fails_minimizes_instead(cx: &mut TestAppContext) {
        let handle = window(cx);
        let calls = cx.update(|cx| install_fake(cx, handle, true));
        calls.hide_fails.set(true);
        cx.update_window(handle, |_, window, cx| close_requested(window, cx))
            .unwrap();
        assert_eq!(calls.minimized.get(), 1);
        assert_eq!(cx.update(|cx| away(cx)), Some(Away::Minimized));
    }

    #[gpui_kit::test]
    fn step_aside_from_outside_the_window_follows_the_same_rule(cx: &mut TestAppContext) {
        let handle = window(cx);
        let calls = cx.update(|cx| install_fake(cx, handle, true));
        cx.update(step_aside);
        assert_eq!(calls.hidden.get(), 1);
        let calls = cx.update(|cx| install_fake(cx, handle, false));
        cx.update(step_aside);
        assert_eq!((calls.hidden.get(), calls.minimized.get()), (0, 1));
    }

    #[gpui_kit::test]
    fn show_brings_the_window_back_and_clears_the_away_state(cx: &mut TestAppContext) {
        let handle = window(cx);
        let calls = cx.update(|cx| install_fake(cx, handle, true));
        cx.update(step_aside);
        cx.update(show);
        assert_eq!(calls.shown.get(), 1);
        assert_eq!(cx.update(|cx| away(cx)), None);
        assert_eq!(*calls.log.borrow(), ["hide", "show"]);
    }

    #[gpui_kit::test]
    fn with_no_main_window_registered_step_aside_and_show_do_nothing(cx: &mut TestAppContext) {
        cx.update(step_aside);
        cx.update(show);
        assert_eq!(cx.update(|cx| away(cx)), None);
    }
}
