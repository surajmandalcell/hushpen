//! Where the main window goes when it is not in front: hidden to the tray, or minimized when no
//! tray can show it again.
//!
//! The close button, the window manager's close request, and History's Re-paste call
//! [`close_requested`] or [`step_aside`]. The tray menu, a second start of the app, and the
//! flow bar's "Open history" call [`show`]. The app menu's Hide calls [`hide_app`].
//!
//! Every native call (order out, order front, miniaturize, activate, hide) runs after the App is
//! free again, never inside the update that asked for it. AppKit reports each of them through
//! callbacks that update the App, which fails when the App is still borrowed (it logs
//! `RefCell already borrowed` and loses the update). [`after_update`] is the one way in.

use gpui_kit::{AnyWindowHandle, App, AsyncApp, Global};
use hushpen_platform::window::{self, WindowHandle};
use std::rc::Rc;

/// The native side of the window. The platform crate does it; tests use a recording stand-in.
/// Only [`after_update`] work calls it, so an implementation may update the App.
pub trait Native {
    fn hide(&self) -> Result<(), String>;
    fn show(&self) -> Result<(), String>;
    fn minimize(&self, window: AnyWindowHandle, cx: &AsyncApp);
    /// Brings the whole app to the front.
    fn activate_app(&self);
    /// Hides every window of the app.
    fn hide_app(&self);
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

    fn minimize(&self, window: AnyWindowHandle, cx: &AsyncApp) {
        if matches!(self.0, WindowHandle::AppKit(_)) {
            if let Err(error) = window::minimize(self.0) {
                log::warn!("the window could not be minimized: {error}");
            }
            return;
        }
        cx.update(|cx| {
            let _ = window.update(cx, |_, window, _| window.minimize_window());
        });
    }

    fn activate_app(&self) {
        if let Err(error) = window::activate_app() {
            log::warn!("the app could not be brought to the front: {error}");
        }
    }

    fn hide_app(&self) {
        if let Err(error) = window::hide_app() {
            log::warn!("the app could not be hidden: {error}");
        }
    }
}

