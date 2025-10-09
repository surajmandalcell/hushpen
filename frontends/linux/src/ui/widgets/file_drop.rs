//! File drop widget for drag-and-drop functionality
//!
//! This module provides a widget that handles drag-and-drop events for audio files,
//! with visual feedback and file filtering capabilities.

use glib::clone;
use gtk4::prelude::*;
use gtk4::{
    ApplicationWindow, Box, Label, DragSource, DropTarget, EventControllerMotion,
    GestureClick, Picture, Widget, gdk::DragAction, gio::File,
};
use std::path::PathBuf;
use std::sync::Arc;

use crate::models::{AudioFile, AudioFileType};
use crate::utils::AppError;

/// Callback type for when files are dropped
pub type FileDroppedCallback = Arc<dyn Fn(Vec<PathBuf>) + Send + Sync>;

/// File drop widget that handles drag-and-drop events
#[derive(Debug)]
pub struct FileDropWidget {
    /// Main container widget
    container: Box,
    /// Icon widget
    icon: Picture,
    /// Label widget
    label: Label,
    /// Subtitle label widget
    subtitle: Label,
    /// Application window reference
    window: ApplicationWindow,
    /// Callback for when files are dropped
    on_files_dropped: Option<FileDroppedCallback>,
}

impl FileDropWidget {
    /// Create a new FileDropWidget
    pub fn new(window: ApplicationWindow) -> Self {
        // Create the main container
        let container = Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(12)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .css_classes(vec!["file-drop-area".to_string()])
            .build();

        // Create the icon
        let icon = Picture::for_filename("folder-document-symbolic");
        icon.set_css_classes(&["file-drop-icon"]);

        // Create the main label
        let label = Label::builder()
            .label("Drop audio files here")
            .css_classes(vec!["file-drop-label".to_string(), "title".to_string()])
            .build();

        // Create the subtitle label
        let subtitle = Label::builder()
            .label("or click to browse")
            .css_classes(vec!["file-drop-subtitle".to_string(), "subtitle".to_string()])
            .build();

        // Add widgets to container
        container.append(&icon);
        container.append(&label);
        container.append(&subtitle);

        // Create the widget
        let widget = Self {
            container,
            icon,
            label,
            subtitle,
            window,
            on_files_dropped: None,
        };

        // Set up drag-and-drop
        widget.setup_drag_drop();
        widget.setup_click_handler();

        widget
    }

    /// Set the callback for when files are dropped
    pub fn set_files_dropped_callback<F>(&mut self, callback: F)
    where
        F: Fn(Vec<PathBuf>) + Send + Sync + 'static,
    {
        self.on_files_dropped = Some(Arc::new(callback));
    }

    /// Set the text of the main label
    pub fn set_label_text(&self, text: &str) {
        self.label.set_text(text);
    }

    /// Set the text of the subtitle label
    pub fn set_subtitle_text(&self, text: &str) {
        self.subtitle.set_text(text);
    }

    /// Get the container widget
    pub fn get_widget(&self) -> &Box {
        &self.container
    }

    /// Set up drag-and-drop functionality
    fn setup_drag_drop(&self) {
        // Create a drop target for files
        let drop_target = DropTarget::builder()
            .actions(DragAction::Copy)
            .build();

        // Add supported file types
        drop_target.set_gtypes(&[gio::File::static_type()]);

        // Connect to drop signal
        let widget_weak = self.container.downgrade();
        let on_files_dropped = self.on_files_dropped.clone();
        
        drop_target.connect_drop(move |_, value, _, _| {
            if let Some(widget) = widget_weak.upgrade() {
                // Remove drag-over styling
                widget.remove_css_class("drag-over");
                
                // Handle the dropped files
                if let Ok(file) = value.get::<gio::File>() {
                    if let Some(path) = file.path() {
                        if let Some(ref callback) = on_files_dropped {
                            callback(vec![path]);
                        }
                    }
                }
            }
            true
        });

        // Connect to drag-enter signal for visual feedback
        let widget_weak_enter = self.container.downgrade();
        drop_target.connect_enter(move |_, _, _| {
            if let Some(widget) = widget_weak_enter.upgrade() {
                widget.add_css_class("drag-over");
            }
            gdk::DragAction::Copy
        });

        // Connect to drag-leave signal for visual feedback
        let widget_weak_leave = self.container.downgrade();
        drop_target.connect_leave(move |_| {
            if let Some(widget) = widget_weak_leave.upgrade() {
                widget.remove_css_class("drag-over");
            }
        });

        // Add the drop target to the container
        self.container.add_controller(drop_target);

        // Create a motion controller for hover effects
        let motion_controller = EventControllerMotion::new();
        let widget_weak_motion = self.container.downgrade();
        
        motion_controller.connect_enter(move |_| {
            if let Some(widget) = widget_weak_motion.upgrade() {
                widget.add_css_class("hover");
            }
        });

        motion_controller.connect_leave(move |_| {
            if let Some(widget) = widget_weak_motion.upgrade() {
                widget.remove_css_class("hover");
            }
        });

        self.container.add_controller(motion_controller);
    }

