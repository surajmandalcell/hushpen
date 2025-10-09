mod models;
mod utils;
mod ui;
mod services;

use glib::clone;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow};
use gtk4::glib::Propagation;
use std::sync::Arc;

// Import our modules
use models::AppState;

// Application constants
const APP_ID: &str = "com.asrpro.gtk4";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const APP_NAME: &str = "ASRPro";

fn main() {
    // Initialize GTK
    gtk4::init().expect("Failed to initialize GTK");
    
    // Create the application
    let app = Application::builder()
        .application_id(APP_ID)
        .build();
    
    // Connect to activate signal
    app.connect_activate(build_ui);
    
    // Connect to startup signal for initialization
    app.connect_startup(|app| {
        // Initialize the application
        initialize_application(app);
    });
    
    // Run the application
    app.run();
}

/// Initialize the application with proper setup
fn initialize_application(app: &Application) {
    // Create the async runtime
    let runtime = tokio::runtime::Runtime::new()
        .expect("Failed to create async runtime");
    
    // Create the application state
    let app_state = Arc::new(AppState::new());
    
    // Initialize the application state
    let app_state_clone = app_state.clone();
    runtime.block_on(async move {
        if let Err(e) = app_state_clone.initialize().await {
            eprintln!("Failed to initialize application state: {}", e);
            // Show error dialog
            glib::idle_add_once(move || {
                show_error_dialog(None, "Initialization Error", &format!("Failed to initialize application: {}", e));
            });
        }
    });
    
    // Store the app state and runtime in the application
    unsafe {
        app.set_data("app_state", app_state);
        app.set_data("runtime", runtime);
    }
}

/// Build the main UI
fn build_ui(app: &Application) {
    // Get the app state from the application
    let app_state: Arc<AppState> = unsafe {
        app.data::<Arc<AppState>>("app_state")
            .expect("App state not initialized")
            .as_ref()
            .clone()
    };
    
    // Get the runtime from the application
    let runtime: &'static tokio::runtime::Runtime = unsafe {
        app.data::<tokio::runtime::Runtime>("runtime")
            .expect("Runtime not initialized")
            .as_ref()
    };
    
    // Create the main window
    let window = ApplicationWindow::builder()
        .application(app)
        .title(format!("{} - GTK4 Frontend", APP_NAME))
        .default_width(1200)
        .default_height(800)
        .build();
    
    // Set up window close handler
    window.connect_close_request(clone!(@strong app => move |window| {
        // Handle application shutdown
        handle_shutdown(&app);
        Propagation::Proceed
    }));
    
    // Create the main UI
    let main_ui = ui::MainUI::new(window.clone(), app_state.clone());
    
    // Initialize the UI
    let window_clone = window.clone();
    let error_msg = runtime.block_on(async move {
        if let Err(e) = main_ui.initialize().await {
            Some(format!("Failed to initialize UI: {}", e))
        } else {
            None
        }
    });
    
    if let Some(msg) = error_msg {
        show_error_dialog(Some(&window_clone), "UI Error", &msg);
    }
    
    // Show the window
    window.present();
    
    // Show welcome message
    let app_state_clone = app_state.clone();
    runtime.spawn(async move {
        app_state_clone.set_status_message("Application started successfully".to_string()).await;
        app_state_clone.show_notification(
            "Welcome to ASRPro".to_string(),
            "The application is ready to use".to_string(),
        ).await;
    });
}

/// Handle application shutdown
fn handle_shutdown(app: &Application) {
    // Get the app state from the application
    let app_state: Option<Arc<AppState>> = unsafe {
        app.data::<Arc<AppState>>("app_state")
            .map(|ptr| ptr.as_ref().clone())
    };
    
    if let Some(app_state) = app_state {
        // Get the runtime from the application
        let runtime: Option<&'static tokio::runtime::Runtime> = unsafe {
            app.data::<tokio::runtime::Runtime>("runtime")
                .map(|ptr| ptr.as_ref())
        };
        
        if let Some(runtime) = runtime {
            // Save configuration
            let app_state_clone = app_state.clone();
            runtime.block_on(async move {
                if let Err(e) = app_state_clone.save_config().await {
                    eprintln!("Failed to save configuration: {}", e);
                }
            });
        }
    }
}

/// Show an error dialog
fn show_error_dialog(parent: Option<&ApplicationWindow>, title: &str, message: &str) {
    let dialog = gtk4::MessageDialog::builder()
        .title(title)
        .text(message)
        .buttons(gtk4::ButtonsType::Ok)
        .message_type(gtk4::MessageType::Error)
        .modal(true)
        .build();
    
    if let Some(parent) = parent {
        dialog.set_transient_for(Some(parent));
    }
    
    dialog.connect_response(|dialog, _| {
        dialog.close();
    });
    
    dialog.show();
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_app_constants() {
        assert_eq!(APP_ID, "com.asrpro.gtk4");
        assert_eq!(APP_NAME, "ASRPro");
        assert!(!APP_VERSION.is_empty());
    }
}