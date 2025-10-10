//! Transcription Panel Component
//!
//! This module provides the main transcription panel that displays transcribed
//! text and related information to users.

use glib::clone;
use gtk4::prelude::*;
use gtk4::{
    Box, Paned, Label, ScrolledWindow, Frame, Expander, Button, Orientation,
    Align, PositionType, Grid, ProgressBar, Adjustment, Scale, Switch,
    ApplicationWindow, InfoBar, Widget
};
use std::sync::{Arc, Mutex};

use crate::models::{AppState, AudioFile};
use crate::models::transcription::{TranscriptionResult, TranscriptionTask, TranscriptionStatus};
use crate::services::TranscriptionService;
use crate::ui::widgets::transcription_text::{TranscriptionTextWidget, TranscriptionViewMode};
use crate::ui::widgets::transcription_controls::TranscriptionControlsWidget;
use crate::ui::widgets::{audio_preview::AudioPreviewWidget, waveform::WaveformWidget};
use crate::utils::AppError;

/// Transcription panel for displaying and managing transcriptions
pub struct TranscriptionPanel {
    /// Main container
    container: Box,
    /// Application state
    app_state: Arc<AppState>,
    /// Transcription service
    transcription_service: Option<Arc<TranscriptionService>>,
    /// Paned container for layout
    paned: Paned,
    /// Text widget for displaying transcription
    text_widget: TranscriptionTextWidget,
    /// Controls widget for transcription operations
    controls_widget: TranscriptionControlsWidget,
    /// Sidebar with metadata and information
    sidebar: Box,
    /// Transcription info frame
    info_frame: Frame,
    /// Confidence indicator
    confidence_progress: ProgressBar,
    /// Language label
    language_label: Label,
    /// Duration label
    duration_label: Label,
    /// Status label
    status_label: Label,
    /// Model label
    model_label: Label,
    /// Current transcription task
    current_task: Arc<Mutex<Option<TranscriptionTask>>>,
    /// Current transcription result
    current_result: Arc<Mutex<Option<TranscriptionResult>>>,
    /// Audio preview widget
    audio_preview: AudioPreviewWidget,
    /// Waveform widget
    waveform: WaveformWidget,
    /// Audio preview expander
    audio_preview_expander: Expander,
    /// Waveform expander
    waveform_expander: Expander,
    /// Current audio file
    current_audio_file: Arc<Mutex<Option<AudioFile>>>,
}

