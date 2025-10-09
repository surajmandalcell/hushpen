use gio::prelude::*;
use glib::clone;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Button, TextView, TextBuffer, ScrolledWindow, Box as GtkBox, Orientation, MessageDialog, MessageType, ButtonsType};
use gtk4::prelude::{ApplicationExt, ApplicationExtManual};
use glib::MainContext;
use std::sync::Arc;
use tokio::sync::Mutex;

// HTTP client for backend communication
use reqwest;
use serde_json;

const APP_ID: &str = "com.asrpro.gtk4";
const BACKEND_URL: &str = "http://localhost:8000";

struct AppState {
    http_client: reqwest::Client,
    result_buffer: TextBuffer,
}

fn main() {
    // Initialize GTK
    let app = Application::builder()
        .application_id(APP_ID)
        .build();

    // Connect to activate signal
    app.connect_activate(build_ui);

    // Run the application
    app.run();
}

fn build_ui(app: &Application) {
    // Create main window
    let window = ApplicationWindow::builder()
        .application(app)
        .title("ASRPro - GTK4 Frontend")
        .default_width(600)
        .default_height(400)
        .build();

    // Create main container
    let main_box = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(10)
        .margin_end(10)
        .build();

    // Create welcome label
    let welcome_label = gtk4::Label::builder()
        .label("<b>Welcome to ASRPro</b>\n\nThis is the GTK4 frontend for the ASRPro application.")
        .use_markup(true)
        .justify(gtk4::Justification::Center)
        .margin_bottom(20)
        .build();

    // Create transcribe button
    let transcribe_button = Button::builder()
        .label("Test Backend Connection")
        .margin_top(10)
        .halign(gtk4::Align::Center)
        .build();

    // Create text view for results
    let result_buffer = TextBuffer::new(None);
    // Set initial welcome message
    result_buffer.set_text("Welcome to ASRPro!\n\nClick the button above to test the backend connection.\n\nIf the backend is not running, you'll see an error message, but the application will continue to work.");
    
    let text_view = TextView::builder()
        .buffer(&result_buffer)
        .editable(false)
        .wrap_mode(gtk4::WrapMode::Word)
        .build();

    // Create scrolled window for text view
    let scrolled_window = ScrolledWindow::builder()
        .min_content_height(200)
        .child(&text_view)
        .build();

    // Add widgets to main container
    main_box.append(&welcome_label);
    main_box.append(&transcribe_button);
    main_box.append(&scrolled_window);

    // Create HTTP client
    let http_client = reqwest::Client::new();
    
    // Set up application state
    let app_state = Arc::new(Mutex::new(AppState {
        http_client,
        result_buffer: result_buffer.clone(),
    }));

    // Connect button click signal
    transcribe_button.connect_clicked(clone!(@strong window, @strong transcribe_button => move |_| {
        transcribe_button.set_sensitive(false);
        transcribe_button.set_label("Testing...");
        
        let app_state_clone = Arc::clone(&app_state);
        let window_clone = window.clone();
        let button_clone = transcribe_button.clone();
        
        // Execute async operation
        let context = MainContext::default();
        context.spawn_local(async move {
            match check_backend_health(&app_state_clone).await {
                Ok(health_status) => {
                    update_result_text(&app_state_clone, &format!("✓ Backend health check: {}\n\nReady to transcribe audio files.\n\nThe backend is running and accessible.", health_status)).await;
                    show_info_dialog(&window_clone, "Backend Connected", "Successfully connected to the ASRPro backend!");
                }
                Err(error) => {
                    update_result_text(&app_state_clone, &format!("⚠ Backend connection failed: {}\n\nPlease ensure the backend server is running at {}.\n\nYou can still use this application to test the GUI, but transcription features require the backend.", error, BACKEND_URL)).await;
                    show_info_dialog(&window_clone, "Backend Not Available", "The backend is not running, but the GUI is working correctly!");
                }
            }
            
            // Reset button
            button_clone.set_sensitive(true);
            button_clone.set_label("Test Backend Connection");
        });
    }));

    // Add main container to window
    window.set_child(Some(&main_box));
    
    // Show window
    window.present();
    
    // Show a welcome dialog
    show_info_dialog(&window, "Welcome to ASRPro", "The GTK4 frontend is running successfully!\n\nThis application provides a graphical interface for the ASRPro speech recognition system.");
}

async fn check_backend_health(app_state: &Arc<Mutex<AppState>>) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let state = app_state.lock().await;
    let response = state.http_client
        .get(&format!("{}/health", BACKEND_URL))
        .send()
        .await?;
    
    if response.status().is_success() {
        let health_json: serde_json::Value = response.json().await?;
        Ok(health_json.get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string())
    } else {
        Err(format!("Backend returned status: {}", response.status()).into())
    }
}

async fn update_result_text(app_state: &Arc<Mutex<AppState>>, text: &str) {
    let state = app_state.lock().await;
    state.result_buffer.set_text(text);
}

fn show_info_dialog(parent: &ApplicationWindow, title: &str, message: &str) {
    let dialog = MessageDialog::builder()
        .transient_for(parent)
        .modal(true)
        .title(title)
        .text(message)
        .buttons(ButtonsType::Ok)
        .message_type(MessageType::Info)
        .build();
    
    dialog.connect_response(|dialog, _| {
        dialog.close();
    });
    
    dialog.show();
}

fn show_error_dialog(app: &Application, title: &str, message: &str) {
    let dialog = MessageDialog::builder()
        .title(title)
        .text(message)
        .buttons(ButtonsType::Ok)
        .message_type(MessageType::Error)
        .build();
    
    dialog.connect_response(|dialog, _| {
        dialog.close();
    });
    
    dialog.show();
}