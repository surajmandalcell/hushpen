//! Process start: single instance, the fixed window, and shutdown.

use crate::assets::{AppAssets, register_fonts};
use crate::controller::{Controller, Engine, InsertSupport, KeysStatus, monotonic_clock};
use crate::dictation::Dictation;
use crate::engine_host::EngineHost;
use crate::instance::{self, Start};
use crate::mic::{self, CpalBackend, Mic};
use crate::models::{self, Models};
use crate::shell::Shell;
use crate::storage;
use crate::theme::{self, space};
use futures::StreamExt;
use futures::channel::mpsc;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Entity, Window, WindowBounds, WindowOptions, px,
    size,
};
use hushpen_platform::keys::{GlobalKeys, HoldKey};
use hushpen_platform::window::{WindowHandle, lock_chrome};
use hushpen_store::data_dir::DataDir;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::Arc;

const APP_ID: &str = "hushpen";

pub fn run() -> ExitCode {
    let data = match DataDir::open(hushpen_store::data_dir::resolve_from_env()) {
        Ok(data) => data,
        Err(error) => {
            eprintln!("hushpen: could not open the data folder: {error}");
            return ExitCode::FAILURE;
        }
    };
    let instance = match instance::start(data.root()) {
        Ok(Start::First(instance)) => instance,
        Ok(Start::AlreadyRunning) => return ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("hushpen: could not start: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = storage::install_logger(&data) {
        eprintln!("hushpen: could not start the log: {error}");
    }
    if let Some(exit) = no_display_exit() {
        return exit;
    }
    #[cfg(feature = "test-automation")]
    let (_hook_server, hook_jobs) = match crate::hook::start(&data) {
        Some((server, jobs)) => (Some(server), Some(jobs)),
        None => (None, None),
    };
    let storage = match storage::open(data) {
        Ok(storage) => Rc::new(storage),
        Err(error) => {
            eprintln!("hushpen: could not open the data folder: {error}");
            return ExitCode::FAILURE;
        }
    };
    let default_model = hushpen_core::catalog::embedded()
        .default_whisper()
        .map(|entry| entry.id.clone())
        .unwrap_or_default();
    let engine = Rc::new(EngineHost::start(&storage.data, &default_model));
    let recovered = mic::sweep_orphans(&storage.data);
    let mic_storage = Rc::clone(&storage);
    let models_storage = Rc::clone(&storage);
    let models_engine = Rc::clone(&engine);
    let dictation_storage = Rc::clone(&storage);
    let dictation_engine = Rc::clone(&engine);
    #[cfg(feature = "test-automation")]
    let hook_storage = Rc::clone(&storage);
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
            // The logger is a process-wide static, so `Drop` never runs for it.
            let quit_engine = Rc::clone(&engine);
            cx.on_app_quit(move |_| {
                quit_engine.shutdown();
                log::logger().flush();
                async {}
            })
            .detach();
            // Close quits for now; it will hide to the tray once the tray exists.
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let (handle, shell) = match open_main_window(cx) {
                Ok(opened) => opened,
                Err(error) => {
                    eprintln!("hushpen: could not open the window: {error:#}");
                    cx.quit();
                    return;
                }
            };
            let mic = cx.new(|cx| {
                let mut mic = Mic::new(mic_storage, Arc::new(CpalBackend), cx);
                mic.set_recovered(recovered);
                mic.refresh(cx);
                mic
            });
            shell.update(cx, |shell, cx| shell.attach_mic(mic.clone(), cx));
            let models = cx.new(|cx| {
                Models::new(
                    models_storage,
                    hushpen_core::catalog::embedded(),
                    models::production_transfer(),
                    models::thread_spawner(),
                    Some(models_engine),
                    cx,
                )
            });
            shell.update(cx, |shell, cx| shell.attach_models(models.clone(), cx));
            let controller = cx.new(|cx| {
                Controller::new(
                    Rc::clone(&dictation_storage),
                    mic.clone(),
                    models.clone(),
                    Engine::host(&dictation_engine),
                    models::thread_spawner(),
                    monotonic_clock(),
                    cx,
                )
            });
            start_keys(&controller, cx);
            start_insert(&controller, cx);
            controller.update(cx, |controller, _| {
                controller.attach_permissions(hushpen_platform::permissions::system());
            });
            let dictation =
                cx.new(|cx| Dictation::new(dictation_storage, models.clone(), controller, cx));
            shell.update(cx, |shell, cx| {
                shell.attach_dictation(dictation.clone(), cx)
            });
            #[cfg(feature = "test-automation")]
            if let Some(jobs) = hook_jobs {
                let settings_storage = Rc::clone(&hook_storage);
                crate::hook::attach(
                    cx,
                    jobs,
                    crate::hook::Surface {
                        window: handle,
                        shell,
                        settings: Rc::new(move || settings_storage.settings.values()),
                    },
                );
                crate::hook::attach_engine(cx, Rc::clone(&engine), hook_storage);
                crate::hook::attach_mic(cx, mic);
                crate::hook::attach_models(cx, models);
                crate::hook::attach_dictation(cx, dictation);
            }
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

/// The window toolkit panics when it finds no display server, so a session with none ends
/// here, with the reason in the log and a clean exit.
#[cfg(target_os = "linux")]
fn no_display_exit() -> Option<ExitCode> {
    let session = hushpen_platform::keys::session::display_problem()?;
    let reason = match session {
        hushpen_platform::keys::session::Session::Wayland => "wayland",
        _ => "no-display",
    };
    log::warn!("global keys not available: {reason}; no display server can be reached");
    eprintln!("hushpen: no display server can be reached (global keys not available: {reason})");
    log::logger().flush();
    Some(ExitCode::SUCCESS)
}

#[cfg(not(target_os = "linux"))]
fn no_display_exit() -> Option<ExitCode> {
    None
}

/// Starts the hold key and Esc listener and hands the controller what it needs to run them.
/// A failure is not fatal: the buttons still work, and Home says why the keys are off.
fn start_keys(controller: &Entity<Controller>, cx: &mut App) {
    let events = controller.read(cx).sender();
    let sink: hushpen_platform::keys::Sink = Arc::new(move |event| events.send(event));
    let (status, session_active): (KeysStatus, Rc<dyn Fn(bool)>) =
        match GlobalKeys::start(HoldKey::default(), sink) {
            Ok(keys) => (
                KeysStatus::Available,
                Rc::new(move |active| keys.set_session_active(active)),
            ),
            Err(why) => {
                log::warn!("global keys not available: {}", why.reason.key());
                (KeysStatus::Unavailable(why), Rc::new(|_| {}))
            }
        };
    controller.update(cx, |controller, _| {
        controller.attach_keys(status, session_active)
    });
}

/// Sets up paste. When the system cannot paste, finished text is copied and Home says why.
fn start_insert(controller: &Entity<Controller>, cx: &mut App) {
    let support = match hushpen_platform::insert::system() {
        Ok(inserter) => InsertSupport::Ready(inserter),
        Err(why) => {
            log::warn!("paste not available: {}", why.reason.key());
            InsertSupport::Unavailable(why)
        }
    };
    controller.update(cx, |controller, _| controller.attach_insert(support));
}

fn open_main_window(cx: &mut App) -> gpui_kit::Result<(AnyWindowHandle, Entity<Shell>)> {
    let window_size = size(px(space::WINDOW_WIDTH), px(space::WINDOW_HEIGHT));
    let (handle, shell) = gpui_kit::open_window(
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
    Ok((handle, shell))
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