impl TranscriptionPanel {
    /// Create a new transcription panel
    pub fn new(app_state: Arc<AppState>) -> Result<Self, AppError> {
        // Create the main container
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(0)
            .build();

        // Create the paned container for layout
        let paned = Paned::builder()
            .orientation(Orientation::Horizontal)
            .wide_handle(true)
            .build();

        // Create the text widget
        let text_widget = TranscriptionTextWidget::new()?;

        // Create the controls widget
        let controls_widget = TranscriptionControlsWidget::new()?;
        
        // Create the audio preview widget
        let audio_preview = AudioPreviewWidget::new()?;
        
        // Create the waveform widget
        let waveform = WaveformWidget::new()?;
        
        // Create expanders for audio components
        let audio_preview_expander = Expander::builder()
            .label("Audio Preview")
            .expanded(false)
            .build();
        
        let waveform_expander = Expander::builder()
            .label("Waveform")
            .expanded(false)
            .build();
        
        // Set up audio components
        audio_preview_expander.set_child(Some(&audio_preview.get_widget()));
        waveform_expander.set_child(Some(&waveform.get_widget()));

        // Create the sidebar
        let sidebar = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .width_request(300)
            .build();

        // Create the info frame
        let info_frame = Frame::builder()
            .label("Transcription Information")
            .build();

        // Create the info grid
        let info_grid = Grid::builder()
            .row_spacing(5)
            .column_spacing(10)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        // Create labels for metadata
        let status_label = Label::builder()
            .label("Status:")
            .halign(Align::Start)
            .build();
        let status_value = Label::builder()
            .label("No transcription")
            .halign(Align::Start)
            .build();
        info_grid.attach(&status_label, 0, 0, 1, 1);
        info_grid.attach(&status_value, 1, 0, 1, 1);

        let language_label = Label::builder()
            .label("Language:")
            .halign(Align::Start)
            .build();
        let language_value = Label::builder()
            .label("-")
            .halign(Align::Start)
            .build();
        info_grid.attach(&language_label, 0, 1, 1, 1);
        info_grid.attach(&language_value, 1, 1, 1, 1);

        let duration_label = Label::builder()
            .label("Duration:")
            .halign(Align::Start)
            .build();
        let duration_value = Label::builder()
            .label("-")
            .halign(Align::Start)
            .build();
        info_grid.attach(&duration_label, 0, 2, 1, 1);
        info_grid.attach(&duration_value, 1, 2, 1, 1);

        let model_label = Label::builder()
            .label("Model:")
            .halign(Align::Start)
            .build();
        let model_value = Label::builder()
            .label("-")
            .halign(Align::Start)
            .build();
        info_grid.attach(&model_label, 0, 3, 1, 1);
        info_grid.attach(&model_value, 1, 3, 1, 1);

        let confidence_label = Label::builder()
            .label("Confidence:")
            .halign(Align::Start)
            .build();
        let confidence_progress = ProgressBar::builder()
            .text("0%")
            .show_text(true)
            .hexpand(true)
            .build();
        info_grid.attach(&confidence_label, 0, 4, 1, 1);
        info_grid.attach(&confidence_progress, 1, 4, 1, 1);

        // Add the grid to the info frame
        info_frame.set_child(Some(&info_grid));

        // Create actions frame
        let actions_frame = Frame::builder()
            .label("Actions")
            .build();

        let actions_box = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(5)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        let new_transcription_button = Button::builder()
            .label("New Transcription")
            .build();
        actions_box.append(&new_transcription_button);

        let save_button = Button::builder()
            .label("Save")
            .build();
        actions_box.append(&save_button);

        let share_button = Button::builder()
            .label("Share")
            .build();
        actions_box.append(&share_button);

        actions_frame.set_child(Some(&actions_box));

        // Create notes frame
        let notes_frame = Frame::builder()
            .label("Notes")
            .build();

        let notes_scrolled = ScrolledWindow::builder()
            .min_content_height(150)
            .vexpand(true)
            .build();

        let notes_text = gtk4::TextView::builder()
            .wrap_mode(gtk4::WrapMode::Word)
            .left_margin(5)
            .right_margin(5)
            .top_margin(5)
            .bottom_margin(5)
            .build();

        notes_scrolled.set_child(Some(&notes_text));
        notes_frame.set_child(Some(&notes_scrolled));

        // Add components to the sidebar
        sidebar.append(&info_frame);
        sidebar.append(&actions_frame);
        sidebar.append(&notes_frame);

        // Create the main content area
        let content_box = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(0)
            .build();

        // Add the text widget to the content area
        content_box.append(text_widget.get_widget());

        // Add the controls widget to the content area
        content_box.append(controls_widget.get_widget());
        
        // Add a separator before audio components
        content_box.append(&gtk4::Separator::builder()
            .orientation(Orientation::Horizontal)
            .margin_top(5)
            .margin_bottom(5)
            .build());

        // Add audio components to the content area
        content_box.append(&audio_preview_expander);
        content_box.append(&waveform_expander);

        // Add the content area and sidebar to the paned container
        paned.set_start_child(Some(&content_box));
        paned.set_end_child(Some(&sidebar));
        paned.set_position(700); // Set initial divider position

        // Add the paned container to the main container
        container.append(&paned);

        // Create the widget
        let widget = Self {
            container,
            app_state,
            transcription_service: None,
            paned,
            text_widget,
            controls_widget,
            sidebar,
            info_frame,
            confidence_progress,
            language_label: language_value,
            duration_label: duration_value,
            status_label: status_value,
            model_label: model_value,
            current_task: Arc::new(Mutex::new(None)),
            current_result: Arc::new(Mutex::new(None)),
            audio_preview,
            waveform,
            audio_preview_expander,
            waveform_expander,
            current_audio_file: Arc::new(Mutex::new(None)),
        };

        // Set up event handlers
        widget.setup_events()?;

        Ok(widget)
    }

    /// Set up event handlers
    fn setup_events(&self) -> Result<(), AppError> {
        // Connect zoom adjustment between text widget and controls
        let text_zoom_adj = self.text_widget.get_zoom_adjustment();
        let controls_zoom_adj = self.controls_widget.get_zoom_adjustment();
        
        text_zoom_adj.connect_value_changed(clone!(@strong controls_zoom_adj => move |adj| {
            controls_zoom_adj.set_value(adj.value());
        }));
        
        controls_zoom_adj.connect_value_changed(clone!(@strong text_zoom_adj => move |adj| {
            text_zoom_adj.set_value(adj.value());
        }));

        Ok(())
    }

