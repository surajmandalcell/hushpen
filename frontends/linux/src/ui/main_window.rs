//! Main window component for the ASRPro application
//! 
//! This module contains the MainWindow struct which provides the main application window
//! with proper GTK4 setup, header bar, content area, and status bar.

use glib::clone;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, HeaderBar, Label, Box, Paned,
    ProgressBar, Statusbar, Button, Orientation, Align, PolicyType,
    EventControllerKey, ShortcutTrigger, Shortcut, ShortcutAction, Widget,
    Image, Revealer, IconTheme
};
use gtk4::glib::{Propagation, ControlFlow};
use std::sync::Arc;

use crate::models::{AppState, websocket::ConnectionState, Settings, UiSettings};
use crate::services::{FileManager, BackendClient, ModelManager, TranscriptionService};
use crate::ui::menu_bar::MenuBar;
use crate::ui::{FilePanel, ModelPanel, TranscriptionPanel, initialize_styling, Theme};
use crate::utils::AppError;

/// Main application window
pub struct MainWindow {
    window: ApplicationWindow,
    app_state: Arc<AppState>,
    header_bar: HeaderBar,
    content_area: Paned,
    status_bar: Statusbar,
    progress_bar: ProgressBar,
    menu_bar: MenuBar,
    status_context_id: gtk4::glib::GString,
    file_panel: FilePanel,
    model_panel: Option<ModelPanel>,
    transcription_panel: Option<TranscriptionPanel>,
    model_manager: Option<Arc<ModelManager>>,
    transcription_service: Option<Arc<TranscriptionService>>,
    // WebSocket status indicators
    ws_status_image: Image,
    ws_status_label: Label,
    ws_status_revealer: Revealer,
}

impl MainWindow {
    /// Create a new MainWindow instance
    pub fn new(app: &Application, app_state: Arc<AppState>) -> Result<Self, AppError> {
        // Use default settings for now
        // In a real implementation, you would load this asynchronously
        let ui_settings = UiSettings::default();
        
        // Initialize styling with theme
        let theme = match ui_settings.theme.as_str() {
            "light" => Theme::Light,
            "dark" => Theme::Dark,
            _ => Theme::System,
        };
        initialize_styling(theme)?;
        
        // Create the main window
        let window = ApplicationWindow::builder()
            .application(app)
            .title("ASRPro - Speech Recognition")
            .default_width(1200)
            .default_height(800)
            .build();
        
        // Apply window state if remembered
        if ui_settings.remember_window_state {
            if let Some(window_state) = &ui_settings.window_state {
                window.set_default_size(window_state.width, window_state.height);
                if window_state.maximized {
                    window.maximize();
                }
            }
        }

        // Create the menu bar
        let menu_bar = MenuBar::new(&window, app_state.clone())?;
        
        // Create the file manager
        let mut file_manager = FileManager::with_default_config();
        
        // Set up the backend client for the file manager if available
        // In a real implementation, you would get this from the app state
        // For now, we'll leave it as None since the backend client setup
        // would require more complex initialization
        let file_manager = Arc::new(file_manager);
        
        // Create the file panel
        let file_panel = FilePanel::new(window.clone(), app_state.clone(), file_manager.clone())?;

        // Create the transcription panel
        let transcription_panel = TranscriptionPanel::new(app_state.clone())?;

        // Create the model manager and panel if backend client is available
        let (model_manager, model_panel, backend_client) = if let Ok(backend_config) = crate::models::api::BackendConfig::default() {
            match BackendClient::new(backend_config) {
                Ok(backend_client) => {
                    let backend_client = Arc::new(backend_client);
                    let model_manager = Arc::new(ModelManager::new(backend_client.clone()));
                    
                    // Initialize the model manager
                    let model_manager_clone = model_manager.clone();
                    gtk4::glib::spawn_future_local(async move {
                        if let Err(e) = model_manager_clone.initialize().await {
                            eprintln!("Failed to initialize model manager: {}", e);
                        }
                    });
                    
                    match ModelPanel::new(window.clone(), app_state.clone(), model_manager.clone()) {
                        Ok(model_panel) => (Some(model_manager), Some(model_panel), Some(backend_client)),
                        Err(e) => {
                            eprintln!("Failed to create model panel: {}", e);
                            (None, None, Some(backend_client))
                        }
                    }
                },
                Err(e) => {
                    eprintln!("Failed to create backend client: {}", e);
                    (None, None, None)
                }
            }
        } else {
            (None, None, None)
        };

        // Create the transcription service
        let transcription_service = Arc::new(TranscriptionService::new(
            app_state.clone(),
            backend_client,
            Some(file_manager.clone()),
        ));

        // Create the header bar
        let header_bar = HeaderBar::builder()
            .title_widget(&Label::new(Some("ASRPro")))
            .build();

        // Add menu button to header bar
        let menu_button = gtk4::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .tooltip_text("Application Menu")
            .build();
        
        let menu_model = menu_bar.get_menu_model();
        menu_button.set_menu_model(Some(&menu_model.clone().upcast::<gtk4::gio::MenuModel>()));
        header_bar.pack_end(&menu_button);

        // Create the main content area with paned layout
        let content_area = Paned::builder()
            .orientation(Orientation::Horizontal)
            .wide_handle(true)
            .build();

        // Create the status bar
        let status_bar = Statusbar::builder()
            .margin_top(5)
            .margin_bottom(5)
            .margin_start(10)
            .margin_end(10)
            .build();
        
        let status_context_id = status_bar.context_id("main").to_string();

        // Create the progress bar
        let progress_bar = ProgressBar::builder()
            .text("")
            .show_text(true)
            .hexpand(false)
            .width_request(200)
            .margin_end(10)
            .build();

        // Create WebSocket status indicators
        let ws_status_image = Image::builder()
            .icon_name("network-offline-symbolic")
            .pixel_size(16)
            .margin_end(5)
            .build();
        
        let ws_status_label = Label::builder()
            .label("Disconnected")
            .margin_end(10)
            .build();
        
        let ws_status_container = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(5)
            .build();
        ws_status_container.append(&ws_status_image);
        ws_status_container.append(&ws_status_label);
        
        let ws_status_revealer = Revealer::builder()
            .child(&ws_status_container)
            .reveal_child(false)
            .build();

        // Create the main window instance
        let main_window = Self {
            window,
            app_state,
            header_bar,
            content_area,
            status_bar,
            progress_bar,
            menu_bar,
            status_context_id: status_context_id.into(),
            file_panel,
            model_panel,
            transcription_panel: Some(transcription_panel),
            model_manager,
            transcription_service: Some(transcription_service),
            ws_status_image,
            ws_status_label,
            ws_status_revealer,
        };

        // Set up the window
        main_window.setup_window()?;
        main_window.setup_keyboard_shortcuts()?;
        main_window.setup_content_areas()?;
        main_window.setup_status_updates()?;
        main_window.setup_websocket_status_updates()?;
        
        // Set the application window for the transcription panel
        if let Some(ref mut transcription_panel) = main_window.transcription_panel {
            transcription_panel.set_application_window(main_window.window.clone());
            transcription_panel.set_transcription_service(transcription_service.clone());
        }

        // Apply UI settings
        Self::apply_ui_settings(&main_window, &ui_settings);
        
        Ok(main_window)
    }
    