    /// Set up click handler for file browsing
    fn setup_click_handler(&self) {
        let gesture = GestureClick::new();
        let window = self.window.clone();
        let on_files_dropped = self.on_files_dropped.clone();

        gesture.connect_pressed(move |_, _, _, _| {
            // Create a file chooser dialog
            let dialog = gtk4::FileChooserNative::builder()
                .title("Select Audio Files")
                .modal(true)
                .transient_for(&window)
                .action(gtk4::FileChooserAction::Open)
                .select_multiple(true)
                .build();

            // Add file filters
            let all_filter = gtk4::FileFilter::new();
            all_filter.set_name(Some("All Supported Audio Files"));
            all_filter.add_mime_type("audio/*");
            dialog.add_filter(&all_filter);

            // Add specific format filters
            let formats = [
                ("MP3 Files", "audio/mpeg", "mp3"),
                ("WAV Files", "audio/wav", "wav"),
                ("FLAC Files", "audio/flac", "flac"),
                ("AAC Files", "audio/aac", "aac"),
                ("OGG Files", "audio/ogg", "ogg"),
                ("WebM Audio", "audio/webm", "webm"),
                ("M4A Files", "audio/mp4", "m4a"),
            ];

            for (name, mime, extension) in formats {
                let filter = gtk4::FileFilter::new();
                filter.set_name(Some(name));
                filter.add_mime_type(mime);
                filter.add_pattern(&format!("*.{extension}"));
                dialog.add_filter(&filter);
            }

            // Connect to response signal
            let callback = on_files_dropped.clone();
            dialog.connect_response(move |dialog, response| {
                if response == gtk4::ResponseType::Accept {
                    if let Some(files) = dialog.files() {
                        let paths: Vec<PathBuf> = files
                            .iter()
                            .filter_map(|file| file.path())
                            .collect();
                        
                        if !paths.is_empty() {
                            if let Some(ref callback) = callback {
                                callback(paths);
                            }
                        }
                    }
                }
                dialog.close();
            });

            dialog.show();
        });

        self.container.add_controller(gesture);
    }

    /// Show loading state
    pub fn show_loading(&self) {
        self.set_label_text("Processing files...");
        self.set_subtitle_text("Please wait while we validate your files");
        self.container.add_css_class("loading");
    }

    /// Show error state
    pub fn show_error(&self, message: &str) {
        self.set_label_text("Error");
        self.set_subtitle_text(message);
        self.container.add_css_class("error");
    }

    /// Reset to normal state
    pub fn reset_state(&self) {
        self.set_label_text("Drop audio files here");
        self.set_subtitle_text("or click to browse");
        self.container.remove_css_class("loading");
        self.container.remove_css_class("error");
    }

    /// Validate if a file is a supported audio file
    pub fn validate_file(path: &PathBuf) -> Result<AudioFileType, AppError> {
        // Check if the file exists
        if !path.exists() {
            return Err(AppError::file("File does not exist"));
        }

        // Check if it's a file (not a directory)
        if !path.is_file() {
            return Err(AppError::file("Path is not a file"));
        }

        // Get the file extension
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| AppError::file("No file extension"))?;

        // Check if the extension is supported
        let file_type = AudioFileType::from_extension(extension);
        if !file_type.is_supported() {
            return Err(AppError::file(format!(
                "Unsupported file format: {}",
                extension
            )));
        }

        Ok(file_type)
    }

    /// Filter a list of files to only include supported audio files
    pub fn filter_supported_files(paths: Vec<PathBuf>) -> (Vec<PathBuf>, Vec<(PathBuf, AppError)>) {
        let mut supported_files = Vec::new();
        let mut unsupported_files = Vec::new();

        for path in paths {
            match Self::validate_file(&path) {
                Ok(_) => supported_files.push(path),
                Err(e) => unsupported_files.push((path, e)),
            }
        }

        (supported_files, unsupported_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_file_validation() {
        let temp_dir = tempdir().unwrap();
        
        // Test with a valid audio file
        let valid_file = temp_dir.path().join("test.mp3");
        fs::write(&valid_file, b"fake audio data").unwrap();
        
        let result = FileDropWidget::validate_file(&valid_file);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), AudioFileType::Mp3);
        
        // Test with an invalid extension
        let invalid_file = temp_dir.path().join("test.txt");
        fs::write(&invalid_file, b"not audio").unwrap();
        
        let result = FileDropWidget::validate_file(&invalid_file);
        assert!(result.is_err());
        
        // Test with a directory
        let result = FileDropWidget::validate_file(&temp_dir.path().to_path_buf());
        assert!(result.is_err());
        
        // Test with a non-existent file
        let nonexistent_file = temp_dir.path().join("nonexistent.mp3");
        let result = FileDropWidget::validate_file(&nonexistent_file);
        assert!(result.is_err());
    }

    #[test]
    fn test_file_filtering() {
        let temp_dir = tempdir().unwrap();
        
        // Create test files
        let mp3_file = temp_dir.path().join("test.mp3");
        fs::write(&mp3_file, b"fake mp3 data").unwrap();
        
        let wav_file = temp_dir.path().join("test.wav");
        fs::write(&wav_file, b"fake wav data").unwrap();
        
        let txt_file = temp_dir.path().join("test.txt");
        fs::write(&txt_file, b"not audio").unwrap();
        
        let files = vec![mp3_file, wav_file, txt_file];
        let (supported, unsupported) = FileDropWidget::filter_supported_files(files);
        
        assert_eq!(supported.len(), 2);
        assert_eq!(unsupported.len(), 1);
    }
}