/// Runs `work` once the App is no longer borrowed: on the next turn of the run loop, which is
/// where GPUI itself runs its own window calls.
fn after_update(cx: &mut App, work: impl FnOnce(&AsyncApp) + 'static) {
    cx.spawn(async move |cx| work(cx)).detach();
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

/// Picks hide or minimize, hides when a tray can bring the window back, and makes the native
/// call once the App is free. A hide that fails (a backend that cannot hide) becomes a
/// minimize, so the window never vanishes with no way back. `None` when no main window is
/// registered.
fn pick(cx: &mut App) -> Option<Away> {
    let main = cx.try_global::<MainWindow>()?;
    let away = if (main.tray_host)() {
        Away::Hidden
    } else {
        Away::Minimized
    };
    let (native, handle) = (Rc::clone(&main.native), main.handle);
    cx.global_mut::<MainWindow>().away = Some(away);
    after_update(cx, move |cx| {
        if away == Away::Minimized {
            return native.minimize(handle, cx);
        }
        if let Err(reason) = native.hide() {
            log::warn!("the window could not be hidden, so it is minimized: {reason}");
            // A show that came in meanwhile has already cleared the state.
            cx.update(|cx| {
                let main = cx.global_mut::<MainWindow>();
                if main.away == Some(Away::Hidden) {
                    main.away = Some(Away::Minimized);
                }
            });
            native.minimize(handle, cx);
        }
    });
    Some(away)
}

/// The window was asked to close, by its button, a key, or the window manager. It hides to the
/// tray, or minimizes when there is no tray. With no main window registered the app quits.
pub fn close_requested(cx: &mut App) {
    if pick(cx).is_none() {
        cx.quit();
    }
}

/// Like [`close_requested`], from code that runs outside the window's own update: a menu action
/// or a deferred call. Does nothing when no main window is registered.
pub fn step_aside(cx: &mut App) {
    pick(cx);
}

/// Puts the window back on the screen and in front, from the tray, a second start of the app,
/// or the flow bar.
pub fn show(cx: &mut App) {
    let Some(main) = cx.try_global::<MainWindow>() else {
        return;
    };
    let (native, handle) = (Rc::clone(&main.native), main.handle);
    cx.global_mut::<MainWindow>().away = None;
    after_update(cx, move |cx| {
        if let Err(reason) = native.show() {
            log::warn!("the window could not be shown by the platform: {reason}");
        }
        native.activate_app();
        cx.update(|cx| {
            let _ = handle.update(cx, |_, window, _| window.activate_window());
        });
    });
}

/// Hides every window of the app, from the app menu. Does nothing when no main window is
/// registered.
pub fn hide_app(cx: &mut App) {
    let Some(main) = cx.try_global::<MainWindow>() else {
        return;
    };
    let native = Rc::clone(&main.native);
    after_update(cx, move |_| native.hide_app());
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
        pub activated: Cell<u32>,
        pub app_hidden: Cell<u32>,
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

        fn minimize(&self, _: AnyWindowHandle, _: &AsyncApp) {
            self.minimized.set(self.minimized.get() + 1);
            self.log.borrow_mut().push("minimize");
        }

        fn activate_app(&self) {
            self.activated.set(self.activated.get() + 1);
            self.log.borrow_mut().push("activate");
        }

        fn hide_app(&self) {
            self.app_hidden.set(self.app_hidden.get() + 1);
            self.log.borrow_mut().push("hide-app");
        }
    }

    /// A window whose native calls update the App, as AppKit callbacks do through GPUI. Each
    /// call panics with `already borrowed` when the App is still borrowed.
    pub struct Reentrant {
        app: AsyncApp,
        pub calls: Cell<u32>,
    }

    impl Reentrant {
        fn touch(&self) {
            self.app.update(|_| ());
            self.calls.set(self.calls.get() + 1);
        }
    }

    impl Native for Reentrant {
        fn hide(&self) -> Result<(), String> {
            self.touch();
            Ok(())
        }

        fn show(&self) -> Result<(), String> {
            self.touch();
            Ok(())
        }

        fn minimize(&self, _: AnyWindowHandle, _: &AsyncApp) {
            self.touch();
        }

        fn activate_app(&self) {
            self.touch();
        }

        fn hide_app(&self) {
            self.touch();
        }
    }

    pub fn install_reentrant(
        cx: &mut App,
        handle: AnyWindowHandle,
        host: bool,
        app: AsyncApp,
    ) -> Rc<Reentrant> {
        let native = Rc::new(Reentrant {
            app,
            calls: Cell::new(0),
        });
        install_with(cx, handle, native.clone(), Rc::new(move || host));
        native
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
    use super::testkit::{install_fake, install_reentrant};
    use super::*;
    use gpui_kit::{
        AppContext as _, Context, IntoElement, Render, TestAppContext, Window, WindowOptions, div,
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

    /// A close from inside the window's own update, which is where the button and the window
    /// manager's request arrive, followed by the run loop turn that does the native call.
    fn close(cx: &mut TestAppContext, handle: AnyWindowHandle) {
        cx.update_window(handle, |_, _, cx| close_requested(cx))
            .unwrap();
        cx.run_until_parked();
    }

    #[gpui_kit::test]
    fn close_with_a_tray_hides_the_window_and_keeps_the_app(cx: &mut TestAppContext) {
        let handle = window(cx);
        let calls = cx.update(|cx| install_fake(cx, handle, true));
        close(cx, handle);
        assert_eq!(calls.hidden.get(), 1);
        assert_eq!(calls.minimized.get(), 0);
        assert_eq!(cx.update(|cx| away(cx)), Some(Away::Hidden));
    }

    #[gpui_kit::test]
    fn close_with_no_tray_minimizes_and_never_hides(cx: &mut TestAppContext) {
        let handle = window(cx);
        let calls = cx.update(|cx| install_fake(cx, handle, false));
        close(cx, handle);
        assert_eq!(calls.hidden.get(), 0, "a withdrawn window has no way back");
        assert_eq!(calls.minimized.get(), 1);
        assert_eq!(cx.update(|cx| away(cx)), Some(Away::Minimized));
    }

    #[gpui_kit::test]
    fn a_hide_that_fails_minimizes_instead(cx: &mut TestAppContext) {
        let handle = window(cx);
        let calls = cx.update(|cx| install_fake(cx, handle, true));
        calls.hide_fails.set(true);
        close(cx, handle);
        assert_eq!(calls.minimized.get(), 1);
        assert_eq!(cx.update(|cx| away(cx)), Some(Away::Minimized));
    }

    #[gpui_kit::test]
    fn step_aside_from_outside_the_window_follows_the_same_rule(cx: &mut TestAppContext) {
        let handle = window(cx);
        let calls = cx.update(|cx| install_fake(cx, handle, true));
        cx.update(step_aside);
        cx.run_until_parked();
        assert_eq!(calls.hidden.get(), 1);
        let calls = cx.update(|cx| install_fake(cx, handle, false));
        cx.update(step_aside);
        cx.run_until_parked();
        assert_eq!((calls.hidden.get(), calls.minimized.get()), (0, 1));
    }

    #[gpui_kit::test]
    fn show_brings_the_window_back_and_clears_the_away_state(cx: &mut TestAppContext) {
        let handle = window(cx);
        let calls = cx.update(|cx| install_fake(cx, handle, true));
        cx.update(step_aside);
        cx.update(show);
        assert_eq!(cx.update(|cx| away(cx)), None);
        cx.run_until_parked();
        assert_eq!(calls.shown.get(), 1);
        assert_eq!(calls.activated.get(), 1);
        assert_eq!(*calls.log.borrow(), ["hide", "show", "activate"]);
    }

    #[gpui_kit::test]
    fn a_show_right_after_a_failed_hide_keeps_the_window_shown(cx: &mut TestAppContext) {
        let handle = window(cx);
        let calls = cx.update(|cx| install_fake(cx, handle, true));
        calls.hide_fails.set(true);
        cx.update(step_aside);
        cx.update(show);
        cx.run_until_parked();
        assert_eq!(cx.update(|cx| away(cx)), None);
        assert_eq!(*calls.log.borrow(), ["minimize", "show", "activate"]);
    }

    #[gpui_kit::test]
    fn hide_app_hides_the_app_after_the_app_is_free(cx: &mut TestAppContext) {
        let handle = window(cx);
        let calls = cx.update(|cx| install_fake(cx, handle, true));
        cx.update(hide_app);
        assert_eq!(calls.app_hidden.get(), 0, "not inside the update");
        cx.run_until_parked();
        assert_eq!(calls.app_hidden.get(), 1);
        let native = cx.update(|cx| install_reentrant(cx, handle, true, cx.to_async()));
        cx.update(hide_app);
        cx.run_until_parked();
        assert_eq!(native.calls.get(), 1);
    }

    #[gpui_kit::test]
    fn show_makes_its_native_calls_after_the_app_is_free(cx: &mut TestAppContext) {
        let handle = window(cx);
        let native = cx.update(|cx| install_reentrant(cx, handle, true, cx.to_async()));
        cx.update(show);
        cx.run_until_parked();
        assert_eq!(native.calls.get(), 2, "the order front and the activation");
        assert_eq!(cx.update(|cx| away(cx)), None);
    }

    #[gpui_kit::test]
    fn hiding_makes_its_native_calls_after_the_app_is_free(cx: &mut TestAppContext) {
        let handle = window(cx);
        let native = cx.update(|cx| install_reentrant(cx, handle, true, cx.to_async()));
        close(cx, handle);
        cx.run_until_parked();
        assert_eq!(native.calls.get(), 1);
        assert_eq!(cx.update(|cx| away(cx)), Some(Away::Hidden));
    }

    #[gpui_kit::test]
    fn minimizing_makes_its_native_calls_after_the_app_is_free(cx: &mut TestAppContext) {
        let handle = window(cx);
        let native = cx.update(|cx| install_reentrant(cx, handle, false, cx.to_async()));
        close(cx, handle);
        cx.run_until_parked();
        assert_eq!(native.calls.get(), 1);
        assert_eq!(cx.update(|cx| away(cx)), Some(Away::Minimized));
    }

    #[gpui_kit::test]
    fn with_no_main_window_registered_step_aside_and_show_do_nothing(cx: &mut TestAppContext) {
        cx.update(step_aside);
        cx.update(show);
        assert_eq!(cx.update(|cx| away(cx)), None);
    }
}
