//! Process start: single instance, the fixed window, and shutdown.

use crate::assets::{AppAssets, register_fonts};
use crate::instance::{self, Start};
use crate::shell::Shell;
use crate::theme::{self, space};
use futures::StreamExt;
use futures::channel::mpsc;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Window, WindowBounds, WindowOptions, px, size,
};
use hushpen_platform::window::{WindowHandle, lock_chrome};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::process::ExitCode;

const APP_ID: &str = "hushpen";

pub fn run() -> ExitCode {
    let data_dir = crate::paths::data_dir();
    let instance = match instance::start(&data_dir) {
        Ok(Start::First(instance)) => instance,
        Ok(Start::AlreadyRunning) => return ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("hushpen: could not start: {error}");
            return ExitCode::FAILURE;
        }
    };
    let (show_requests, shown) = mpsc::unbounded::<()>();
    let _guard = instance.serve(move || {
        let _ = show_requests.unbounded_send(());
    });

    gpui_kit::application()
        .with_assets(AppAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            register_fonts(cx);
            theme::install(cx);
            // Close quits for now; it will hide to the tray once the tray exists.
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let handle = match open_main_window(cx) {
                Ok(handle) => handle,
                Err(error) => {
                    eprintln!("hushpen: could not open the window: {error:#}");
                    cx.quit();
                    return;
                }
            };
            cx.spawn(async move |cx| {
                let mut shown = shown;
                while shown.next().await.is_some() {
                    cx.update(|cx| {
                        let _ = handle.update(cx, |_, window, _| window.activate_window());
                    });
                }
            })
            .detach();
        });
    ExitCode::SUCCESS
}

fn open_main_window(cx: &mut App) -> gpui_kit::Result<AnyWindowHandle> {
    let window_size = size(px(space::WINDOW_WIDTH), px(space::WINDOW_HEIGHT));
    let (handle, _shell) = gpui_kit::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                window_size,
                cx,
            ))),
            titlebar: None,
            is_resizable: false,
            is_minimizable: true,
            window_min_size: Some(window_size),
            app_id: Some(APP_ID.into()),
            ..Default::default()
        },
        cx,
        |window, cx| cx.new(|cx| Shell::new(window, cx)),
    )?;
    let _ = handle.update(cx, |_, window, _| lock_native_chrome(window));
    Ok(handle)
}

fn lock_native_chrome(window: &Window) {
    let native = match HasWindowHandle::window_handle(window).map(|handle| handle.as_raw()) {
        Ok(RawWindowHandle::Xcb(handle)) => WindowHandle::X11(handle.window.get()),
        Ok(RawWindowHandle::Xlib(handle)) => match u32::try_from(handle.window) {
            Ok(id) => WindowHandle::X11(id),
            Err(_) => WindowHandle::Other,
        },
        Ok(RawWindowHandle::AppKit(handle)) => {
            WindowHandle::AppKit(handle.ns_view.as_ptr() as usize)
        }
        _ => WindowHandle::Other,
    };
    if let Err(error) = lock_chrome(
        native,
        space::WINDOW_WIDTH as u16,
        space::WINDOW_HEIGHT as u16,
    ) {
        log::warn!("could not lock the window chrome: {error}");
    }
}