    /// Apply UI settings to the window
    fn apply_ui_settings(main_window: &Self, ui_settings: &UiSettings) {
        // Apply font settings
        let css_provider = gtk4::CssProvider::new();
        let css = format!(
            "* {{
                font-family: {};
                font-size: {}pt;
            }}",
            ui_settings.font_family,
            ui_settings.font_size
        );
        
        css_provider.load_from_data(css.as_bytes());
        
        // Apply the CSS provider to the style context
        let style_context = main_window.window.style_context();
        style_context.add_provider(&css_provider, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION);
        
        // Apply animation settings
        if ui_settings.animate_transitions {
            main_window.window.add_css_class("animate-transitions");
        } else {
            main_window.window.add_css_class("no-animations");
        }
    }
    
    /// Update UI based on settings changes
    pub async fn update_from_settings(&mut self) -> Result<(), AppError> {
        let settings = self.app_state.get_settings().await;
        let ui_settings = settings.ui.clone();
        
        // Apply theme
        let theme = match ui_settings.theme.as_str() {
            "light" => Theme::Light,
            "dark" => Theme::Dark,
            _ => Theme::System,
        };
        initialize_styling(theme)?;
        
        // Apply UI settings
        Self::apply_ui_settings(self, &ui_settings);
        
        // Apply window state
        if ui_settings.remember_window_state {
            if let Some(window_state) = &ui_settings.window_state {
                self.window.set_default_size(window_state.width, window_state.height);
                if window_state.maximized {
                    self.window.maximize();
                } else {
                    self.window.unmaximize();
                }
            }
        }
        
        Ok(())
    }

    /// Set up the main window properties and event handlers
    fn setup_window(&self) -> Result<(), AppError> {
        // Set the header bar
        self.window.set_titlebar(Some(&self.header_bar));

        // Create the main container
        let main_container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(0)
            .build();

        // Create a container for the status bar and progress bar
        let status_container = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(10)
            .margin_top(5)
            .margin_bottom(5)
            .margin_start(10)
            .margin_end(10)
            .build();

        status_container.append(&self.status_bar);
        status_container.append(&self.progress_bar);
        status_container.append(&gtk4::Separator::builder()
            .orientation(Orientation::Vertical)
            .margin_start(10)
            .margin_end(10)
            .build());
        status_container.append(&self.ws_status_revealer);

        // Add components to the main container
        main_container.append(&self.content_area.clone().upcast::<Widget>());
        main_container.append(&status_container);

        // Set the main container as the window child
        self.window.set_child(Some(&main_container));

        // Set up window close handler
        let app_state = self.app_state.clone();
        self.window.connect_close_request(move |_| {
            // Handle application shutdown
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Shutting down...".to_string()).await;
            });
            Propagation::Proceed
        });

        // Set up window state change handlers
        let app_state = self.app_state.clone();
        self.window.connect_state_flags_changed(move |_, flags| {
            let is_maximized = flags.contains(gtk4::StateFlags::BACKDROP);
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                if is_maximized {
                    app_state_clone.set_status_message("Window state changed".to_string()).await;
                } else {
                    app_state_clone.set_status_message("Window restored".to_string()).await;
                }
            });
        });

        Ok(())
    }

    /// Set up keyboard shortcuts
    fn setup_keyboard_shortcuts(&self) -> Result<(), AppError> {
        // Create a shortcut controller
        let controller = EventControllerKey::new();

        // Set up basic keyboard shortcuts
        // Note: GDK key handling would require additional dependencies
        // For now, we'll skip the complex keyboard shortcuts

        self.window.add_controller(controller);
        Ok(())
    }

    /// Set up the content areas
    fn setup_content_areas(&self) -> Result<(), AppError> {
        // Create left panel (sidebar)
        let left_panel = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .width_request(250)
            .build();

        // Add sidebar content
        let sidebar_label = Label::builder()
            .label("<b>Sidebar</b>")
            .use_markup(true)
            .margin_bottom(10)
            .build();
        left_panel.append(&sidebar_label);

        // Add navigation buttons
        let file_panel_button = Button::builder()
            .label("Files")
            .margin_bottom(5)
            .build();
        left_panel.append(&file_panel_button);

        let model_panel_button = Button::builder()
            .label("Models")
            .margin_bottom(5)
            .build();
        left_panel.append(&model_panel_button);

        let button1 = Button::builder()
            .label("Record Audio")
            .margin_bottom(5)
            .build();
        left_panel.append(&button1);

        let button2 = Button::builder()
            .label("Upload File")
            .margin_bottom(5)
            .build();
        left_panel.append(&button2);

        let transcription_button = Button::builder()
            .label("Transcription")
            .margin_bottom(5)
            .build();
        left_panel.append(&transcription_button);

        let button3 = Button::builder()
            .label("History")
            .margin_bottom(5)
            .build();
        left_panel.append(&button3);

        // Create right panel (main content)
        let right_panel = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        // Create a stack for switching between panels
        let content_stack = gtk4::Stack::builder()
            .transition_type(gtk4::StackTransitionType::SlideLeftRight)
            .build();

        // Add welcome page to the stack
        let welcome_page = Self::create_welcome_page();
        content_stack.add_titled(&welcome_page, Some("welcome"), "Welcome");

        // Add file panel to the stack
        content_stack.add_titled(&self.file_panel.get_widget(), Some("files"), "Files");

        // Add model panel to the stack if available
        if let Some(ref model_panel) = self.model_panel {
            content_stack.add_typed(model_panel.get_widget(), Some("models"), "Models");
        }

        // Add transcription panel to the stack
        if let Some(ref transcription_panel) = self.transcription_panel {
            content_stack.add_typed(transcription_panel.get_widget(), Some("transcription"), "Transcription");
        }

        // Set the default visible child
        content_stack.set_visible_child_name("welcome");

        right_panel.append(&content_stack);

        // Add panels to the paned container
        self.content_area.set_start_child(Some(&left_panel));
        self.content_area.set_end_child(Some(&right_panel));
        self.content_area.set_position(300); // Set initial divider position

        // Set up button handlers
        let app_state = self.app_state.clone();
        let content_stack = content_stack.clone();
        file_panel_button.connect_clicked(move |_| {
            content_stack.set_visible_child_name("files");
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Switched to Files panel".to_string()).await;
            });
        });

        let app_state = self.app_state.clone();
        let content_stack = content_stack.clone();
        model_panel_button.connect_clicked(move |_| {
            content_stack.set_visible_child_name("models");
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Switched to Models panel".to_string()).await;
            });
        });

        let app_state = self.app_state.clone();
        button1.connect_clicked(move |_| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Recording audio...".to_string()).await;
                app_state_clone.set_progress(0.0).await;
            });
        });

        let app_state = self.app_state.clone();
        button2.connect_clicked(move |_| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Opening file dialog...".to_string()).await;
            });
        });

        let app_state = self.app_state.clone();
        let content_stack_clone = content_stack.clone();
        transcription_button.connect_clicked(move |_| {
            content_stack_clone.set_visible_child_name("transcription");
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Switched to Transcription panel".to_string()).await;
            });
        });

        let app_state = self.app_state.clone();
        button3.connect_clicked(move |_| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Loading history...".to_string()).await;
            });
        });

        Ok(())
    }

    /// Set up status updates
    fn setup_status_updates(&self) -> Result<(), AppError> {
        let app_state = self.app_state.clone();
        let status_bar = self.status_bar.clone();
        let progress_bar = self.progress_bar.clone();
        let status_context_id = self.status_context_id.clone();

        // Update status periodically
        gtk4::glib::timeout_add_seconds_local(1, move || {
            let app_state_clone = app_state.clone();
            let status_bar_clone = status_bar.clone();
            let progress_bar_clone = progress_bar.clone();
            let context_id = status_context_id.clone();

            gtk4::glib::spawn_future_local(async move {
                let ui_state = app_state_clone.get_ui_state().await;
                
                // Update status bar
                let _ = status_bar_clone.push(context_id.parse::<u32>().unwrap_or(0), &ui_state.status_message);
                
                // Update progress bar
                progress_bar_clone.set_fraction(ui_state.progress.into());
                if ui_state.progress > 0.0 {
                    progress_bar_clone.set_text(Some(&format!("{:.0}%", ui_state.progress * 100.0)));
                } else {
                    progress_bar_clone.set_text(None);
                }
            });
            
            ControlFlow::Continue
        });

        Ok(())
    }

    /// Get the application window
    pub fn get_window(&self) -> &ApplicationWindow {
        &self.window
    }

    /// Get the content area
    pub fn get_content_area(&self) -> &Paned {
        &self.content_area
    }

    /// Show the window
    pub fn show(&self) {
        self.window.present();
        
        // Show welcome message
        let app_state = self.app_state.clone();
        gtk4::glib::spawn_future_local(async move {
            app_state.set_status_message("Welcome to ASRPro".to_string()).await;
            app_state.show_notification(
                "Welcome to ASRPro".to_string(),
                "The application is ready to use".to_string(),
            ).await;
        });
    }

    /// Update the window title
    pub fn set_title(&self, title: &str) {
        self.window.set_title(Some(title));
    }

    /// Get the menu bar
    pub fn get_menu_bar(&self) -> &MenuBar {
        &self.menu_bar
    }

    /// Get the transcription service
    pub fn get_transcription_service(&self) -> Option<Arc<TranscriptionService>> {
        self.transcription_service.clone()
    }

    /// Get the transcription panel
    pub fn get_transcription_panel(&self) -> Option<&TranscriptionPanel> {
        self.transcription_panel.as_ref()
    }

    /// Create the welcome page
    fn create_welcome_page() -> gtk4::Box {
        let container = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(20)
            .margin_top(40)
            .margin_bottom(40)
            .margin_start(40)
            .margin_end(40)
            .build();

        // Add welcome title
        let title_label = Label::builder()
            .label("<b>Welcome to ASRPro</b>")
            .use_markup(true)
            .justify(gtk4::Justification::Center)
            .margin_bottom(20)
            .build();
        container.append(&title_label);

        // Add welcome message
        let welcome_label = Label::builder()
            .label("This is the main content area where transcription results will be displayed.\n\nUse the sidebar to start recording, upload audio files, or manage models.")
            .justify(gtk4::Justification::Center)
            .wrap(true)
            .margin_bottom(20)
            .build();
        container.append(&welcome_label);

        // Add action buttons
        let button_container = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(15)
            .halign(Align::Center)
            .build();

        let record_button = Button::builder()
            .label("Record Audio")
            .icon_name("audio-input-microphone")
            .build();
        button_container.append(&record_button);

        let upload_button = Button::builder()
            .label("Upload File")
            .icon_name("document-open")
            .build();
        button_container.append(&upload_button);

        let models_button = Button::builder()
            .label("Manage Models")
            .icon_name("system-run")
            .build();
        button_container.append(&models_button);

        container.append(&button_container);

        container
    }
}