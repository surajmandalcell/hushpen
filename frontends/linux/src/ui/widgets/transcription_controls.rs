//! Transcription Controls Widget
//!
//! This module provides controls for transcription operations including
//! playback, editing, export, and view mode toggles.

use glib::clone;
use gtk4::prelude::*;
use gtk4::{
    Box, Button, Label, Scale, Adjustment, Switch, CheckButton, 
    Popover, MenuButton, PositionType, Orientation, Align, IconSize,
    Separator, SpinButton, ToolBar, ToggleToolButton, ToolButton,
    ApplicationWindow, FileChooserAction, FileChooserNative,
    ResponseType, DialogFlags, ButtonsType, MessageType,
    MessageDialog, Widget
};
use std::sync::{Arc, Mutex};

use crate::models::transcription::TranscriptionResult;
use crate::ui::widgets::transcription_text::TranscriptionViewMode;
use crate::utils::AppError;

/// Export format options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExportFormat {
    PlainText,
    SRT,
    VTT,
    JSON,
}

/// Transcription controls widget
pub struct TranscriptionControlsWidget {
    /// Main container
    container: Box,
    /// Application window for dialogs
    app_window: Option<ApplicationWindow>,
    /// Current transcription result
    current_result: Arc<Mutex<Option<TranscriptionResult>>>,
    /// Current view mode
    view_mode: Arc<Mutex<TranscriptionViewMode>>,
    /// Playback controls
    playback_controls: Box,
    /// Edit controls
    edit_controls: Box,
    /// Export controls
    export_controls: Box,
    /// View controls
    view_controls: Box,
    /// Play button
    play_button: ToggleToolButton,
    /// Stop button
    stop_button: ToolButton,
    /// Position scale
    position_scale: Scale,
    /// Volume scale
    volume_scale: Scale,
    /// Zoom scale
    zoom_scale: Scale,
    /// Export format popover
    export_popover: Popover,
    /// View mode popover
    view_mode_popover: Popover,
}

