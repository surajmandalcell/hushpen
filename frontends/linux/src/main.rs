mod models;
mod utils;
mod ui;
mod services;

use glib::clone;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow};
use gtk4::glib::Propagation;
use std::sync::Arc;
use std::env;

// Import our modules
use models::AppState;
use services::{BackendClient, ModelManager};
use models::api::BackendConfig;

// Application constants
const APP_ID: &str = "com.asrpro.gtk4";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const APP_NAME: &str = "ASRPro";

// Command line arguments
#[derive(Debug, Default)]
struct AppArgs {
    files: Vec<String>,
    new_transcription: bool,
    file_selector: bool,
    help: bool,
    version: bool,
}

fn parse_args() -> AppArgs {
    let args: Vec<String> = env::args().collect();
    let mut app_args = AppArgs::default();
    
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                app_args.help = true;
            }
            "--version" | "-v" => {
                app_args.version = true;
            }
            "--new" | "-n" => {
                app_args.new_transcription = true;
            }
            "--file-selector" | "-f" => {
                app_args.file_selector = true;
            }
            arg => {
                if arg.starts_with("--") {
                    eprintln!("Unknown option: {}", arg);
                } else if arg.starts_with("asrpro://") {
                    // Handle protocol handler
                    app_args.files.push(arg.clone());
                } else {
                    // Assume it's a file path
                    app_args.files.push(arg.clone());
                }
            }
        }
        i += 1;
    }
    
    app_args
}

fn print_help() {
    println!("{} - GTK4 Frontend v{}", APP_NAME, APP_VERSION);
    println!("Automatic Speech Recognition Application");
    println!();
    println!("Usage: {} [OPTIONS] [FILES...]", env::args().next().unwrap_or_else(|| "asrpro-gtk4".to_string()));
    println!();
    println!("Options:");
    println!("  -h, --help              Show this help message");
    println!("  -v, --version           Show version information");
    println!("  -n, --new               Start with a new transcription");
    println!("  -f, --file-selector     Open file selector on startup");
    println!();
    println!("Files:");
    println!("  FILES                   Audio files to open on startup");
    println!("  asrpro://URL            ASRPro protocol URLs");
    println!();
    println!("Supported formats:");
    println!("  MP3, WAV, FLAC, OGG, M4A, AAC");
}

fn print_version() {
    println!("{} v{}", APP_NAME, APP_VERSION);
    println!("GTK4 Frontend for ASRPro");
}

fn main() {
    // Parse command line arguments
    let args = parse_args();
    
    // Handle help and version flags
    if args.help {
        print_help();
        return;
    }
    
    if args.version {
        print_version();
        return;
    }
    
    // Initialize GTK
    gtk4::init().expect("Failed to initialize GTK");
    
    // Create the application
    let app = Application::builder()
        .application_id(APP_ID)
        .flags(gtk4::ApplicationFlags::HANDLES_OPEN)
        .build();
    
    // Store command line arguments in application data
    unsafe {
        app.set_data("app_args", args);
    }
    
    // Connect to activate signal
    app.connect_activate(build_ui);
    
    // Connect to open signal for file handling
    app.connect_open(|app, files, _| {
        let args: AppArgs = unsafe {
            app.data::<AppArgs>("app_args")
                .expect("App args not set")
                .as_ref()
                .clone()
        };
        
        // Convert GFile paths to strings
        let file_paths: Vec<String> = files
            .iter()
            .filter_map(|file| file.path())
            .filter_map(|path| path.to_str())
            .map(|s| s.to_string())
            .collect();
        
        // Store files to open
        let mut args_with_files = args;
        args_with_files.files.extend(file_paths);
        
        unsafe {
            app.set_data("app_args", args_with_files);
        }
        
        // Build UI if not already built
        build_ui(app);
    });
    
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
    let app_state = match AppState::new() {
        Ok(state) => Arc::new(state),
        Err(e) => {
            eprintln!("Failed to create application state: {}", e);
            // Show error dialog
            glib::idle_add_once(move || {
                show_error_dialog(None, "Initialization Error", &format!("Failed to create application: {}", e));
            });
            return;
        }
    };
    
    // Create the backend client
    let backend_config = BackendConfig::default();
    let backend_client = match BackendClient::new(backend_config) {
        Ok(client) => Arc::new(client),
        Err(e) => {
            eprintln!("Failed to create backend client: {}", e);
            // Show warning dialog but continue
            glib::idle_add_once(move || {
                show_error_dialog(None, "Backend Warning", &format!("Failed to connect to backend: {}. Some features may not be available.", e));
            });
            // Continue without backend client
            None
        }
    };
    
    // Create the model manager if backend client is available
    let model_manager = if let Some(ref backend_client) = backend_client {
        let manager = Arc::new(ModelManager::new(Arc::clone(backend_client)));
        
        // Initialize the model manager
        let manager_clone = manager.clone();
        let app_state_clone = app_state.clone();
        runtime.spawn(async move {
            if let Err(e) = manager_clone.initialize().await {
                eprintln!("Failed to initialize model manager: {}", e);
                app_state_clone.set_status_message(format!("Failed to initialize models: {}", e)).await;
            }
        });
        
        Some(manager)
    } else {
        None
    };
    
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
    
    // Store the app state, runtime, backend client, and model manager in the application
    unsafe {
        app.set_data("app_state", app_state);
        app.set_data("runtime", runtime);
        if let Some(backend_client) = backend_client {
            app.set_data("backend_client", backend_client);
        }
        if let Some(model_manager) = model_manager {
            app.set_data("model_manager", model_manager);
        }
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
    
    // Create the main UI with the new components
    let main_window = match ui::MainWindow::new(app, app_state.clone()) {
        Ok(window) => window,
        Err(e) => {
            show_error_dialog(Some(&window), "UI Error", &format!("Failed to create UI: {}", e));
            return;
        }
    };
    
    // Show the window
    main_window.show();
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