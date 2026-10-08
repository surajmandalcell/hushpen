//! Process start: single instance, the fixed window, and shutdown.

use crate::app_menu;
use crate::assets::{AppAssets, register_fonts};
use crate::controller::{Controller, Engine, InsertSupport, KeysStatus, monotonic_clock};
use crate::dictation::Dictation;
use crate::dictionary::Dictionary;
use crate::engine_host::EngineHost;
use crate::flow_bar::{self, FlowBar, OpenHistory, PopUp};
use crate::history::History;
use crate::instance::{self, Start};
use crate::main_window;
use crate::mic::{self, CpalBackend, Mic};
use crate::models::{self, Models};
use crate::native::native_handle;
use crate::onboarding::{Onboarding, Parts as OnboardingParts, SystemGuide};
use crate::shell::Shell;
use crate::shortcuts::Shortcuts;
use crate::storage;
use crate::theme::{self, space};
use crate::tray::{self, Surfaces};
use crate::views::View;
use futures::StreamExt;
use futures::channel::mpsc;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Entity, Window, WindowBounds, WindowOptions, px,
    size,
};
use hushpen_core::shortcut::Platform;
use hushpen_platform::keys::GlobalKeys;
use hushpen_platform::window::lock_chrome;
use hushpen_store::data_dir::DataDir;
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
    let dictionary_storage = Rc::clone(&storage);
    let history_storage = Rc::clone(&storage);
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
            let (handle, shell) = match open_main_window(cx) {
                Ok(opened) => opened,
                Err(error) => {
                    eprintln!("hushpen: could not open the window: {error:#}");
                    cx.quit();
                    return;
                }
            };
            // Close hides the window to the tray, or minimizes it when no tray can bring it
            // back; only the tray's Quit ends the app. If the window is ever closed for real,
            // the app ends with it. The flow bar opens and closes its own window, which must
            // not end the app.
            cx.on_window_closed(move |cx, _| {
                if !cx.windows().contains(&handle) {
                    cx.quit();
                }
            })
            .detach();
            let tray_host = Rc::new(hushpen_platform::tray_host::available);
            if !tray_host() {
                log::warn!(
                    "no tray host: no StatusNotifier watcher owns the session bus name, so closing the window minimizes it"
                );
            }
            let _ = handle.update(cx, |_, window, cx| {
                main_window::install(cx, handle, native_handle(window), tray_host);
                window.on_window_should_close(cx, |window, cx| {
                    main_window::close_requested(window, cx);
                    false
                });
            });
            app_menu::install(cx);
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
            let dictionary = handle
                .update(cx, |_, window, cx| {
                    cx.new(|cx| Dictionary::new(dictionary_storage, window, cx))
                })
                .ok();
            if let Some(dictionary) = &dictionary {
                shell.update(cx, |shell, cx| {
                    shell.attach_dictionary(dictionary.clone(), cx)
                });
            }
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
            let shortcuts =
                cx.new(|cx| Shortcuts::new(Rc::clone(&dictation_storage), Platform::current(), cx));
            shell.update(cx, |shell, cx| {
                shell.attach_shortcuts(shortcuts.clone(), cx)
            });
            start_keys(&controller, &shortcuts, cx);
            start_insert(&controller, cx);
            start_cues(&controller, cx);
            controller.update(cx, |controller, _| {
                controller.attach_permissions(hushpen_platform::permissions::system());
            });
            let onboarding = cx.new(|cx| {
                Onboarding::new(
                    OnboardingParts {
                        storage: Rc::clone(&dictation_storage),
                        mic: mic.clone(),
                        models: models.clone(),
                        controller: controller.clone(),
                        guide: Rc::new(SystemGuide),
                        platform: Platform::current(),
                        session: display_session(),
                    },
                    cx,
                )
            });
            onboarding.update(cx, |onboarding, cx| {
                onboarding.attach_shortcuts(shortcuts.clone(), cx)
            });
            shell.update(cx, |shell, cx| {
                shell.attach_onboarding(onboarding.clone(), cx)
            });
            let history = handle
                .update(cx, |_, window, cx| {
                    cx.new(|cx| History::new(history_storage, controller.clone(), window, cx))
                })
                .ok();
            if let Some(history) = &history {
                shell.update(cx, |shell, cx| shell.attach_history(history.clone(), cx));
                let quitting = history.clone();
                cx.on_app_quit(move |cx| {
                    quitting.update(cx, |history, _| history.finish_undo());
                    async {}
                })
                .detach();
            }
            let dictation = cx.new(|cx| {
                Dictation::new(
                    Rc::clone(&dictation_storage),
                    models.clone(),
                    controller.clone(),
                    cx,
                )
            });
            shell.update(cx, |shell, cx| {
                shell.attach_dictation(dictation.clone(), cx)
            });
            let tray_controller = controller.clone();
            let flow_bar = cx.new(|_| {
                FlowBar::new(
                    dictation_storage,
                    controller.clone(),
                    dictation.clone(),
                    mic.clone(),
                )
            });
            flow_bar.update(cx, |bar, _| {
                bar.on_open_history(open_history(shell.clone(), history.clone()))
            });
            tray::register_actions(
                cx,
                &Surfaces {
                    controller,
                    flow_bar: flow_bar.clone(),
                    shell: shell.clone(),
                },
            );
            let tray = tray::install(cx, &tray_controller);
            if let Some(tray) = &tray {
                tray::follow_appearance(cx, handle, tray);
                let closing = tray.clone();
                cx.on_app_quit(move |cx| {
                    // The icon leaves the panel now, not when the process is reaped.
                    let _ = closing.close(cx);
                    async {}
                })
                .detach();
            }
            #[cfg_attr(not(feature = "test-automation"), allow(unused_variables))]
            let flow_bar_host =
                flow_bar::run(cx, flow_bar.clone(), Box::new(PopUp::new(&flow_bar)));
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
                crate::hook::attach_settings(cx, Rc::clone(&hook_storage));
                crate::hook::attach_engine(cx, Rc::clone(&engine), hook_storage);
                crate::hook::attach_flow_bar(cx, flow_bar, flow_bar_host);
                crate::hook::attach_tray(cx, tray_controller, tray.is_some());
                crate::hook::attach_mic(cx, mic);
                crate::hook::attach_shortcuts(cx, shortcuts);
                crate::hook::attach_models(cx, models);
                crate::hook::attach_onboarding(cx, onboarding);
                crate::hook::attach_dictation(cx, dictation);
                if let Some(dictionary) = dictionary {
                    crate::hook::attach_dictionary(cx, dictionary);
                }
                if let Some(history) = history {
                    crate::hook::attach_history(cx, history, handle);
                }
            }
            cx.spawn(async move |cx| {
                let mut shown = shown;
                while shown.next().await.is_some() {
                    cx.update(main_window::show);
                }
            })
            .detach();
        });
    ExitCode::SUCCESS
}

