use tauri::{
    api, AppHandle, CustomMenuItem, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu,
    SystemTrayMenuItem,
};
use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn show_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_window("main") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn hide_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn quit_app(app: AppHandle) -> Result<(), String> {
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
    // Create a notification using Tauri's notification API
    let notification =
        tauri::api::notification::Notification::new(&app.config().tauri.bundle.identifier)
            .title(title)
            .body(message)
            .icon("icon");

    // Set different icons based on notification type
    let icon_path = match notification_type.as_str() {
        "error" => "icons/error.png",
        "warning" => "icons/warning.png",
        "success" => "icons/success.png",
        _ => "icons/icon.png",
    };

    let notification_with_icon = notification.icon(icon_path);

    // Show the notification
    notification_with_icon.show().map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn start_recording(app: AppHandle) -> Result<(), String> {
    // Send a message to the frontend to start recording
    app.emit("recording-start", {}).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn stop_recording(app: AppHandle) -> Result<(), String> {
    // Send a message to the frontend to stop recording
    app.emit("recording-stop", {}).map_err(|e| e.to_string())?;
    Ok(())
}

fn handle_global_shortcut(app: &AppHandle, shortcut: ShortcutState) {
    match shortcut {
        ShortcutState::Pressed => {
            // Check if the main window is visible and focused
            if let Some(window) = app.get_window("main") {
                if window.is_visible().unwrap_or(false) {
                    // If window is visible, toggle recording
                    let _ = app.emit("toggle-recording", {});
                } else {
                    // If window is hidden, show it first, then start recording
                    let _ = window.show();
                    let _ = window.set_focus();
                    // Start recording after a short delay to allow window to appear
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

fn create_tray_menu() -> SystemTrayMenu {
    let show = CustomMenuItem::new("show".to_string(), "Show ASR Pro");
    let hide = CustomMenuItem::new("hide".to_string(), "Hide");
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");

    SystemTrayMenu::new()
        .add_item(show)
        .add_item(hide)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(quit)
}

fn handle_tray_event(app: &AppHandle, event: SystemTrayEvent) {
    match event {
        SystemTrayEvent::LeftClick {
            position: _,
            size: _,
            ..
        } => {
            if let Some(window) = app.get_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
            "show" => {
                if let Some(window) = app.get_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "hide" => {
                if let Some(window) = app.get_window("main") {
                    let _ = window.hide();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        },
        _ => {}
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let tray_menu = create_tray_menu();

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
        .system_tray(SystemTray::new().with_menu(tray_menu))
        .on_system_tray_event(handle_tray_event)
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
