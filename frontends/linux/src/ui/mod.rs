//! UI components for the ASRPro application
//!
//! This module contains all the UI components and widgets used in the application.

use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow};
use std::sync::Arc;

use crate::models::AppState;
use crate::utils::AppError;

// Import our UI components
pub mod main_window;
pub mod menu_bar;
pub mod style;
pub mod file_panel;
pub mod model_panel;
pub mod transcription_panel;

// Import widget modules
pub mod widgets {
    pub mod file_drop;
    pub mod file_list;
    pub mod model_selector;
    pub mod model_details;
    pub mod transcription_text;
    pub mod transcription_controls;
    pub mod audio_preview;
    pub mod waveform;
}

// Re-export the main components
pub use main_window::MainWindow;
pub use style::{Theme, initialize_styling};
pub use file_panel::FilePanel;
pub use model_panel::ModelPanel;
pub use transcription_panel::TranscriptionPanel;
pub use widgets::model_selector::ModelSelectorWidget;
pub use widgets::model_details::ModelDetailsWidget;
pub use widgets::transcription_text::TranscriptionTextWidget;
pub use widgets::transcription_controls::TranscriptionControlsWidget;
pub use widgets::audio_preview::AudioPreviewWidget;
pub use widgets::waveform::WaveformWidget;

/// Main UI container
pub struct MainUI {
    main_window: MainWindow,
    app_state: Arc<AppState>,
}

impl MainUI {
    /// Create a new MainUI instance
    pub fn new(window: ApplicationWindow, app_state: Arc<AppState>) -> Result<Self, AppError> {
        // Create the main window component
        let main_window = MainWindow::new(&window.application().unwrap(), app_state.clone())?;
        
        Ok(Self {
            main_window,
            app_state,
        })
    }
    
    /// Initialize the UI
    pub async fn initialize(&self) -> Result<(), AppError> {
        // Initialize the styling
        let _style = initialize_styling(Theme::Light);
        
        // Show the main window
        self.main_window.show();
        
        // Show welcome message
        let app_state = self.app_state.clone();
        gtk4::glib::spawn_future_local(async move {
            app_state.set_status_message("UI initialized successfully".to_string()).await;
        });
        
        Ok(())
    }
    
    /// Get the main window
    pub fn get_main_window(&self) -> &MainWindow {
        &self.main_window
    }
    
    /// Get the application window
    pub fn get_window(&self) -> &ApplicationWindow {
        self.main_window.get_window()
    }
}