impl TranscriptionControlsWidget {
    /// Create a new transcription controls widget
    pub fn new() -> Result<Self, AppError> {
        // Create the main container
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(5)
            .build();

        // Create playback controls
        let playback_controls = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(5)
            .margin_top(5)
            .margin_bottom(5)
            .margin_start(10)
            .margin_end(10)
            .build();

        // Create edit controls
        let edit_controls = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(5)
            .margin_top(5)
            .margin_bottom(5)
            .margin_start(10)
            .margin_end(10)
            .build();

        // Create export controls
        let export_controls = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(5)
            .margin_top(5)
            .margin_bottom(5)
            .margin_start(10)
            .margin_end(10)
            .build();

        // Create view controls
        let view_controls = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(5)
            .margin_top(5)
            .margin_bottom(5)
            .margin_start(10)
            .margin_end(10)
            .build();

        // Create toolbar for playback controls
        let playback_toolbar = ToolBar::builder()
            .build();

        // Create play button
        let play_button = ToggleToolButton::builder()
            .icon_name("media-playback-start")
            .label("Play")
            .tooltip_text("Play/Pause")
            .build();

        // Create stop button
        let stop_button = ToolButton::builder()
            .icon_name("media-playback-stop")
            .label("Stop")
            .tooltip_text("Stop")
            .build();

        // Create position adjustment and scale
        let position_adjustment = Adjustment::new(0.0, 0.0, 100.0, 1.0, 10.0, 0.0);
        let position_scale = Scale::builder()
            .orientation(Orientation::Horizontal)
            .adjustment(&position_adjustment)
            .hexpand(true)
            .draw_value(false)
            .has_origin(false)
            .build();

        // Create volume adjustment and scale
        let volume_adjustment = Adjustment::new(0.7, 0.0, 1.0, 0.05, 0.1, 0.0);
        let volume_scale = Scale::builder()
            .orientation(Orientation::Horizontal)
            .adjustment(&volume_adjustment)
            .hexpand(false)
            .width_request(100)
            .draw_value(false)
            .has_origin(false)
            .build();

        // Create zoom adjustment and scale
        let zoom_adjustment = Adjustment::new(1.0, 0.5, 3.0, 0.1, 0.1, 0.0);
        let zoom_scale = Scale::builder()
            .orientation(Orientation::Horizontal)
            .adjustment(&zoom_adjustment)
            .hexpand(false)
            .width_request(100)
            .draw_value(false)
            .has_origin(false)
            .build();

        // Add buttons to toolbar
        playback_toolbar.add(&play_button);
        playback_toolbar.add(&stop_button);

        // Add position and volume controls to playback controls
        playback_controls.append(&playback_toolbar);
        playback_controls.append(&position_scale);
        
        let volume_label = Label::builder()
            .label("Volume:")
            .margin_start(10)
            .build();
        playback_controls.append(&volume_label);
        playback_controls.append(&volume_scale);

        // Create edit buttons
        let undo_button = Button::builder()
            .icon_name("edit-undo")
            .tooltip_text("Undo")
            .build();
        let redo_button = Button::builder()
            .icon_name("edit-redo")
            .tooltip_text("Redo")
            .build();
        let cut_button = Button::builder()
            .icon_name("edit-cut")
            .tooltip_text("Cut")
            .build();
        let copy_button = Button::builder()
            .icon_name("edit-copy")
            .tooltip_text("Copy")
            .build();
        let paste_button = Button::builder()
            .icon_name("edit-paste")
            .tooltip_text("Paste")
            .build();
        let find_button = Button::builder()
            .icon_name("edit-find")
            .label("Find")
            .tooltip_text("Find and Replace")
            .build();

        // Add edit buttons to edit controls
        edit_controls.append(&undo_button);
        edit_controls.append(&redo_button);
        edit_controls.append(&Separator::new(Orientation::Vertical));
        edit_controls.append(&cut_button);
        edit_controls.append(&copy_button);
        edit_controls.append(&paste_button);
        edit_controls.append(&Separator::new(Orientation::Vertical));
        edit_controls.append(&find_button);

        // Create export controls
        let export_button = MenuButton::builder()
            .icon_name("document-save-as")
            .label("Export")
            .tooltip_text("Export transcription")
            .build();

        // Create export format popover
        let export_popover = Popover::new();
        let export_box = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(5)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        let export_text_button = Button::builder()
            .label("Plain Text (.txt)")
            .build();
        let export_srt_button = Button::builder()
            .label("SubRip (.srt)")
            .build();
        let export_vtt_button = Button::builder()
            .label("WebVTT (.vtt)")
            .build();
        let export_json_button = Button::builder()
            .label("JSON (.json)")
            .build();

        export_box.append(&export_text_button);
        export_box.append(&export_srt_button);
        export_box.append(&export_vtt_button);
        export_box.append(&export_json_button);

        export_popover.set_child(Some(&export_box));
        export_button.set_popover(Some(&export_popover));

        export_controls.append(&export_button);

        // Create view controls
        let view_mode_button = MenuButton::builder()
            .icon_name("view-more")
            .label("View")
            .tooltip_text("View options")
            .build();

        // Create view mode popover
        let view_mode_popover = Popover::new();
        let view_box = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(5)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        let view_plain_radio = CheckButton::builder()
            .label("Plain Text")
            .build();
        let view_timestamps_radio = CheckButton::builder()
            .label("With Timestamps")
            .group(&view_plain_radio)
            .build();
        let view_speakers_radio = CheckButton::builder()
            .label("With Speakers")
            .group(&view_plain_radio)
            .build();
        let view_full_radio = CheckButton::builder()
            .label("Full View")
            .group(&view_plain_radio)
            .active(true)
            .build();

        view_box.append(&view_plain_radio);
        view_box.append(&view_timestamps_radio);
        view_box.append(&view_speakers_radio);
        view_box.append(&view_full_radio);

        let separator = Separator::new(Orientation::Horizontal);
        view_box.append(&separator);

        let zoom_label = Label::builder()
            .label("Text Size:")
            .halign(Align::Start)
            .build();
        view_box.append(&zoom_label);
        view_box.append(&zoom_scale);

        view_mode_popover.set_child(Some(&view_box));
        view_mode_button.set_popover(Some(&view_mode_popover));

        view_controls.append(&view_mode_button);

        // Create the widget
        let widget = Self {
            container,
            app_window: None,
            current_result: Arc::new(Mutex::new(None)),
            view_mode: Arc::new(Mutex::new(TranscriptionViewMode::Full)),
            playback_controls,
            edit_controls,
            export_controls,
            view_controls,
            play_button,
            stop_button,
            position_scale,
            volume_scale,
            zoom_scale,
            export_popover,
            view_mode_popover,
        };

        // Set up event handlers
        widget.setup_events()?;

        // Layout the components
        widget.layout_components();

        Ok(widget)
    }