    /// Set the application window for dialogs
    pub fn set_application_window(&mut self, window: ApplicationWindow) {
        self.controls_widget.set_application_window(window);
        self.audio_preview.set_application_window(window);
        self.waveform.set_application_window(window);
    }

    /// Set the transcription service
    pub fn set_transcription_service(&mut self, service: Arc<TranscriptionService>) {
        self.transcription_service = Some(service);
    }

    /// Get the transcription service
    pub fn get_transcription_service(&self) -> Option<Arc<TranscriptionService>> {
        self.transcription_service.clone()
    }

    /// Set a transcription task to display
    pub fn set_transcription_task(&self, task: TranscriptionTask) {
        *self.current_task.lock().unwrap() = Some(task.clone());
        
        // Update the UI based on the task status
        self.update_task_status(&task);
        
        // If the task has a result, display it
        if let Some(ref result) = task.result {
            self.set_transcription_result(result.clone());
        }
    }

    /// Update the UI based on task status
    fn update_task_status(&self, task: &TranscriptionTask) {
        // Update status label
        let status_text = match task.status {
            TranscriptionStatus::Pending => "Pending".to_string(),
            TranscriptionStatus::InProgress => format!("In Progress ({:.0}%)", task.progress * 100.0),
            TranscriptionStatus::Completed => "Completed".to_string(),
            TranscriptionStatus::Failed => {
                if let Some(ref error) = task.error_message {
                    format!("Failed: {}", error)
                } else {
                    "Failed".to_string()
                }
            },
            TranscriptionStatus::Cancelled => "Cancelled".to_string(),
        };
        self.status_label.set_text(&status_text);
        
        // Update model label
        self.model_label.set_text(&task.model_name);
        
        // Update duration if available
        if let Some(ref result) = task.result {
            let duration_secs = result.audio_duration.as_secs();
            let minutes = duration_secs / 60;
            let seconds = duration_secs % 60;
            self.duration_label.set_text(&format!("{}:{:02}", minutes, seconds));
            
            // Update language
            self.language_label.set_text(&result.language);
            
            // Update confidence
            self.confidence_progress.set_fraction(result.confidence.into());
            self.confidence_progress.set_text(Some(&format!("{:.0}%", result.confidence * 100.0)));
        }
    }

    /// Set a transcription result to display
    pub fn set_transcription_result(&self, result: TranscriptionResult) {
        *self.current_result.lock().unwrap() = Some(result.clone());
        
        // Update the text widget
        self.text_widget.set_transcription_result(result.clone());
        
        // Update the controls widget
        self.controls_widget.set_transcription_result(result.clone());
        
        // Update the metadata
        let duration_secs = result.audio_duration.as_secs();
        let minutes = duration_secs / 60;
        let seconds = duration_secs % 60;
        self.duration_label.set_text(&format!("{}:{:02}", minutes, seconds));
        
        self.language_label.set_text(&result.language);
        
        self.confidence_progress.set_fraction(result.confidence.into());
        self.confidence_progress.set_text(Some(&format!("{:.0}%", result.confidence * 100.0)));
        
        self.status_label.set_text("Completed");
    }

    /// Set the view mode
    pub fn set_view_mode(&self, mode: TranscriptionViewMode) {
        self.text_widget.set_view_mode(mode);
        self.controls_widget.set_view_mode(mode);
    }

    /// Get the current view mode
    pub fn get_view_mode(&self) -> TranscriptionViewMode {
        self.text_widget.get_view_mode()
    }

    /// Get the current transcription result
    pub fn get_transcription_result(&self) -> Option<TranscriptionResult> {
        self.current_result.lock().unwrap().clone()
    }

    /// Get the current transcription task
    pub fn get_transcription_task(&self) -> Option<TranscriptionTask> {
        self.current_task.lock().unwrap().clone()
    }

    /// Clear the current transcription
    pub fn clear(&self) {
        *self.current_task.lock().unwrap() = None;
        *self.current_result.lock().unwrap() = None;
        
        // Clear the text widget
        self.text_widget.set_transcription_result(TranscriptionResult {
            id: uuid::Uuid::new_v4(),
            text: String::new(),
            confidence: 0.0,
            language: String::new(),
            audio_duration: std::time::Duration::default(),
            segments: Vec::new(),
            completed_at: chrono::Utc::now(),
        });
        
        // Reset the metadata
        self.status_label.set_text("No transcription");
        self.language_label.set_text("-");
        self.duration_label.set_text("-");
        self.model_label.set_text("-");
        self.confidence_progress.set_fraction(0.0);
        self.confidence_progress.set_text(Some("0%"));
    }