/// What the flow bar's "Open history" button does: show the main window on the History view
/// with the row open.
fn open_history(shell: Entity<Shell>, history: Option<Entity<History>>) -> OpenHistory {
    Rc::new(move |id, cx| {
        shell.update(cx, |shell, cx| shell.select(View::History, cx));
        if let Some(history) = &history {
            history.update(cx, |history, cx| {
                if let Err(reason) = history.open(id, cx) {
                    log::warn!("the flow bar could not open that row: {reason}");
                }
            });
        }
        main_window::show(cx);
    })
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

/// The display session for the onboarding permissions page. macOS has no such choice.
#[cfg(target_os = "linux")]
fn display_session() -> hushpen_core::onboarding::Session {
    use hushpen_core::onboarding::Session;
    use hushpen_platform::keys::session::{Session as Found, detect};
    match detect() {
        Found::X11 => Session::X11,
        Found::Wayland => Session::Wayland,
        Found::NoDisplay => Session::NoDisplay,
    }
}

#[cfg(not(target_os = "linux"))]
fn display_session() -> hushpen_core::onboarding::Session {
    hushpen_core::onboarding::Session::X11
}

/// Starts the key listener with the saved shortcuts and hands the controller and the Shortcuts
/// section what they need to run it. A failure is not fatal: the buttons still work, and Home
/// says why the keys are off.
fn start_keys(controller: &Entity<Controller>, shortcuts: &Entity<Shortcuts>, cx: &mut App) {
    let events = controller.read(cx).sender();
    let sink: hushpen_platform::keys::Sink = Arc::new(move |event| events.send(event));
    let (bindings, record) = {
        let shortcuts = shortcuts.read(cx);
        (shortcuts.bindings(), shortcuts.record_sink())
    };
    match GlobalKeys::start(bindings, sink, record) {
        Ok(keys) => {
            let keys = Rc::new(keys);
            let session = Rc::clone(&keys);
            controller.update(cx, |controller, _| {
                controller.attach_keys(
                    KeysStatus::Available,
                    Rc::new(move |active| session.set_session_active(active)),
                )
            });
            shortcuts.update(cx, |shortcuts, cx| shortcuts.attach_keys(Ok(keys), cx));
        }
        Err(why) => {
            log::warn!("global keys not available: {}", why.reason.key());
            let message = why.message.clone();
            controller.update(cx, |controller, _| {
                controller.attach_keys(KeysStatus::Unavailable(why), Rc::new(|_| {}))
            });
            shortcuts.update(cx, |shortcuts, cx| shortcuts.attach_keys(Err(message), cx));
        }
    }
}

/// Gives the controller the sound player. The player opens the output on its own thread.
fn start_cues(controller: &Entity<Controller>, cx: &mut App) {
    let player = Arc::new(hushpen_audio::CuePlayer::new());
    player.warm();
    controller.update(cx, |controller, _| {
        controller.attach_cues(Arc::new(move |cue, volume| player.play(cue, volume)))
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
    if let Err(error) = lock_chrome(
        native_handle(window),
        space::WINDOW_WIDTH as u16,
        space::WINDOW_HEIGHT as u16,
    ) {
        log::warn!("could not lock the window chrome: {error}");
    }
}
