use tauri::{AppHandle, Emitter, Manager};
use tauri::menu::{Menu, MenuItemBuilder, PredefinedMenuItem};
use tauri::tray::{MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri_plugin_global_shortcut::{Shortcut, ShortcutEvent, ShortcutState};
use serde_json;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn show_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn hide_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn quit_app(app: AppHandle) -> Result<(), String> {
    println!("quit_app command received");
    app.exit(0);
    Ok(())
}

#[tauri::command]
async fn show_tray_notification(
    app: AppHandle,
    title: String,
    message: String,
    notification_type: String,
) -> Result<(), String> {
    app.emit("show-notification", serde_json::json!({
        "title": title,
        "message": message,
        "type": notification_type
    })).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn start_recording(app: AppHandle) -> Result<(), String> {
    app.emit("recording-start", {}).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn stop_recording(app: AppHandle) -> Result<(), String> {
    app.emit("recording-stop", {}).map_err(|e| e.to_string())?;
    Ok(())
}

fn handle_global_shortcut(app: &AppHandle, _shortcut: &Shortcut, event: ShortcutEvent) {
    match event.state {
        ShortcutState::Pressed => {
            if let Some(window) = app.get_webview_window("main") {
                if window.is_visible().unwrap_or(false) {
                    let _ = app.emit("toggle-recording", {});
                } else {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let app = app.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        let _ = app.emit("recording-start", {});
                    });
                }
            }
        }
        ShortcutState::Released => {}
    }
}

fn create_tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let show = MenuItemBuilder::with_id("show", "Show ASR Pro").build(app)?;
    let hide = MenuItemBuilder::with_id("hide", "Hide").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = Menu::new(app)?;
    menu.append(&show)?;
    menu.append(&hide)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&quit)?;
    Ok(menu)
}

fn handle_tray_event(tray: &TrayIcon, event: TrayIconEvent) {
    let app = tray.app_handle();
    match event {
        TrayIconEvent::Click {
            button: MouseButton::Left,
            ..
        } => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        TrayIconEvent::Enter { .. } => {}
        TrayIconEvent::Leave { .. } => {}
        TrayIconEvent::DoubleClick { .. } => {}
        _ => {}
    }
}

fn handle_tray_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    match event.id.as_ref() {
        "show" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        "hide" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
        }
        "quit" => {
            app.exit(0);
        }
        _ => {}
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    println!("Starting ASR Pro application...");
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcuts(["CommandOrControl+Shift+Space"])
                .unwrap()
                .with_handler(handle_global_shortcut)
                .build(),
        )
        .setup(|app| {
            println!("Setting up tray menu...");
            let tray_menu = create_tray_menu(app.handle())?;
            let _tray_icon = TrayIconBuilder::new()
                .menu(&tray_menu)
                .on_menu_event(handle_tray_menu_event)
                .on_tray_icon_event(handle_tray_event)
                .build(app)?;
            println!("Application setup complete");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            show_window,
            hide_window,
            quit_app,
            show_tray_notification,
            start_recording,
            stop_recording
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}