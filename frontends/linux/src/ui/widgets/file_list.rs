//! File list widget for displaying added files
//!
//! This module provides a widget to display a list of audio files with their metadata,
//! status, and progress indicators. It supports selection, context menus, and sorting.

use glib::clone;
use gtk4::prelude::*;
use gtk4::{
    Adjustment, Align, Box, Button, ColumnView, ColumnViewColumn, EventControllerKey,
    GestureClick, Label, ListView, Menu, MenuItem, NoSelection, PopoverMenu, ProgressBar,
    ScrolledWindow, SignalListItemFactory, SingleSelection, StringObject, Widget,
};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::models::{AudioFile, FileStatus};

/// Callback type for file selection changes
pub type FileSelectionCallback = Arc<dyn Fn(Option<&AudioFile>) + Send + Sync>;

/// Callback type for file removal requests
pub type FileRemovalCallback = Arc<dyn Fn(&Uuid) + Send + Sync>;

/// Callback type for file clearing requests
pub type FileClearCallback = Arc<dyn Fn() + Send + Sync>;

/// File list widget that displays added files with metadata
#[derive(Debug)]
pub struct FileListWidget {
    /// Main container widget
    container: Box,
    /// List view for files
    list_view: ListView,
    /// Scrolled window containing the list
    scrolled_window: ScrolledWindow,
    /// Header with title and actions
    header: Box,
    /// Clear all button
    clear_button: Button,
    /// Files displayed in the list
    files: Arc<gtk4::gio::ListStore>,
    /// Selection model
    selection_model: SingleSelection,
    /// Callback for selection changes
    on_selection_changed: Option<FileSelectionCallback>,
    /// Callback for file removal
    on_file_removed: Option<FileRemovalCallback>,
    /// Callback for clearing all files
    on_files_cleared: Option<FileClearCallback>,
    /// Progress tracking for uploads
    upload_progress: Arc<std::sync::RwLock<HashMap<Uuid, f32>>>,
}

impl FileListWidget {
    /// Create a new FileListWidget
    pub fn new() -> Self {
        // Create the main container
        let container = Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(0)
            .css_classes(vec!["file-list-container".to_string()])
            .build();

        // Create the header
        let header = Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(12)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        // Create the title label
        let title_label = Label::builder()
            .label("Files")
            .css_classes(vec!["file-list-title".to_string(), "title".to_string()])
            .halign(Align::Start)
            .hexpand(true)
            .build();

        // Create the clear all button
        let clear_button = Button::builder()
            .label("Clear All")
            .css_classes(vec!["clear-all-button".to_string(), "destructive-action".to_string()])
            .halign(Align::End)
            .build();

        // Add widgets to header
        header.append(&title_label);
        header.append(&clear_button);

        // Create the list store for files
        let files = gtk4::gio::ListStore::new::<AudioFileObject>();

        // Create the selection model
        let selection_model = SingleSelection::new(Some(&NoSelection::new(Some(&files))));
        selection_model.set_can_unselect(true);
        selection_model.set_autoselect(false);

        // Create the factory for list items
        let factory = SignalListItemFactory::new();
        
        // Setup the factory
        factory.connect_setup(move |_, list_item| {
            let file_item = FileListItem::new();
            list_item.set_child(Some(&file_item.get_widget()));
        });

        factory.connect_bind(move |_, list_item| {
            if let Some(file_item) = list_item.child().and_downcast::<FileListItem>() {
                if let Some(file_object) = list_item.item().and_downcast::<AudioFileObject>() {
                    file_item.bind(&file_object);
                }
            }
        });

        factory.connect_unbind(move |_, list_item| {
            if let Some(file_item) = list_item.child().and_downcast::<FileListItem>() {
                file_item.unbind();
            }
        });

        // Create the list view
        let list_view = ListView::builder()
            .model(&selection_model)
            .factory(&factory)
            .css_classes(vec!["file-list".to_string()])
            .build();

        // Create the scrolled window
        let scrolled_window = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .min_content_height(200)
            .css_classes(vec!["file-list-scrolled".to_string()])
            .build();

        scrolled_window.set_child(Some(&list_view));
        scrolled_window.set_vadjustment(Some(&Adjustment::new(0.0, 0.0, 100.0, 1.0, 10.0, 10.0)));

        // Add widgets to container
        container.append(&header);
        container.append(&scrolled_window);

        // Create the widget
        let widget = Self {
            container,
            list_view,
            scrolled_window,
            header,
            clear_button,
            files: Arc::new(files),
            selection_model,
            on_selection_changed: None,
            on_file_removed: None,
            on_files_cleared: None,
            upload_progress: Arc::new(std::sync::RwLock::new(HashMap::new())),
        };

        // Set up event handlers
        widget.setup_event_handlers();

        widget
    }

