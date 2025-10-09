//! UI components for the ASRPro application
//! 
//! This module contains all the UI components and widgets used in the application.

use gtk4::prelude::*;
use gtk4::ApplicationWindow;
use std::sync::Arc;

use crate::models::AppState;
use crate::utils::AppError;

/// Main UI container
pub struct MainUI {
    window: ApplicationWindow,
    app_state: Arc<AppState>,
}

impl MainUI {
    /// Create a new MainUI instance
    pub fn new(window: ApplicationWindow, app_state: Arc<AppState>) -> Self {
        Self {
            window,
            app_state,
        }
    }
    
    /// Initialize the UI
    pub async fn initialize(&self) -> Result<(), AppError> {
        // Create the main container
        let main_container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(0)
            .build();
        
        // Create the header bar
        let header_bar = self.create_header_bar();
        
        // Create the main content area
        let content_area = self.create_content_area();
        
        // Create the status bar
        let status_bar = self.create_status_bar();
        
        // Add all components to the main container
        main_container.append(&header_bar);
        main_container.append(&content_area);
        main_container.append(&status_bar);
        
        // Set the main container as the window child
        self.window.set_child(Some(&main_container));
        
        // Initialize the default view
        self.show_welcome_view();
        
        Ok(())
    }
    
    /// Create the header bar
    fn create_header_bar(&self) -> gtk4::Widget {
        let header_bar = gtk4::HeaderBar::builder()
            .title_widget(&gtk4::Label::new(Some("ASRPro")))
            .build();
        
        // Add a menu button
        let menu_button = gtk4::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .build();
        
        let menu_model = self.create_menu_model();
        menu_button.set_menu_model(Some(&menu_model));
        
        header_bar.pack_end(&menu_button);
        
        header_bar.upcast()
    }
    
    /// Create the menu model
    fn create_menu_model(&self) -> gtk4::gio::MenuModel {
        let menu = gtk4::gio::Menu::new();
        
        menu.append(Some("Preferences"), Some("app.preferences"));
        menu.append(Some("Keyboard Shortcuts"), Some("app.shortcuts"));
        menu.append(Some("Help"), Some("app.help"));
        menu.append(Some("About"), Some("app.about"));
        
        menu.upcast()
    }
    
    /// Create the main content area
    fn create_content_area(&self) -> gtk4::Widget {
        let content_area = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(10)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();
        
        // Welcome label
        let welcome_label = gtk4::Label::builder()
            .label("<b>Welcome to ASRPro</b>\n\nThis is the GTK4 frontend for the ASRPro speech recognition application.")
            .use_markup(true)
            .justify(gtk4::Justification::Center)
            .margin_bottom(20)
            .build();
        
        content_area.append(&welcome_label);
        
        // Test button
        let test_button = gtk4::Button::builder()
            .label("Test Application State")
            .halign(gtk4::Align::Center)
            .build();
        
        let app_state = self.app_state.clone();
        test_button.connect_clicked(move |_| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Test button clicked".to_string()).await;
                app_state_clone.set_progress(0.5).await;
            });
        });
        
        content_area.append(&test_button);
        
        content_area.upcast()
    }
    
    /// Create the status bar
    fn create_status_bar(&self) -> gtk4::Widget {
        let status_bar = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(10)
            .margin_top(5)
            .margin_bottom(5)
            .margin_start(10)
            .margin_end(10)
            .build();
        
        // Status label
        let status_label = gtk4::Label::builder()
            .label("Ready")
            .halign(gtk4::Align::Start)
            .hexpand(true)
            .build();
        
        // Progress bar
        let progress_bar = gtk4::ProgressBar::builder()
            .text("")
            .show_text(true)
            .hexpand(false)
            .width_request(200)
            .build();
        
        status_bar.append(&status_label);
        status_bar.append(&progress_bar);
        
        // Store widgets for updates
        unsafe {
            self.window.set_data("status_label", status_label);
            self.window.set_data("progress_bar", progress_bar);
        }
        
        // Set up status updates
        self.setup_status_updates();
        
        status_bar.upcast()
    }
    
    /// Set up status updates
    fn setup_status_updates(&self) {
        let app_state = self.app_state.clone();
        let window = self.window.clone();
        
        // Update status periodically
        gtk4::glib::timeout_add_seconds_local(1, move || {
            let app_state_clone = app_state.clone();
            let window_clone = window.clone();
            
            gtk4::glib::spawn_future_local(async move {
                let ui_state = app_state_clone.get_ui_state().await;
                
                if let Some(status_label) = unsafe {
                    window_clone.data::<gtk4::Label>("status_label")
                        .map(|ptr| ptr.as_ref())
                } {
                    status_label.set_text(&ui_state.status_message);
                }
                
                if let Some(progress_bar) = unsafe {
                    window_clone.data::<gtk4::ProgressBar>("progress_bar")
                        .map(|ptr| ptr.as_ref())
                } {
                    progress_bar.set_fraction(ui_state.progress.into());
                    if ui_state.progress > 0.0 {
                        progress_bar.set_text(Some(&format!("{:.0}%", ui_state.progress * 100.0)));
                    } else {
                        progress_bar.set_text(None);
                    }
                }
            });
            
            gtk4::glib::ControlFlow::Continue
        });
    }
    
    /// Show the welcome view
    fn show_welcome_view(&self) {
        let app_state = self.app_state.clone();
        gtk4::glib::spawn_future_local(async move {
            app_state.set_status_message("Welcome to ASRPro".to_string()).await;
        });
    }
}

/// Create a module for widgets
pub mod widgets {
    // Widget modules will be added here
}