    /// Set up event handlers
    fn setup_events(&self) -> Result<(), AppError> {
        // Set up play/pause button
        let play_button = self.play_button.clone();
        let stop_button = self.stop_button.clone();
        let position_scale = self.position_scale.clone();
        
        play_button.connect_toggled(move |button| {
            if button.is_active() {
                // Start playing
                stop_button.set_sensitive(true);
                // In a real implementation, you would start audio playback here
                println!("Playing audio");
            } else {
                // Pause playing
                println!("Paused audio");
            }
        });

        // Set up stop button
        stop_button.connect_clicked(move |_| {
            // Stop playing
            play_button.set_active(false);
            position_scale.set_value(0.0);
            println!("Stopped audio");
        });

        // Set up position scale
        position_scale.connect_value_changed(move |scale| {
            let position = scale.value();
            // In a real implementation, you would seek to this position
            println!("Position: {}", position);
        });

        // Set up volume scale
        let volume_scale = self.volume_scale.clone();
        volume_scale.connect_value_changed(move |scale| {
            let volume = scale.value();
            // In a real implementation, you would set the volume
            println!("Volume: {}", volume);
        });

        // Set up zoom scale
        let zoom_scale = self.zoom_scale.clone();
        zoom_scale.connect_value_changed(move |scale| {
            let zoom = scale.value();
            // In a real implementation, you would update the text view zoom
            println!("Zoom: {}", zoom);
        });

        Ok(())
    }

    /// Layout the components
    fn layout_components(&self) {
        // Add a separator between control groups
        let separator1 = Separator::new(Orientation::Horizontal);
        let separator2 = Separator::new(Orientation::Horizontal);
        let separator3 = Separator::new(Orientation::Horizontal);

        // Add components to the main container
        self.container.append(&self.playback_controls);
        self.container.append(&separator1);
        self.container.append(&self.edit_controls);
        self.container.append(&separator2);
        self.container.append(&self.export_controls);
        self.container.append(&separator3);
        self.container.append(&self.view_controls);
    }

    /// Set the application window for dialogs
    pub fn set_application_window(&mut self, window: ApplicationWindow) {
        self.app_window = Some(window);
    }

    /// Set the transcription result
    pub fn set_transcription_result(&self, result: TranscriptionResult) {
        *self.current_result.lock().unwrap() = Some(result);
    }

    /// Get the current view mode
    pub fn get_view_mode(&self) -> TranscriptionViewMode {
        *self.view_mode.lock().unwrap()
    }

    /// Set the view mode
    pub fn set_view_mode(&self, mode: TranscriptionViewMode) {
        *self.view_mode.lock().unwrap() = mode;
        
        // Update the radio buttons in the view mode popover
        // This would require storing references to the radio buttons
        // For now, we'll just update the internal state
    }

    /// Get the zoom adjustment
    pub fn get_zoom_adjustment(&self) -> &Adjustment {
        self.zoom_scale.adjustment().unwrap()
    }

    /// Get the position adjustment
    pub fn get_position_adjustment(&self) -> &Adjustment {
        self.position_scale.adjustment().unwrap()
    }

    /// Get the volume adjustment
    pub fn get_volume_adjustment(&self) -> &Adjustment {
        self.volume_scale.adjustment().unwrap()
    }

