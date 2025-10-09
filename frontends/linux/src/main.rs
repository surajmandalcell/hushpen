use gio::prelude::*;
use glib::clone;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Button, TextView, TextBuffer, ScrolledWindow, Box as GtkBox, Orientation};
use gtk4 as gtk;
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

#[tokio::main]
async fn main() {
    // Create GTK application
    let app = Application::builder()
        .application_id(APP_ID)
        .build();

    // Connect to activate signal
    app.connect_activate(build_ui);

    // Run the application
    app.run();
}

fn build_ui(app: &Application) {
    // Create HTTP client
    let http_client = reqwest::Client::new();
    
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

    // Create transcribe button
    let transcribe_button = Button::builder()
        .label("Transcribe")
        .margin_top(10)
        .build();

    // Create text view for results
    let result_buffer = TextBuffer::new(None);
    let text_view = TextView::builder()
        .buffer(&result_buffer)
        .editable(false)
        .wrap_mode(gtk::WrapMode::Word)
        .build();

    // Create scrolled window for text view
    let scrolled_window = ScrolledWindow::builder()
        .min_content_height(200)
        .child(&text_view)
        .build();

    // Add widgets to main container
    main_box.append(&transcribe_button);
    main_box.append(&scrolled_window);

    // Set up application state
    let app_state = Arc::new(Mutex::new(AppState {
        http_client,
        result_buffer: result_buffer.clone(),
    }));

    // Connect button click signal
    transcribe_button.connect_clicked(clone!(@strong window => move |button| {
        button.set_sensitive(false);
        button.set_label("Transcribing...");
        
        let app_state_clone = Arc::clone(&app_state);
        let window_clone = window.clone();
        
        // Execute async operation
        let context = MainContext::default();
        context.spawn_local(async move {
            match check_backend_health(&app_state_clone).await {
                Ok(health_status) => {
                    update_result_text(&app_state_clone, &format!("Backend health check: {}\n\nReady to transcribe audio files.", health_status)).await;
                }
                Err(error) => {
                    update_result_text(&app_state_clone, &format!("Error connecting to backend: {}\n\nPlease ensure the backend server is running at {}.", error, BACKEND_URL)).await;
                }
            }
            
            // Reset button
            button.set_sensitive(true);
            button.set_label("Transcribe");
        });
    }));

    // Add main container to window
    window.set_child(Some(&main_box));
    
    // Show window
    window.present();
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