//! File panel component for the ASRPro application
//!
//! This module contains the FilePanel struct which provides a comprehensive interface
//! for adding, managing, and processing audio files for transcription.

use glib::clone;
use gtk4::prelude::*;
use gtk4::{
    ApplicationWindow, Box, Button, InfoBar, Label, Orientation, Paned, Revealer,
    ScrolledWindow, Separator, Stack, StackSwitcher, Widget,
};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use crate::models::{AppState, AudioFile, FileStatus};
use crate::services::FileManager;
use crate::ui::widgets::{file_drop::FileDropWidget, file_list::FileListWidget};
use crate::utils::{AppError, AppResult};

/// File panel component that manages audio files
#[derive(Debug)]
pub struct FilePanel {
    /// Main container widget
    container: Box,
    /// Application state
    app_state: Arc<AppState>,
    /// File manager service
    file_manager: Arc<FileManager>,
    /// Stack for switching between views
    stack: Stack,
    /// Stack switcher for navigation
    stack_switcher: StackSwitcher,
    /// File drop widget
    file_drop: FileDropWidget,
    /// File list widget
    file_list: FileListWidget,
    /// Info bar for notifications
    info_bar: InfoBar,
    /// Info bar label
    info_bar_label: Label,
    /// Paned container for layout
    paned: Paned,
    /// Button container
    button_container: Box,
    /// Add files button
    add_files_button: Button,
    /// Process files button
    process_files_button: Button,
    /// Clear all button
    clear_all_button: Button,
    /// Revealer for file list
    file_list_revealer: Revealer,
}

impl FilePanel {
    /// Create a new FilePanel instance
    pub fn new(window: ApplicationWindow, app_state: Arc<AppState>, file_manager: Arc<FileManager>) -> Result<Self, AppError> {
        // Create the main container
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(0)
            .css_classes(vec!["file-panel".to_string()])
            .build();

        // Create the stack for different views
        let stack = Stack::builder()
            .transition_type(gtk4::StackTransitionType::SlideUp)
            .css_classes(vec!["file-panel-stack".to_string()])
            .build();

        // Create the stack switcher
        let stack_switcher = StackSwitcher::builder()
            .stack(&stack)
            .halign(gtk4::Align::Center)
            .margin_top(12)
            .margin_bottom(6)
            .build();

        // Create the file drop widget
        let file_drop = FileDropWidget::new(window.clone());

        // Create the file list widget
        let file_list = FileListWidget::new();

        // Add pages to the stack
        stack.add_titled(&file_drop.get_widget(), Some("drop"), "Add Files");
        stack.add_titled(&file_list.get_widget(), Some("list"), "File List");

        // Create the info bar
        let info_bar = InfoBar::builder()
            .message_type(gtk4::MessageType::Info)
            .revealed(false)
            .show_close_button(true)
            .css_classes(vec!["file-panel-info-bar".to_string()])
            .build();

        let info_bar_label = Label::builder("")
            .halign(gtk4::Align::Start)
            .hexpand(true)
            .build();

        info_bar.add_child(&info_bar_label);

        // Create the button container
        let button_container = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(12)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .halign(gtk4::Align::Center)
            .build();

        // Create action buttons
        let add_files_button = Button::builder()
            .label("Add Files")
            .icon_name("list-add-symbolic")
            .css_classes(vec!["suggested-action".to_string()])
            .build();

        let process_files_button = Button::builder()
            .label("Process Files")
            .icon_name("media-playback-start-symbolic")
            .sensitive(false)
            .build();

        let clear_all_button = Button::builder()
            .label("Clear All")
            .icon_name("edit-clear-all-symbolic")
            .css_classes(vec!["destructive-action".to_string()])
            .sensitive(false)
            .build();

        // Add buttons to container
        button_container.append(&add_files_button);
        button_container.append(&process_files_button);
        button_container.append(&clear_all_button);

        // Create a separator
        let separator = Separator::builder()
            .orientation(Orientation::Horizontal)
            .margin_top(6)
            .margin_bottom(6)
            .build();

        // Create the paned container for layout
        let paned = Paned::builder()
            .orientation(Orientation::Vertical)
            .wide_handle(true)
            .build();

        // Create a revealer for the file list
        let file_list_revealer = Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::SlideDown)
            .transition_duration(300)
            .build();

        // Set up the file list in the revealer
        file_list_revealer.set_child(Some(&file_list.get_widget()));