    /// Set the callback for selection changes
    pub fn set_selection_changed_callback<F>(&mut self, callback: F)
    where
        F: Fn(Option<&AudioFile>) + Send + Sync + 'static,
    {
        self.on_selection_changed = Some(Arc::new(callback));
    }

    /// Set the callback for file removal
    pub fn set_file_removed_callback<F>(&mut self, callback: F)
    where
        F: Fn(&Uuid) + Send + Sync + 'static,
    {
        self.on_file_removed = Some(Arc::new(callback));
    }

    /// Set the callback for clearing all files
    pub fn set_files_cleared_callback<F>(&mut self, callback: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_files_cleared = Some(Arc::new(callback));
    }

    /// Get the container widget
    pub fn get_widget(&self) -> &Box {
        &self.container
    }

    /// Add a file to the list
    pub fn add_file(&self, file: AudioFile) {
        let file_object = AudioFileObject::new(file);
        self.files.append(&file_object);
        self.update_clear_button_state();
    }

    /// Remove a file from the list
    pub fn remove_file(&self, file_id: &Uuid) {
        let mut index_to_remove = None;
        
        for (i, obj) in self.files.iter::<AudioFileObject>().enumerate() {
            if let Ok(file_object) = obj {
                if file_object.get_file().id == *file_id {
                    index_to_remove = Some(i);
                    break;
                }
            }
        }
        
        if let Some(index) = index_to_remove {
            self.files.remove(index);
            self.update_clear_button_state();
        }
    }

    /// Clear all files from the list
    pub fn clear_files(&self) {
        self.files.remove_all();
        self.update_clear_button_state();
    }

    /// Get the number of files in the list
    pub fn get_file_count(&self) -> u32 {
        self.files.n_items()
    }

    /// Get the selected file
    pub fn get_selected_file(&self) -> Option<AudioFile> {
        let selected = self.selection_model.selected();
        if selected != gtk4::INVALID_LIST_POSITION {
            if let Some(obj) = self.files.item(selected) {
                if let Ok(file_object) = obj.downcast::<AudioFileObject>() {
                    return Some(file_object.get_file());
                }
            }
        }
        None
    }

    /// Update the progress for a file
    pub fn update_file_progress(&self, file_id: &Uuid, progress: f32) {
        // Store the progress
        {
            let mut progress_map = self.upload_progress.write().unwrap();
            progress_map.insert(*file_id, progress);
        }
        
        // Find and update the file in the list
        for obj in self.files.iter::<AudioFileObject>() {
            if let Ok(file_object) = obj {
                if file_object.get_file().id == *file_id {
                    file_object.update_progress(progress);
                    break;
                }
            }
        }
    }

    /// Update the status of a file
    pub fn update_file_status(&self, file_id: &Uuid, status: FileStatus, error_message: Option<String>) {
        for obj in self.files.iter::<AudioFileObject>() {
            if let Ok(file_object) = obj {
                if file_object.get_file().id == *file_id {
                    file_object.update_status(status, error_message);
                    break;
                }
            }
        }
    }