    /// Get the main container widget
    pub fn get_widget(&self) -> &Box {
        &self.container
    }

    /// Get the text widget
    pub fn get_text_widget(&self) -> &TranscriptionTextWidget {
        &self.text_widget
    }

    /// Get the controls widget
    pub fn get_controls_widget(&self) -> &TranscriptionControlsWidget {
        &self.controls_widget
    }

    /// Show the find dialog
    pub fn show_find_dialog(&self) {
        if let Some(window) = self.container.root().and_then(|root| {
            root.downcast::<ApplicationWindow>().ok()
        }) {
            self.text_widget.show_find_replace_dialog(&window);
        }
    }

    /// Toggle search mode
    pub fn toggle_search(&self) {
        self.text_widget.toggle_search();
    }

    /// Export the transcription
    pub fn export_transcription(&self) {
        // This would typically be called from a menu or button
        // For now, we'll just show the export dialog with a default format
        self.controls_widget.show_export_dialog(
            crate::ui::widgets::transcription_controls::ExportFormat::PlainText
        );
    }
    
    /// Load an audio file for preview in the transcription panel
    pub fn load_audio_file(&self, audio_file: AudioFile) -> Result<(), AppError> {
        // Store the current audio file
        if let Ok(mut current_file_guard) = self.current_audio_file.lock() {
            *current_file_guard = Some(audio_file.clone());
        }
        
        // Load the file in the audio preview widget
        self.audio_preview.load_file(audio_file.clone())?;
        
        // Load the waveform
        self.waveform.load_waveform(&audio_file.file_path)?;
        
        // Set up position synchronization between waveform and audio preview
        let waveform = self.waveform.clone();
        let audio_preview = self.audio_preview.clone();
        
        // When waveform position changes, update audio preview
        self.waveform.set_position_changed_callback(move |position| {
            audio_preview.seek(position);
        });
        
        // When audio preview position changes, update waveform
        let waveform_clone = waveform.clone();
        self.audio_preview.set_position_changed_callback(move |position| {
            waveform_clone.set_playback_position(position);
        });
        
        // Expand the audio preview sections
        self.audio_preview_expander.set_expanded(true);
        self.waveform_expander.set_expanded(true);
        
        Ok(())
    }
    
    /// Clear the audio preview
    pub fn clear_audio_preview(&self) {
        // Clear the current audio file
        if let Ok(mut current_file_guard) = self.current_audio_file.lock() {
            *current_file_guard = None;
        }
        
        // Clear the widgets
        self.audio_preview.clear();
        self.waveform.clear();
        
        // Collapse the audio preview sections
        self.audio_preview_expander.set_expanded(false);
        self.waveform_expander.set_expanded(false);
    }
    
    /// Get the current audio file
    pub fn get_current_audio_file(&self) -> Option<AudioFile> {
        if let Ok(current_file_guard) = self.current_audio_file.lock() {
            current_file_guard.clone()
        } else {
            None
        }
    }
    
    /// Get the audio preview widget
    pub fn get_audio_preview(&self) -> &AudioPreviewWidget {
        &self.audio_preview
    }
    
    /// Get the waveform widget
    pub fn get_waveform(&self) -> &WaveformWidget {
        &self.waveform
    }
    
    /// Set the audio preview to sync with transcription segments
    pub fn sync_with_transcription(&self) {
        // In a real implementation, you would synchronize the audio playback
        // with the transcription segments, highlighting the current segment
        // as the audio plays
        
        // Get the current transcription result
        if let Some(ref result) = self.current_result.lock().unwrap().as_ref() {
            // Set up position synchronization
            let segments = result.segments.clone();
            let waveform = self.waveform.clone();
            let text_widget = self.text_widget.clone();
            
            // When audio position changes, highlight the current segment
            self.audio_preview.set_position_changed_callback(move |position| {
                // Find the segment that corresponds to the current position
                for segment in &segments {
                    // In a real implementation, you would check the segment timestamps
                    // and highlight the appropriate text
                }
            });
        }
    }
}