    /// Show export dialog and export the transcription
    pub fn show_export_dialog(&self, format: ExportFormat) {
        if let Some(ref window) = self.app_window {
            if let Some(ref result) = *self.current_result.lock().unwrap() {
                let (title, filter_name, filter_pattern) = match format {
                    ExportFormat::PlainText => ("Export as Plain Text", "Text files", "*.txt"),
                    ExportFormat::SRT => ("Export as SubRip", "SRT files", "*.srt"),
                    ExportFormat::VTT => ("Export as WebVTT", "VTT files", "*.vtt"),
                    ExportFormat::JSON => ("Export as JSON", "JSON files", "*.json"),
                };

                let file_chooser = FileChooserNative::builder()
                    .title(title)
                    .transient_for(window)
                    .action(FileChooserAction::Save)
                    .modal(true)
                    .build();

                // Add filter for the selected format
                let filter = gtk4::FileFilter::new();
                filter.set_name(Some(filter_name));
                filter.add_pattern(filter_pattern);
                file_chooser.add_filter(&filter);

                // Add "All files" filter
                let all_filter = gtk4::FileFilter::new();
                all_filter.set_name(Some("All files"));
                all_filter.add_pattern("*");
                file_chooser.add_filter(&all_filter);

                // Set default filename
                let default_name = match format {
                    ExportFormat::PlainText => "transcription.txt",
                    ExportFormat::SRT => "transcription.srt",
                    ExportFormat::VTT => "transcription.vtt",
                    ExportFormat::JSON => "transcription.json",
                };
                file_chooser.set_current_name(default_name);

                // Handle response
                let result_clone = result.clone();
                file_chooser.connect_response(move |dialog, response| {
                    if response == ResponseType::Accept {
                        if let Some(file) = dialog.file() {
                            if let Some(path) = file.path() {
                                let path_str = path.to_string_lossy().to_string();
                                
                                // Export the transcription
                                let content = match format {
                                    ExportFormat::PlainText => {
                                        // Export as plain text
                                        let mut content = String::new();
                                        for segment in &result_clone.segments {
                                            content.push_str(&segment.text);
                                            content.push(' ');
                                        }
                                        content
                                    },
                                    ExportFormat::SRT => {
                                        // Export as SRT
                                        let mut content = String::new();
                                        for (i, segment) in result_clone.segments.iter().enumerate() {
                                            content.push_str(&format!("{}\n", i + 1));
                                            
                                            let start_time = format_seconds_to_srt_time(
                                                segment.start.as_secs() as u32,
                                                segment.start.subsec_millis()
                                            );
                                            let end_time = format_seconds_to_srt_time(
                                                segment.end.as_secs() as u32,
                                                segment.end.subsec_millis()
                                            );
                                            content.push_str(&format!("{} --> {}\n", start_time, end_time));
                                            
                                            content.push_str(&segment.text);
                                            content.push_str("\n\n");
                                        }
                                        content
                                    },
                                    ExportFormat::VTT => {
                                        // Export as VTT
                                        let mut content = String::new();
                                        content.push_str("WEBVTT\n\n");
                                        
                                        for segment in &result_clone.segments {
                                            let start_time = format_seconds_to_vtt_time(
                                                segment.start.as_secs() as u32,
                                                segment.start.subsec_millis()
                                            );
                                            let end_time = format_seconds_to_vtt_time(
                                                segment.end.as_secs() as u32,
                                                segment.end.subsec_millis()
                                            );
                                            content.push_str(&format!("{} --> {}\n", start_time, end_time));
                                            
                                            content.push_str(&segment.text);
                                            content.push_str("\n\n");
                                        }
                                        content
                                    },
                                    ExportFormat::JSON => {
                                        // Export as JSON
                                        serde_json::to_string_pretty(&result_clone).unwrap_or_default()
                                    },
                                };
                                
                                // Write to file
                                if let Err(e) = std::fs::write(&path_str, content) {
                                    eprintln!("Failed to write to file: {}", e);
                                    
                                    // Show error dialog
                                    let error_dialog = MessageDialog::builder()
                                        .title("Export Error")
                                        .text(&format!("Failed to export transcription: {}", e))
                                        .buttons(ButtonsType::Ok)
                                        .message_type(MessageType::Error)
                                        .transient_for(dialog)
                                        .modal(true)
                                        .build();
                                    
                                    error_dialog.connect_response(|dialog, _| {
                                        dialog.close();
                                    });
                                    
                                    error_dialog.show();
                                } else {
                                    // Show success dialog
                                    let success_dialog = MessageDialog::builder()
                                        .title("Export Successful")
                                        .text(&format!("Transcription exported to: {}", path_str))
                                        .buttons(ButtonsType::Ok)
                                        .message_type(MessageType::Info)
                                        .transient_for(dialog)
                                        .modal(true)
                                        .build();
                                    
                                    success_dialog.connect_response(|dialog, _| {
                                        dialog.close();
                                    });
                                    
                                    success_dialog.show();
                                }
                            }
                        }
                    }
                    dialog.close();
                });

                file_chooser.show();
            }
        }
    }

    /// Get the main container widget
    pub fn get_widget(&self) -> &Box {
        &self.container
    }
}

/// Helper function to format seconds to SRT time format (HH:MM:SS,mmm)
fn format_seconds_to_srt_time(seconds: u32, milliseconds: u32) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    format!("{:02}:{:02}:{:02},{:03}", hours, minutes, secs, milliseconds)
}

/// Helper function to format seconds to VTT time format (HH:MM:SS.mmm)
fn format_seconds_to_vtt_time(seconds: u32, milliseconds: u32) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, secs, milliseconds)
}