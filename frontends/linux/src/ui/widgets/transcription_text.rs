//! Transcription Text Widget
//!
//! This module provides a widget for displaying and editing transcribed text
//! with syntax highlighting for timestamps, speakers, and other elements.

use glib::clone;
use gtk4::prelude::*;
use gtk4::{
    TextBuffer, TextView, TextTag, TextTagTable, ScrolledWindow, SearchEntry,
    SearchBar, Popover, Box, Button, Label, CheckButton, Orientation, Align,
    PopoverMenu, MenuButton, PositionType, Adjustment, Scale, EventControllerKey,
    Dialog, ResponseType, MessageDialog, DialogFlags, ButtonsType, MessageType,
    ApplicationWindow, Widget
};
use gtk4::glib::{Propagation, ControlFlow};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::models::transcription::{TranscriptionResult, TranscriptionSegment};
use crate::utils::AppError;

/// View mode for the transcription text
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TranscriptionViewMode {
    /// Plain text without any formatting
    PlainText,
    /// Text with timestamps
    WithTimestamps,
    /// Text with speaker labels
    WithSpeakers,
    /// Text with both timestamps and speakers
    Full,
}

/// Transcription text widget for displaying and editing transcribed content
pub struct TranscriptionTextWidget {
    /// Main container
    container: Box,
    /// Text view for displaying content
    text_view: TextView,
    /// Text buffer for the text view
    text_buffer: TextBuffer,
    /// Scrolled window containing the text view
    scrolled_window: ScrolledWindow,
    /// Search bar for find functionality
    search_bar: SearchBar,
    /// Search entry for find functionality
    search_entry: SearchEntry,
    /// Search settings popover
    search_popover: Popover,
    /// Zoom adjustment for text size
    zoom_adjustment: Adjustment,
    /// Current view mode
    view_mode: Arc<Mutex<TranscriptionViewMode>>,
    /// Current transcription result
    current_result: Arc<Mutex<Option<TranscriptionResult>>>,
    /// Text tags for syntax highlighting
    text_tags: Arc<Mutex<HashMap<String, TextTag>>>,
    /// Find/replace dialog
    find_replace_dialog: Arc<Mutex<Option<Dialog>>>,
}

impl TranscriptionTextWidget {
    /// Create a new transcription text widget
    pub fn new() -> Result<Self, AppError> {
        // Create the main container
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(0)
            .build();

        // Create text tag table and buffer
        let tag_table = TextTagTable::new();
        let text_buffer = TextBuffer::builder()
            .tag_table(&tag_table)
            .build();

        // Create the text view
        let text_view = TextView::builder()
            .buffer(&text_buffer)
            .wrap_mode(gtk4::WrapMode::Word)
            .left_margin(10)
            .right_margin(10)
            .top_margin(10)
            .bottom_margin(10)
            .editable(true)
            .cursor_visible(true)
            .build();

        // Create scrolled window
        let scrolled_window = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Always)
            .min_content_width(400)
            .min_content_height(300)
            .build();
        scrolled_window.set_child(Some(&text_view));

        // Create search bar
        let search_bar = SearchBar::new();
        
        // Create search entry
        let search_entry = SearchEntry::builder()
            .placeholder_text("Find in transcription...")
            .build();
        search_bar.set_child(Some(&search_entry));

        // Create zoom adjustment
        let zoom_adjustment = Adjustment::new(1.0, 0.5, 3.0, 0.1, 0.1, 0.0);

        // Create the widget
        let mut widget = Self {
            container,
            text_view,
            text_buffer,
            scrolled_window,
            search_bar,
            search_entry,
            search_popover: Popover::new(),
            zoom_adjustment,
            view_mode: Arc::new(Mutex::new(TranscriptionViewMode::Full)),
            current_result: Arc::new(Mutex::new(None)),
            text_tags: Arc::new(Mutex::new(HashMap::new())),
            find_replace_dialog: Arc::new(Mutex::new(None)),
        };

        // Set up the widget
        widget.setup_tags()?;
        widget.setup_search()?;
        widget.setup_zoom()?;
        widget.setup_events()?;

        // Layout the components
        widget.layout_components();