    /// Set up event handlers
    fn setup_event_handlers(&self) {
        // Handle selection changes
        let files = self.files.clone();
        let on_selection_changed = self.on_selection_changed.clone();
        
        self.selection_model.connect_selection_changed(move |model, _, _| {
            let selected = model.selected();
            if let Some(ref callback) = on_selection_changed {
                if selected != gtk4::INVALID_LIST_POSITION {
                    if let Some(obj) = files.item(selected) {
                        if let Ok(file_object) = obj.downcast::<AudioFileObject>() {
                            callback(Some(&file_object.get_file()));
                        }
                    }
                } else {
                    callback(None);
                }
            }
        });

        // Handle clear all button
        let files = self.files.clone();
        let on_files_cleared = self.on_files_cleared.clone();
        let clear_button = self.clear_button.clone();
        
        self.clear_button.connect_clicked(move |_| {
            if let Some(ref callback) = on_files_cleared {
                callback();
            }
        });

        // Handle right-click context menu
        let gesture = GestureClick::builder()
            .button(gdk::BUTTON_SECONDARY)
            .build();
        
        let list_view = self.list_view.clone();
        let files = self.files.clone();
        let on_file_removed = self.on_file_removed.clone();
        
        gesture.connect_pressed(move |_, _, x, y| {
            // Get the item at the click position
            if let Some(list_item) = list_view.pick_widget(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)) {
                // Find the parent list item if we got a child widget
                let mut current = Some(list_item.clone());
                while let Some(widget) = current {
                    if widget.is::<gtk4::ListItem>() {
                        // Found the list item, get its file
                        if let Some(list_item) = widget.downcast_ref::<gtk4::ListItem>() {
                            if let Some(obj) = list_item.item() {
                                if let Ok(file_object) = obj.downcast::<AudioFileObject>() {
                                    let file_id = file_object.get_file().id;
                                    
                                    // Create context menu
                                    let menu = Menu::new();
                                    let remove_item = MenuItem::builder()
                                        .label("Remove")
                                        .build();
                                    
                                    menu.append(&remove_item);
                                    
                                    // Handle remove action
                                    let file_id = file_id.clone();
                                    let on_file_removed = on_file_removed.clone();
                                    remove_item.connect_activate(move |_| {
                                        if let Some(ref callback) = on_file_removed {
                                            callback(&file_id);
                                        }
                                    });
                                    
                                    // Show the menu
                                    let popover = PopoverMenu::builder()
                                        .menu_model(&menu)
                                        .has_arrow(false)
                                        .build();
                                    popover.set_parent(&list_item);
                                    popover.set_pointing_to(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1));
                                    popover.popup();
                                }
                            }
                        }
                        break;
                    }
                    current = widget.parent();
                }
            }
        });
        
        self.list_view.add_controller(gesture);
    }

    /// Update the clear button state based on whether there are files
    fn update_clear_button_state(&self) {
        let has_files = self.files.n_items() > 0;
        self.clear_button.set_sensitive(has_files);
    }
}

/// Wrapper object for AudioFile to work with GTK list models
#[derive(Debug)]
pub struct AudioFileObject {
    file: AudioFile,
    progress: f32,
}

impl AudioFileObject {
    pub fn new(file: AudioFile) -> Self {
        Self {
            file,
            progress: 0.0,
        }
    }

    pub fn get_file(&self) -> AudioFile {
        self.file.clone()
    }

    pub fn update_progress(&self, progress: f32) {
        // This would need to be implemented with properties for GTK to update
        // For simplicity, we'll just store the value
    }

    pub fn update_status(&self, status: FileStatus, error_message: Option<String>) {
        // This would need to be implemented with properties for GTK to update
        // For simplicity, we'll just store the value
    }
}

#[glib::object_subclass]
impl ObjectSubclass for AudioFileObject {
    const NAME: &'static str = "AudioFileObject";
    type Type = super::AudioFileObject;
    type ParentType = glib::Object;
}

impl ObjectImpl for AudioFileObject {}

/// Individual list item for displaying a file
#[derive(Debug)]
pub struct FileListItem {
    container: Box,
    file_name_label: Label,
    file_size_label: Label,
    duration_label: Label,
    status_label: Label,
    progress_bar: ProgressBar,
    remove_button: Button,
}