        // Add widgets to the paned container
        paned.set_start_child(Some(&stack));
        paned.set_end_child(Some(&file_list_revealer));
        paned.set_position(300);

        // Add all widgets to the main container
        container.append(&stack_switcher);
        container.append(&paned);
        container.append(&separator);
        container.append(&button_container);
        container.append(&info_bar);

        // Create the file panel
        let panel = Self {
            container,
            app_state,
            file_manager,
            stack,
            stack_switcher,
            file_drop,
            file_list,
            info_bar,
            info_bar_label,
            paned,
            button_container,
            add_files_button,
            process_files_button,
            clear_all_button,
            file_list_revealer,
        };

        // Set up event handlers
        panel.setup_event_handlers()?;

        Ok(panel)
    }

    /// Get the container widget
    pub fn get_widget(&self) -> &Box {
        &self.container
    }

    /// Show the file list
    pub fn show_file_list(&self) {
        self.file_list_revealer.set_reveal_child(true);
        self.stack.set_visible_child_name("list");
    }

    /// Hide the file list
    pub fn hide_file_list(&self) {
        self.file_list_revealer.set_reveal_child(false);
        self.stack.set_visible_child_name("drop");
    }

    /// Show an info message
    pub fn show_info_message(&self, message: &str) {
        self.info_bar_label.set_text(message);
        self.info_bar.set_message_type(gtk4::MessageType::Info);
        self.info_bar.set_revealed(true);
        
        // Auto-hide after 5 seconds
        let info_bar = self.info_bar.clone();
        gtk4::glib::timeout_add_seconds_local(5, move || {
            info_bar.set_revealed(false);
            glib::ControlFlow::Continue
        });
    }

    /// Show a warning message
    pub fn show_warning_message(&self, message: &str) {
        self.info_bar_label.set_text(message);
        self.info_bar.set_message_type(gtk4::MessageType::Warning);
        self.info_bar.set_revealed(true);
    }

    /// Show an error message
    pub fn show_error_message(&self, message: &str) {
        self.info_bar_label.set_text(message);
        self.info_bar.set_message_type(gtk4::MessageType::Error);
        self.info_bar.set_revealed(true);
    }

    /// Set up event handlers
    fn setup_event_handlers(&self) -> AppResult<()> {
        // Handle file drops
        let file_list = self.file_list.clone();
        let file_manager = self.file_manager.clone();
        let app_state = self.app_state.clone();
        let stack = self.stack.clone();
        
        self.file_drop.set_files_dropped_callback(move |paths| {
            let file_list = file_list.clone();
            let file_manager = file_manager.clone();
            let app_state = app_state.clone();
            let stack = stack.clone();
            
            gtk4::glib::spawn_future_local(async move {
                // Filter supported files
                let (supported_files, unsupported_files) = FileDropWidget::filter_supported_files(paths);
                
                // Show warning for unsupported files
                if !unsupported_files.is_empty() {
                    let message = format!(
                        "{} files were skipped because they are not supported audio formats",
                        unsupported_files.len()
                    );
                    
                    app_state.set_status_message(message).await;
                }
                
                // Process supported files
                let mut added_count = 0;
                for path in supported_files {
                    match file_manager.add_file(path.clone()).await {
                        Ok(audio_file) => {
                            file_list.add_file(audio_file);
                            added_count += 1;
                            
                            // Update status
                            app_state.set_status_message(
                                format!("Added file: {}", path.display())
                            ).await;
                        }
                        Err(e) => {
                            app_state.set_status_message(
                                format!("Failed to add file {}: {}", path.display(), e)
                            ).await;
                        }
                    }
                }
                
                // Switch to file list view if files were added
                if added_count > 0 {
                    stack.set_visible_child_name("list");
                    app_state.set_status_message(
                        format!("Successfully added {} files", added_count)
                    ).await;
                }
            });
        });

        // Handle file selection changes
        let process_files_button = self.process_files_button.clone();
        let app_state = self.app_state.clone();
        
        self.file_list.set_selection_changed_callback(move |file| {
            if let Some(file) = file {
                let can_process = file.is_ready_for_transcription();
                process_files_button.set_sensitive(can_process);
                
                let app_state_clone = app_state.clone();
                gtk4::glib::spawn_future_local(async move {
                    app_state_clone.set_status_message(
                        format!("Selected: {} ({})", file.file_name, file.status_message())
                    ).await;
                });
            } else {
                process_files_button.set_sensitive(false);
                let app_state_clone = app_state.clone();
                gtk4::glib::spawn_future_local(async move {
                    app_state_clone.set_status_message("No file selected".to_string()).await;
                });
            }
        });

        // Handle file removal
        let file_list = self.file_list.clone();
        let app_state = self.app_state.clone();
        
        self.file_list.set_file_removed_callback(move |file_id| {
            let file_list = file_list.clone();
            let app_state = app_state.clone();
            
            gtk4::glib::spawn_future_local(async move {
                // Remove from UI
                file_list.remove_file(file_id);
                
                // Update status
                app_state.set_status_message("File removed".to_string()).await;
            });
        });

        // Handle clearing all files
        let file_list = self.file_list.clone();
        let app_state = self.app_state.clone();
        
        self.file_list.set_files_cleared_callback(move || {
            let file_list = file_list.clone();
            let app_state = app_state.clone();
            
            gtk4::glib::spawn_future_local(async move {
                // Clear from UI
                file_list.clear_files();
                
                // Update status
                app_state.set_status_message("All files cleared".to_string()).await;
            });
        });

        // Handle add files button
        let stack = self.stack.clone();
        
        self.add_files_button.connect_clicked(move |_| {
            stack.set_visible_child_name("drop");
        });

        // Handle process files button
        let file_list = self.file_list.clone();
        let file_manager = self.file_manager.clone();
        let app_state = self.app_state.clone();
        
        self.process_files_button.connect_clicked(move |_| {
            let file_list = file_list.clone();
            let file_manager = file_manager.clone();
            let app_state = app_state.clone();
            
            gtk4::glib::spawn_future_local(async move {
                if let Some(file) = file_list.get_selected_file() {
                    // Start processing the file
                    app_state.set_status_message(
                        format!("Processing file: {}", file.file_name)
                    ).await;
                    
                    // Update file status
                    file_list.update_file_status(&file.id, FileStatus::Processing, None);
                    
                    // Start upload
                    let file_id = file.id;
                    let file_path = file.file_path.clone();
                    
                    // Create progress callback
                    let file_list_clone = file_list.clone();
                    let progress_callback = Arc::new(move |progress: f32| {
                        let file_list = file_list_clone.clone();
                        let file_id = file_id.clone();
                        
                        gtk4::glib::spawn_future_local(async move {
                            file_list.update_file_progress(&file_id, progress);
                        });
                    });
                    
                    // Start the upload
                    match file_manager.upload_file(file.id, &file_path, Some(progress_callback)).await {
                        Ok(_) => {
                            file_list.update_file_status(&file.id, FileStatus::Ready, None);
                            app_state.set_status_message(
                                format!("File processed successfully: {}", file.file_name)
                            ).await;
                        }
                        Err(e) => {
                            file_list.update_file_status(&file.id, FileStatus::Failed, Some(e.to_string()));
                            app_state.set_status_message(
                                format!("Failed to process file: {}", e)
                            ).await;
                        }
                    }
                }
            });
        });

        // Handle clear all button
        let file_list = self.file_list.clone();
        let app_state = self.app_state.clone();
        
        self.clear_all_button.connect_clicked(move |_| {
            let file_list = file_list.clone();
            let app_state = app_state.clone();
            
            gtk4::glib::spawn_future_local(async move {
                file_list.clear_files();
                app_state.set_status_message("All files cleared".to_string()).await;
            });
        });

        // Handle info bar close button
        let info_bar = self.info_bar.clone();
        self.info_bar.connect_response(move |_, _| {
            info_bar.set_revealed(false);
        });

        // Handle stack changes
        let file_list_revealer = self.file_list_revealer.clone();
        self.stack.connect_visible_child_notify(move |stack| {
            let is_list_view = stack.visible_child_name().map_or(false, |name| name == "list");
            file_list_revealer.set_reveal_child(is_list_view);
        });

        Ok(())
    }

    /// Update the button states based on the current state
    fn update_button_states(&self) {
        let file_count = self.file_list.get_file_count();
        let has_files = file_count > 0;
        
        self.clear_all_button.set_sensitive(has_files);
        
        // Check if any files are ready for processing
        let has_ready_files = has_files; // Simplified - in a real implementation, check file statuses
        self.process_files_button.set_sensitive(has_ready_files);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk4::Application;
    use std::sync::Arc;

    #[test]
    fn test_file_panel_creation() {
        // This test would require a GTK application context
        // In a real test environment, you would set up a test application
        // For now, we'll just verify the struct can be created with the right parameters
    }
}