        Ok(widget)
    }

    /// Set up text tags for syntax highlighting
    fn setup_tags(&mut self) -> Result<(), AppError> {
        let mut tags = self.text_tags.lock().unwrap();
        let tag_table = self.text_buffer.tag_table().unwrap();

        // Create tag for timestamps
        let timestamp_tag = TextTag::builder()
            .name("timestamp")
            .foreground("#1976d2")
            .weight(600)
            .build();
        tag_table.add(&timestamp_tag);
        tags.insert("timestamp".to_string(), timestamp_tag);

        // Create tag for speaker labels
        let speaker_tag = TextTag::builder()
            .name("speaker")
            .foreground("#d32f2f")
            .weight(600)
            .build();
        tag_table.add(&speaker_tag);
        tags.insert("speaker".to_string(), speaker_tag);

        // Create tag for low confidence text
        let low_confidence_tag = TextTag::builder()
            .name("low_confidence")
            .foreground("#ff9800")
            .style(gtk4::pango::Style::Italic)
            .build();
        tag_table.add(&low_confidence_tag);
        tags.insert("low_confidence".to_string(), low_confidence_tag);

        // Create tag for highlighted search results
        let search_highlight_tag = TextTag::builder()
            .name("search_highlight")
            .background("#ffeb3b")
            .foreground("#000000")
            .build();
        tag_table.add(&search_highlight_tag);
        tags.insert("search_highlight".to_string(), search_highlight_tag);

        Ok(())
    }

    /// Set up search functionality
    fn setup_search(&mut self) -> Result<(), AppError> {
        // Connect search entry changes
        let search_entry = self.search_entry.clone();
        let text_buffer = self.text_buffer.clone();
        let text_tags = Arc::clone(&self.text_tags);
        
        search_entry.connect_search_changed(move |entry| {
            let search_text = entry.text().to_string();
            if search_text.is_empty() {
                // Clear previous highlights
                let tags = text_tags.lock().unwrap();
                if let Some(tag) = tags.get("search_highlight") {
                    text_buffer.remove_tag(tag, &text_buffer.start_iter(), &text_buffer.end_iter());
                }
                return;
            }

            // Find and highlight matches
            let mut start_iter = text_buffer.start_iter();
            let tags = text_tags.lock().unwrap();
            if let Some(tag) = tags.get("search_highlight") {
                // Clear previous highlights
                text_buffer.remove_tag(tag, &text_buffer.start_iter(), &text_buffer.end_iter());
                
                // Find and highlight new matches
                while let Some(mut match_start) = start_iter.forward_search(&search_text, gtk4::TextSearchFlags::CASE_INSENSITIVE, None) {
                    let (match_start_iter, match_end_iter) = match_start;
                    text_buffer.apply_tag(tag, &match_start_iter, &match_end_iter);
                    start_iter = match_end_iter;
                }
            }
        });

        // Connect search entry activate event
        let text_view = self.text_view.clone();
        search_entry.connect_activate(move |entry| {
            let search_text = entry.text().to_string();
            if !search_text.is_empty() {
                // Move to next match
                if let Some(cursor_iter) = text_view.cursor_position() {
                    let buffer = text_view.buffer().unwrap();
                    if let Some((match_start, match_end)) = cursor_iter.forward_search(
                        &search_text, 
                        gtk4::TextSearchFlags::CASE_INSENSITIVE, 
                        None
                    ) {
                        buffer.select_range(&match_start, &match_end);
                        text_view.scroll_to_iter(&match_start, 0.0, false, 0.0, 0.0);
                    }
                }
            }
        });

        Ok(())
    }

    /// Set up zoom functionality
    fn setup_zoom(&mut self) -> Result<(), AppError> {
        let text_view = self.text_view.clone();
        
        self.zoom_adjustment.connect_value_changed(move |adjustment| {
            let zoom_level = adjustment.value();
            // Create a new font with the adjusted size
            let font_desc = gtk4::pango::FontDescription::from_string(&format!("Monospace {:.0}pt", 12.0 * zoom_level));
            text_view.set_font(Some(&font_desc));
        });

        Ok(())
    }

    /// Set up event handlers
    fn setup_events(&mut self) -> Result<(), AppError> {
        // Set up key events for text view
        let controller = EventControllerKey::new();
        
        let search_bar = self.search_bar.clone();
        controller.connect_key_pressed(move |_, key, _, _| {
            match key {
                gtk4::gdk::Key::Control_L | gtk4::gdk::Key::Control_R => {
                    // When Ctrl is pressed, enable search
                    search_bar.set_search_mode(true);
                    Propagation::Proceed
                },
                _ => Propagation::Proceed,
            }
        });
        
        self.text_view.add_controller(controller);

        Ok(())
    }

    /// Layout the components
    fn layout_components(&mut self) {
        // Add search bar to container
        self.container.append(&self.search_bar);
        
        // Add scrolled window to container
        self.container.append(&self.scrolled_window);
    }

    /// Set the transcription result to display
    pub fn set_transcription_result(&self, result: TranscriptionResult) {
        // Store the result
        *self.current_result.lock().unwrap() = Some(result.clone());
        
        // Update the text buffer based on current view mode
        let view_mode = *self.view_mode.lock().unwrap();
        self.update_text_content(&result, view_mode);
    }

    /// Update the text buffer content based on view mode
    fn update_text_content(&self, result: &TranscriptionResult, view_mode: TranscriptionViewMode) {
        // Clear the buffer
        self.text_buffer.set_text("");
        
        match view_mode {
            TranscriptionViewMode::PlainText => {
                // Display plain text without any formatting
                self.text_buffer.insert(&mut self.text_buffer.start_iter(), &result.text);
            },
            TranscriptionViewMode::WithTimestamps => {
                // Display text with timestamps
                for segment in &result.segments {
                    let start_iter = self.text_buffer.end_iter();
                    
                    // Insert timestamp
                    let timestamp = format!("[{:02}:{:02}.{:02}] ", 
                        segment.start.as_secs() / 60,
                        segment.start.as_secs() % 60,
                        segment.start.as_millis() / 100);
                    self.text_buffer.insert_with_tags_by_name(&mut start_iter, &timestamp, &["timestamp"]);
                    
                    // Insert segment text
                    let start_iter = self.text_buffer.end_iter();
                    self.text_buffer.insert(&mut start_iter, &segment.text);
                    
                    // Insert newline
                    let start_iter = self.text_buffer.end_iter();
                    self.text_buffer.insert(&mut start_iter, "\n");
                }
            },
            TranscriptionViewMode::WithSpeakers => {
                // Display text with speaker labels
                // This would require speaker information in segments
                // For now, we'll use a generic "Speaker" label
                for (i, segment) in result.segments.iter().enumerate() {
                    let speaker_id = i % 3 + 1; // Rotate between 3 speakers
                    let start_iter = self.text_buffer.end_iter();
                    
                    // Insert speaker label
                    let speaker = format!("Speaker {}: ", speaker_id);
                    self.text_buffer.insert_with_tags_by_name(&mut start_iter, &speaker, &["speaker"]);
                    
                    // Insert segment text
                    let start_iter = self.text_buffer.end_iter();
                    
                    // Apply low confidence tag if needed
                    if segment.confidence < 0.7 {
                        self.text_buffer.insert_with_tags_by_name(&mut start_iter, &segment.text, &["low_confidence"]);
                    } else {
                        self.text_buffer.insert(&mut start_iter, &segment.text);
                    }
                    
                    // Insert newline
                    let start_iter = self.text_buffer.end_iter();
                    self.text_buffer.insert(&mut start_iter, "\n");
                }
            },
            TranscriptionViewMode::Full => {
                // Display text with both timestamps and speakers
                for (i, segment) in result.segments.iter().enumerate() {
                    let speaker_id = i % 3 + 1; // Rotate between 3 speakers
                    let start_iter = self.text_buffer.end_iter();
                    
                    // Insert timestamp
                    let timestamp = format!("[{:02}:{:02}.{:02}] ", 
                        segment.start.as_secs() / 60,
                        segment.start.as_secs() % 60,
                        segment.start.as_millis() / 100);
                    self.text_buffer.insert_with_tags_by_name(&mut start_iter, &timestamp, &["timestamp"]);
                    
                    // Insert speaker label
                    let speaker = format!("Speaker {}: ", speaker_id);
                    self.text_buffer.insert_with_tags_by_name(&mut start_iter, &speaker, &["speaker"]);
                    
                    // Insert segment text
                    let start_iter = self.text_buffer.end_iter();
                    
                    // Apply low confidence tag if needed
                    if segment.confidence < 0.7 {
                        self.text_buffer.insert_with_tags_by_name(&mut start_iter, &segment.text, &["low_confidence"]);
                    } else {
                        self.text_buffer.insert(&mut start_iter, &segment.text);
                    }
                    
                    // Insert newline
                    let start_iter = self.text_buffer.end_iter();
                    self.text_buffer.insert(&mut start_iter, "\n");
                }
            },
        }
    }

    /// Set the view mode
    pub fn set_view_mode(&self, mode: TranscriptionViewMode) {
        *self.view_mode.lock().unwrap() = mode;
        
        // Update the text content if we have a result
        if let Some(ref result) = *self.current_result.lock().unwrap() {
            self.update_text_content(result, mode);
        }
    }

    /// Get the current view mode
    pub fn get_view_mode(&self) -> TranscriptionViewMode {
        *self.view_mode.lock().unwrap()
    }

    /// Show the find/replace dialog
    pub fn show_find_replace_dialog(&self, parent: &ApplicationWindow) {
        let mut dialog_guard = self.find_replace_dialog.lock().unwrap();
        
        if dialog_guard.is_none() {
            // Create the dialog
            let dialog = Dialog::builder()
                .title("Find and Replace")
                .transient_for(parent)
                .modal(true)
                .default_width(400)
                .default_height(300)
                .build();
            
            // Add buttons
            dialog.add_button("Close", ResponseType::Close);
            dialog.add_button("Replace All", ResponseType::Apply);
            dialog.add_button("Replace", ResponseType::Accept);
            dialog.add_button("Find", ResponseType::Ok);
            
            // Create content
            let content_area = dialog.content_area();
            let content_box = Box::builder()
                .orientation(Orientation::Vertical)
                .spacing(10)
                .margin_top(10)
                .margin_bottom(10)
                .margin_start(10)
                .margin_end(10)
                .build();
            
            // Find entry
            let find_label = Label::builder()
                .label("Find:")
                .halign(Align::Start)
                .build();
            content_box.append(&find_label);
            
            let find_entry = gtk4::Entry::builder()
                .build();
            content_box.append(&find_entry);
            
            // Replace entry
            let replace_label = Label::builder()
                .label("Replace with:")
                .halign(Align::Start)
                .build();
            content_box.append(&replace_label);
            
            let replace_entry = gtk4::Entry::builder()
                .build();
            content_box.append(&replace_entry);
            
            // Options
            let options_box = Box::builder()
                .orientation(Orientation::Horizontal)
                .spacing(10)
                .build();
            
            let case_sensitive = CheckButton::builder()
                .label("Case sensitive")
                .build();
            options_box.append(&case_sensitive);
            
            let whole_word = CheckButton::builder()
                .label("Whole word")
                .build();
            options_box.append(&whole_word);
            
            content_box.append(&options_box);
            
            content_area.append(&content_box);
            
            // Store the dialog
            *dialog_guard = Some(dialog.clone());
            
            // Connect response signal
            let text_buffer = self.text_buffer.clone();
            let text_tags = Arc::clone(&self.text_tags);
            dialog.connect_response(move |dialog, response| {
                match response {
                    ResponseType::Ok => {
                        // Find next
                        let find_text = find_entry.text().to_string();
                        if !find_text.is_empty() {
                            if let Some(cursor_iter) = text_buffer.start_iter() {
                                let flags = if case_sensitive.is_active() {
                                    gtk4::TextSearchFlags::empty()
                                } else {
                                    gtk4::TextSearchFlags::CASE_INSENSITIVE
                                };
                                
                                if let Some((match_start, _)) = cursor_iter.forward_search(&find_text, flags, None) {
                                    text_buffer.place_cursor(&match_start);
                                    text_buffer.select_range(&match_start, &match_start);
                                }
                            }
                        }
                    },
                    ResponseType::Accept => {
                        // Replace current
                        // Implementation would go here
                    },
                    ResponseType::Apply => {
                        // Replace all
                        let find_text = find_entry.text().to_string();
                        let replace_text = replace_entry.text().to_string();
                        
                        if !find_text.is_empty() {
                            let flags = if case_sensitive.is_active() {
                                gtk4::TextSearchFlags::empty()
                            } else {
                                gtk4::TextSearchFlags::CASE_INSENSITIVE
                            };
                            
                            let mut start_iter = text_buffer.start_iter();
                            let mut replacements = 0;
                            
                            while let Some((match_start, match_end)) = start_iter.forward_search(&find_text, flags, None) {
                                text_buffer.delete(&mut match_start.clone(), &mut match_end.clone());
                                text_buffer.insert(&mut match_start, &replace_text);
                                start_iter = match_start;
                                replacements += 1;
                            }
                            
                            // Show result message
                            let result_dialog = MessageDialog::builder()
                                .text(&format!("Replaced {} occurrences", replacements))
                                .buttons(ButtonsType::Ok)
                                .message_type(MessageType::Info)
                                .transient_for(dialog)
                                .modal(true)
                                .build();
                            
                            result_dialog.connect_response(|dialog, _| {
                                dialog.close();
                            });
                            
                            result_dialog.show();
                        }
                    },
                    _ => {
                        dialog.close();
                    }
                }
            });
        }
        
        // Show the dialog
        if let Some(ref dialog) = *dialog_guard {
            dialog.present();
        }
    }

    /// Get the text view widget
    pub fn get_text_view(&self) -> &TextView {
        &self.text_view
    }

    /// Get the main container widget
    pub fn get_widget(&self) -> &Box {
        &self.container
    }

    /// Get the zoom adjustment
    pub fn get_zoom_adjustment(&self) -> &Adjustment {
        &self.zoom_adjustment
    }

    /// Toggle search bar visibility
    pub fn toggle_search(&self) {
        let is_search_mode = self.search_bar.is_search_mode();
        self.search_bar.set_search_mode(!is_search_mode);
        
        if !is_search_mode {
            // Focus the search entry when enabling search
            self.search_entry.grab_focus();
        }
    }

    /// Export the current text content
    pub fn export_text(&self) -> String {
        self.text_buffer.text(&self.text_buffer.start_iter(), &self.text_buffer.end_iter(), true).to_string()
    }

    /// Get the current text content with timestamps
    pub fn get_text_with_timestamps(&self) -> String {
        if let Some(ref result) = *self.current_result.lock().unwrap() {
            let mut output = String::new();
            
            for segment in &result.segments {
                let timestamp = format!("[{:02}:{:02}.{:02}] ", 
                    segment.start.as_secs() / 60,
                    segment.start.as_secs() % 60,
                    segment.start.as_millis() / 100);
                
                output.push_str(&timestamp);
                output.push_str(&segment.text);
                output.push('\n');
            }
            
            output
        } else {
            self.text_buffer.text(&self.text_buffer.start_iter(), &self.text_buffer.end_iter(), true).to_string()
        }
    }

    /// Get the current text content as SRT format
    pub fn get_text_as_srt(&self) -> String {
        if let Some(ref result) = *self.current_result.lock().unwrap() {
            let mut output = String::new();
            
            for (i, segment) in result.segments.iter().enumerate() {
                // Sequence number
                output.push_str(&format!("{}\n", i + 1));
                
                // Timestamp
                let start_time = format_seconds_to_srt_time(segment.start.as_secs() as u32, segment.start.subsec_millis());
                let end_time = format_seconds_to_srt_time(segment.end.as_secs() as u32, segment.end.subsec_millis());
                output.push_str(&format!("{} --> {}\n", start_time, end_time));
                
                // Text
                output.push_str(&segment.text);
                output.push_str("\n\n");
            }
            
            output
        } else {
            String::new()
        }
    }

    /// Get the current text content as VTT format
    pub fn get_text_as_vtt(&self) -> String {
        if let Some(ref result) = *self.current_result.lock().unwrap() {
            let mut output = String::new();
            output.push_str("WEBVTT\n\n");
            
            for segment in &result.segments {
                // Timestamp
                let start_time = format_seconds_to_vtt_time(segment.start.as_secs() as u32, segment.start.subsec_millis());
                let end_time = format_seconds_to_vtt_time(segment.end.as_secs() as u32, segment.end.subsec_millis());
                output.push_str(&format!("{} --> {}\n", start_time, end_time));
                
                // Text
                output.push_str(&segment.text);
                output.push_str("\n\n");
            }
            
            output
        } else {
            String::new()
        }
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