impl FileListItem {
    pub fn new() -> Self {
        // Create the main container
        let container = Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(12)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(12)
            .margin_end(12)
            .css_classes(vec!["file-list-item".to_string()])
            .build();

        // Create file info container
        let info_container = Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(4)
            .hexpand(true)
            .build();

        // Create file name label
        let file_name_label = Label::builder()
            .label("")
            .halign(Align::Start)
            .css_classes(vec!["file-name".to_string()])
            .build();

        // Create file details container
        let details_container = Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(12)
            .build();

        // Create file size label
        let file_size_label = Label::builder()
            .label("")
            .halign(Align::Start)
            .css_classes(vec!["file-size".to_string(), "subtitle".to_string()])
            .build();

        // Create duration label
        let duration_label = Label::builder()
            .label("")
            .halign(Align::Start)
            .css_classes(vec!["file-duration".to_string(), "subtitle".to_string()])
            .build();

        // Create status label
        let status_label = Label::builder()
            .label("")
            .halign(Align::Start)
            .css_classes(vec!["file-status".to_string(), "subtitle".to_string()])
            .build();

        // Add details to details container
        details_container.append(&file_size_label);
        details_container.append(&duration_label);
        details_container.append(&status_label);

        // Add widgets to info container
        info_container.append(&file_name_label);
        info_container.append(&details_container);

        // Create progress bar
        let progress_bar = ProgressBar::builder()
            .hexpand(true)
            .visible(false)
            .css_classes(vec!["file-progress".to_string()])
            .build();

        // Create remove button
        let remove_button = Button::builder()
            .icon_name("window-close-symbolic")
            .css_classes(vec!["file-remove-button".to_string(), "flat".to_string()])
            .tooltip_text("Remove file")
            .build();

        // Add widgets to main container
        container.append(&info_container);
        container.append(&progress_bar);
        container.append(&remove_button);

        Self {
            container,
            file_name_label,
            file_size_label,
            duration_label,
            status_label,
            progress_bar,
            remove_button,
        }
    }

    pub fn get_widget(&self) -> &Box {
        &self.container
    }

    pub fn bind(&self, file_object: &AudioFileObject) {
        let file = file_object.get_file();
        
        // Set file name
        self.file_name_label.set_text(&file.file_name);
        
        // Set file size
        self.file_size_label.set_text(&file.formatted_file_size());
        
        // Set duration
        self.duration_label.set_text(&file.formatted_duration());
        
        // Set status
        self.status_label.set_text(&file.status_message());
        
        // Update status styling
        self.update_status_styling(&file.status);
        
        // Show progress bar if uploading
        let show_progress = matches!(file.status, FileStatus::Uploading);
        self.progress_bar.set_visible(show_progress);
        
        if show_progress {
            self.progress_bar.set_fraction(0.0);
        }
    }

    pub fn unbind(&self) {
        // Clear all labels
        self.file_name_label.set_text("");
        self.file_size_label.set_text("");
        self.duration_label.set_text("");
        self.status_label.set_text("");
        self.progress_bar.set_visible(false);
    }

    fn update_status_styling(&self, status: &FileStatus) {
        // Remove existing status classes
        self.status_label.remove_css_class("status-ready");
        self.status_label.remove_css_class("status-processing");
        self.status_label.remove_css_class("status-failed");
        self.status_label.remove_css_class("status-uploading");
        self.status_label.remove_css_class("status-downloading");
        
        // Add appropriate status class
        match status {
            FileStatus::Ready => self.status_label.add_css_class("status-ready"),
            FileStatus::Processing => self.status_label.add_css_class("status-processing"),
            FileStatus::Failed => self.status_label.add_css_class("status-failed"),
            FileStatus::Uploading => self.status_label.add_css_class("status-uploading"),
            FileStatus::Downloading => self.status_label.add_css_class("status-downloading"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn test_file_list_creation() {
        let file_list = FileListWidget::new();
        assert_eq!(file_list.get_file_count(), 0);
    }

    #[test]
    fn test_add_remove_files() {
        let file_list = FileListWidget::new();
        
        // Create a test file
        let file_path = PathBuf::from("/test/path.mp3");
        let mut file = AudioFile::new(file_path);
        file.status = FileStatus::Ready;
        
        // Add file
        file_list.add_file(file.clone());
        assert_eq!(file_list.get_file_count(), 1);
        
        // Remove file
        file_list.remove_file(&file.id);
        assert_eq!(file_list.get_file_count(), 0);
    }

    #[test]
    fn test_clear_files() {
        let file_list = FileListWidget::new();
        
        // Create test files
        let file1 = AudioFile::new(PathBuf::from("/test/path1.mp3"));
        let file2 = AudioFile::new(PathBuf::from("/test/path2.wav"));
        
        // Add files
        file_list.add_file(file1);
        file_list.add_file(file2);
        assert_eq!(file_list.get_file_count(), 2);
        
        // Clear all files
        file_list.clear_files();
        assert_eq!(file_list.get_file_count(), 0);
    }
}