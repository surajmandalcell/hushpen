//! Waveform Visualization Widget
//!
//! This module provides a waveform visualization widget for displaying audio waveforms
//! with zoom, pan, selection, and click-to-seek functionality.

use glib::clone;
use gtk4::prelude::*;
use gtk4::{
    Box, DrawingArea, Adjustment, Scale, Orientation, Align, Button,
    ApplicationWindow, ScrolledWindow, Label, Separator,
    EventControllerMotion, EventControllerScroll,
    EventSequenceState, GestureClick,
    Popover, MenuButton, PositionType
};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::utils::{AppError, AppResult, audio_processor::{AudioProcessor, WaveformData}};

/// Selection region in the waveform
#[derive(Debug, Clone)]
pub struct Selection {
    /// Start position (0.0 to 1.0)
    pub start: f64,
    /// End position (0.0 to 1.0)
    pub end: f64,
}

impl Selection {
    /// Create a new selection
    pub fn new(start: f64, end: f64) -> Self {
        let (normalized_start, normalized_end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        
        Self {
            start: normalized_start.clamp(0.0, 1.0),
            end: normalized_end.clamp(0.0, 1.0),
        }
    }
    
    /// Get the duration of the selection
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }
    
    /// Check if the selection is empty
    pub fn is_empty(&self) -> bool {
        self.duration() < 0.001 // Less than 0.1% of the audio
    }
    
    /// Check if a position is within the selection
    pub fn contains(&self, position: f64) -> bool {
        position >= self.start && position <= self.end
    }
}

/// Waveform visualization widget
pub struct WaveformWidget {
    /// Main container
    container: Box,
    /// Application window for dialogs
    app_window: Option<ApplicationWindow>,
    /// Current waveform data
    waveform_data: Arc<Mutex<Option<WaveformData>>>,
    /// Current selection
    selection: Arc<Mutex<Option<Selection>>>,
    /// Current playback position (0.0 to 1.0)
    playback_position: Arc<Mutex<f64>>,
    /// Zoom level (1.0 = 100%)
    zoom_level: Arc<Mutex<f64>>,
    /// Pan position (0.0 to 1.0)
    pan_position: Arc<Mutex<f64>>,
    /// Viewport start position (0.0 to 1.0)
    viewport_start: Arc<Mutex<f64>>,
    /// Viewport end position (0.0 to 1.0)
    viewport_end: Arc<Mutex<f64>>,
    
    /// UI Components
    /// Drawing area for the waveform
    drawing_area: DrawingArea,
    /// Scrolled window for the drawing area
    scrolled_window: ScrolledWindow,
    /// Zoom scale
    zoom_scale: Scale,
    /// Zoom adjustment
    zoom_adjustment: Adjustment,
    /// Position scale for seeking
    position_scale: Scale,
    /// Position adjustment
    position_adjustment: Adjustment,
    /// Controls box
    controls_box: Box,
    /// Info label
    info_label: Label,
    /// Selection info label
    selection_info_label: Label,
    
    /// Interaction state
    /// Is the user currently selecting
    is_selecting: Arc<Mutex<bool>>,
    /// Selection start position
    selection_start: Arc<Mutex<Option<f64>>>,
    /// Is the user currently dragging the viewport
    is_dragging: Arc<Mutex<bool>>,
    /// Last mouse position for dragging
    last_mouse_position: Arc<Mutex<Option<(f64, f64)>>>,
    
    /// Callbacks
    /// Position change callback
    position_changed_callback: Arc<Mutex<Option<Box<dyn Fn(f64) + Send + Sync>>>>,
    /// Selection changed callback
    selection_changed_callback: Arc<Mutex<Option<Box<dyn Fn(Option<Selection>) + Send + Sync>>>>,
}

impl WaveformWidget {
    /// Create a new waveform widget
    pub fn new() -> Result<Self, AppError> {
        // Create the main container
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(5)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .css_classes(vec!["waveform-widget".to_string()])
            .build();
        
        // Create the drawing area
        let drawing_area = DrawingArea::builder()
            .content_width(800)
            .content_height(200)
            .hexpand(true)
            .vexpand(true)
            .css_classes(vec!["waveform-drawing-area".to_string()])
            .build();
        
        // Create the scrolled window
        let scrolled_window = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Never)
            .min_content_height(200)
            .hexpand(true)
            .vexpand(true)
            .build();
        
        scrolled_window.set_child(Some(&drawing_area));
        
        // Create the controls box
        let controls_box = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(5)
            .margin_top(5)
            .build();
        
        // Create zoom adjustment and scale
        let zoom_adjustment = Adjustment::new(1.0, 0.1, 10.0, 0.1, 1.0, 0.0);
        let zoom_scale = Scale::builder()
            .orientation(Orientation::Horizontal)
            .adjustment(&zoom_adjustment)
            .hexpand(false)
            .width_request(150)
            .draw_value(true)
            .has_origin(true)
            .tooltip_text("Zoom (scroll to zoom)")
            .css_classes(vec!["waveform-zoom-scale".to_string()])
            .build();
        
        // Create position adjustment and scale
        let position_adjustment = Adjustment::new(0.0, 0.0, 1.0, 0.01, 0.1, 0.0);
        let position_scale = Scale::builder()
            .orientation(Orientation::Horizontal)
            .adjustment(&position_adjustment)
            .hexpand(true)
            .draw_value(false)
            .has_origin(false)
            .tooltip_text("Position (click to seek)")
            .css_classes(vec!["waveform-position-scale".to_string()])
            .build();
        
        // Create info label
        let info_label = Label::builder()
            .label("No audio loaded")
            .halign(Align::Start)
            .css_classes(vec!["waveform-info-label".to_string()])
            .build();
        
        // Create selection info label
        let selection_info_label = Label::builder()
            .label("")
            .halign(Align::Start)
            .css_classes(vec!["waveform-selection-info-label".to_string()])
            .build();
        
        // Add components to controls box
        controls_box.append(&Label::builder().label("Zoom:").build());
        controls_box.append(&zoom_scale);
        controls_box.append(&Separator::new(Orientation::Vertical));
        controls_box.append(&Label::builder().label("Position:").build());
        controls_box.append(&position_scale);
        
        // Add components to main container
        container.append(&scrolled_window);
        container.append(&controls_box);
        container.append(&Separator::new(Orientation::Horizontal));
        container.append(&info_label);
        container.append(&selection_info_label);
        
        // Create the widget
        let widget = Self {
            container,
            app_window: None,
            waveform_data: Arc::new(Mutex::new(None)),
            selection: Arc::new(Mutex::new(None)),
            playback_position: Arc::new(Mutex::new(0.0)),
            zoom_level: Arc::new(Mutex::new(1.0)),
            pan_position: Arc::new(Mutex::new(0.0)),
            viewport_start: Arc::new(Mutex::new(0.0)),
            viewport_end: Arc::new(Mutex::new(1.0)),
            drawing_area,
            scrolled_window,
            zoom_scale,
            zoom_adjustment,
            position_scale,
            position_adjustment,
            controls_box,
            info_label,
            selection_info_label,
            is_selecting: Arc::new(Mutex::new(false)),
            selection_start: Arc::new(Mutex::new(None)),
            is_dragging: Arc::new(Mutex::new(false)),
            last_mouse_position: Arc::new(Mutex::new(None)),
            position_changed_callback: Arc::new(Mutex::new(None)),
            selection_changed_callback: Arc::new(Mutex::new(None)),
        };
        
        // Set up event handlers
        widget.setup_events()?;
        
        Ok(widget)
    }
    
    /// Set up event handlers
    fn setup_events(&self) -> AppResult<()> {
        // Set up the drawing area draw function
        let waveform_data = self.waveform_data.clone();
        let selection = self.selection.clone();
        let playback_position = self.playback_position.clone();
        let viewport_start = self.viewport_start.clone();
        let viewport_end = self.viewport_end.clone();
        
        self.drawing_area.set_draw_func(move |area, context, width, height| {
            // Draw background
            context.set_source_rgb(0.95, 0.95, 0.95);
            context.rectangle(0.0, 0.0, width as f64, height as f64);
            context.fill();
            
            // Get waveform data
            if let Ok(data_guard) = waveform_data.lock() {
                if let Some(ref data) = *data_guard {
                    // Draw the waveform
                    Self::draw_waveform(context, data, width, height, &viewport_start, &viewport_end);
                    
                    // Draw selection
                    if let Ok(selection_guard) = selection.lock() {
                        if let Some(ref sel) = *selection_guard {
                            Self::draw_selection(context, sel, width, height, &viewport_start, &viewport_end);
                        }
                    }
                    
                    // Draw playback position
                    if let Ok(pos_guard) = playback_position.lock() {
                        let position = *pos_guard;
                        Self::draw_playback_position(context, position, width, height, &viewport_start, &viewport_end);
                    }
                } else {
                    // Draw placeholder text
                    context.set_source_rgb(0.5, 0.5, 0.5);
                    context.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
                    context.set_font_size(14.0);
                    
                    let text = "No audio loaded";
                    let extents = context.text_extents(text);
                    let x = (width as f64 - extents.width) / 2.0;
                    let y = (height as f64 + extents.height) / 2.0;
                    
                    context.move_to(x, y);
                    context.show_text(text).unwrap();
                }
            }
        });
        
        // Set up mouse events
        self.setup_mouse_events()?;
        
        // Set up zoom events
        self.setup_zoom_events()?;
        
        // Set up position events
        self.setup_position_events()?;
        
        Ok(())
    }
    
    /// Set up mouse events
    fn setup_mouse_events(&self) -> AppResult<()> {
        let selection = self.selection.clone();
        let is_selecting = self.is_selecting.clone();
        let selection_start = self.selection_start.clone();
        let viewport_start = self.viewport_start.clone();
        let viewport_end = self.viewport_end.clone();
        let is_dragging = self.is_dragging.clone();
        let last_mouse_position = self.last_mouse_position.clone();
        let playback_position = self.playback_position.clone();
        let position_changed_callback = self.position_changed_callback.clone();
        let selection_changed_callback = self.selection_changed_callback.clone();
        let drawing_area = self.drawing_area.clone();
        
        // Mouse button press
        let gesture_click = GestureClick::builder()
            .button(0) // Any button
            .build();
        
        gesture_click.connect_pressed(move |_, _, x, y| {
            let width = drawing_area.allocated_width() as f64;
            let height = drawing_area.allocated_height() as f64;
            
            // Convert mouse position to waveform position
            let waveform_pos = Self::screen_to_waveform_position(x, width, &viewport_start, &viewport_end);
            
            // Check if shift is pressed (for selection)
            // This is a simplified check - in a real implementation, you would
            // check the actual modifier keys
            let is_shift_pressed = false; // TODO: Check actual modifier keys
            
            if is_shift_pressed {
                // Start selection
                if let Ok(mut is_selecting_guard) = is_selecting.lock() {
                    *is_selecting_guard = true;
                }
                
                if let Ok(mut selection_start_guard) = selection_start.lock() {
                    *selection_start_guard = Some(waveform_pos);
                }
            } else {
                // Click to seek
                if let Ok(mut pos_guard) = playback_position.lock() {
                    *pos_guard = waveform_pos;
                }
                
                // Call the position changed callback
                if let Ok(callback_guard) = position_changed_callback.lock() {
                    if let Some(ref callback) = *callback_guard {
                        callback(waveform_pos);
                    }
                }
                
                // Start dragging
                if let Ok(mut is_dragging_guard) = is_dragging.lock() {
                    *is_dragging_guard = true;
                }
                
                if let Ok(mut last_pos_guard) = last_mouse_position.lock() {
                    *last_pos_guard = Some((x, y));
                }
            }
            
            // Redraw
            drawing_area.queue_draw();
        });
        
        gesture_click.connect_released(move |_, _, x, y| {
            // Stop selecting
            if let Ok(mut is_selecting_guard) = is_selecting.lock() {
                if *is_selecting_guard {
                    *is_selecting_guard = false;
                    
                    // Create the selection
                    let width = drawing_area.allocated_width() as f64;
                    let waveform_pos = Self::screen_to_waveform_position(x, width, &viewport_start, &viewport_end);
                    
                    if let Ok(mut selection_start_guard) = selection_start.lock() {
                        if let Some(start_pos) = *selection_start_guard {
                            let new_selection = Selection::new(start_pos, waveform_pos);
                            
                            if let Ok(mut selection_guard) = selection.lock() {
                                *selection_guard = Some(new_selection.clone());
                            }
                            
                            // Call the selection changed callback
                            if let Ok(callback_guard) = selection_changed_callback.lock() {
                                if let Some(ref callback) = *callback_guard {
                                    callback(Some(new_selection));
                                }
                            }
                        }
                    }
                    
                    *selection_start_guard = None;
                }
            }
            
            // Stop dragging
            if let Ok(mut is_dragging_guard) = is_dragging.lock() {
                *is_dragging_guard = false;
            }
            
            if let Ok(mut last_pos_guard) = last_mouse_position.lock() {
                *last_pos_guard = None;
            }
            
            // Redraw
            drawing_area.queue_draw();
        });
        
        self.drawing_area.add_controller(gesture_click);
        
        // Mouse motion
        let motion_controller = EventControllerMotion::new();
        
        motion_controller.connect_motion(move |_, x, y| {
            // Handle selection
            if let Ok(is_selecting_guard) = is_selecting.lock() {
                if *is_selecting_guard {
                    let width = drawing_area.allocated_width() as f64;
                    let waveform_pos = Self::screen_to_waveform_position(x, width, &viewport_start, &viewport_end);
                    
                    if let Ok(mut selection_start_guard) = selection_start.lock() {
                        if let Some(start_pos) = *selection_start_guard {
                            let new_selection = Selection::new(start_pos, waveform_pos);
                            
                            if let Ok(mut selection_guard) = selection.lock() {
                                *selection_guard = Some(new_selection.clone());
                            }
                            
                            // Call the selection changed callback
                            if let Ok(callback_guard) = selection_changed_callback.lock() {
                                if let Some(ref callback) = *callback_guard {
                                    callback(Some(new_selection));
                                }
                            }
                        }
                    }
                    
                    // Redraw
                    drawing_area.queue_draw();
                }
            }
            
            // Handle dragging
            if let Ok(is_dragging_guard) = is_dragging.lock() {
                if *is_dragging_guard {
                    if let Ok(mut last_pos_guard) = last_mouse_position.lock() {
                        if let Some((last_x, _)) = *last_pos_guard {
                            let dx = x - last_x;
                            let width = drawing_area.allocated_width() as f64;
                            let viewport_width = if let Ok(start_guard) = viewport_start.lock() {
                                if let Ok(end_guard) = viewport_end.lock() {
                                    *end_guard - *start_guard
                                } else {
                                    1.0
                                }
                            } else {
                                1.0
                            };
                            
                            let pan_amount = dx / width * viewport_width;
                            
                            // Update viewport
                            if let Ok(mut start_guard) = viewport_start.lock() {
                                *start_guard = (*start_guard - pan_amount).clamp(0.0, 1.0 - viewport_width);
                            }
                            
                            if let Ok(mut end_guard) = viewport_end.lock() {
                                if let Ok(start_guard) = viewport_start.lock() {
                                    *end_guard = (*start_guard + viewport_width).clamp(0.0, 1.0);
                                }
                            }
                            
                            *last_pos_guard = Some((x, y));
                            
                            // Redraw
                            drawing_area.queue_draw();
                        }
                    }
                }
            }
        });
        
        self.drawing_area.add_controller(motion_controller);
        
        Ok(())
    }
    
    /// Set up zoom events
    fn setup_zoom_events(&self) -> AppResult<()> {
        let zoom_level = self.zoom_level.clone();
        let viewport_start = self.viewport_start.clone();
        let viewport_end = self.viewport_end.clone();
        let drawing_area = self.drawing_area.clone();
        
        self.zoom_adjustment.connect_value_changed(move |adj| {
            let zoom = adj.value();
            
            if let Ok(mut zoom_guard) = zoom_level.lock() {
                *zoom_guard = zoom;
            }
            
            // Update viewport
            Self::update_viewport_for_zoom(zoom, &viewport_start, &viewport_end);
            
            // Redraw
            drawing_area.queue_draw();
        });
        
        // Mouse scroll for zooming
        let scroll_controller = EventControllerScroll::builder()
            .flags(gtk4::EventControllerScrollFlags::BOTH_AXES)
            .build();
        
        scroll_controller.connect_scroll(move |_, dx, dy| {
            let zoom_delta = if dy != 0.0 { dy } else { dx };
            
            if zoom_delta != 0.0 {
                if let Ok(mut zoom_guard) = zoom_level.lock() {
                    let new_zoom = (*zoom_guard * (1.0 - zoom_delta * 0.1)).clamp(0.1, 10.0);
                    *zoom_guard = new_zoom;
                    
                    // Update the zoom scale
                    if let Some(scale) = zoom_scale.adjustment() {
                        scale.set_value(new_zoom);
                    }
                    
                    // Update viewport
                    Self::update_viewport_for_zoom(new_zoom, &viewport_start, &viewport_end);
                    
                    // Redraw
                    drawing_area.queue_draw();
                }
            }
            
            EventSequenceState::Claimed
        });
        
        self.drawing_area.add_controller(scroll_controller);
        
        Ok(())
    }
    
    /// Set up position events
    fn setup_position_events(&self) -> AppResult<()> {
        let playback_position = self.playback_position.clone();
        let position_changed_callback = self.position_changed_callback.clone();
        let drawing_area = self.drawing_area.clone();
        
        self.position_adjustment.connect_value_changed(move |adj| {
            let position = adj.value();
            
            if let Ok(mut pos_guard) = playback_position.lock() {
                *pos_guard = position;
            }
            
            // Call the position changed callback
            if let Ok(callback_guard) = position_changed_callback.lock() {
                if let Some(ref callback) = *callback_guard {
                    callback(position);
                }
            }
            
            // Redraw
            drawing_area.queue_draw();
        });
        
        Ok(())
    }
    
    /// Load waveform data from an audio file
    pub fn load_waveform(&self, file_path: &Path) -> AppResult<()> {
        // Generate waveform data
        let waveform_data = AudioProcessor::generate_waveform(file_path, 1000)?;
        
        // Update the waveform data
        if let Ok(mut data_guard) = self.waveform_data.lock() {
            *data_guard = Some(waveform_data.clone());
        }
        
        // Update the info label
        let duration_secs = waveform_data.duration.as_secs();
        let minutes = duration_secs / 60;
        let seconds = duration_secs % 60;
        self.info_label.set_text(&format!(
            "Duration: {}:{:02}, Sample Rate: {} Hz, Channels: {}, Samples: {}",
            minutes, seconds, waveform_data.sample_rate, waveform_data.channels, waveform_data.samples.len()
        ));
        
        // Reset view
        self.reset_view();
        
        // Redraw
        self.drawing_area.queue_draw();
        
        Ok(())
    }
    
    /// Reset the view to show the entire waveform
    pub fn reset_view(&self) {
        if let Ok(mut zoom_guard) = self.zoom_level.lock() {
            *zoom_guard = 1.0;
            self.zoom_adjustment.set_value(1.0);
        }
        
        if let Ok(mut start_guard) = self.viewport_start.lock() {
            *start_guard = 0.0;
        }
        
        if let Ok(mut end_guard) = self.viewport_end.lock() {
            *end_guard = 1.0;
        }
        
        if let Ok(mut selection_guard) = self.selection.lock() {
            *selection_guard = None;
            self.selection_info_label.set_text("");
        }
        
        if let Ok(mut pos_guard) = self.playback_position.lock() {
            *pos_guard = 0.0;
        }
        
        self.position_adjustment.set_value(0.0);
        
        // Redraw
        self.drawing_area.queue_draw();
    }
    
    /// Set the playback position
    pub fn set_playback_position(&self, position: f64) {
        let clamped_position = position.clamp(0.0, 1.0);
        
        if let Ok(mut pos_guard) = self.playback_position.lock() {
            *pos_guard = clamped_position;
        }
        
        self.position_adjustment.set_value(clamped_position);
        
        // Redraw
        self.drawing_area.queue_draw();
    }
    
    /// Get the current playback position
    pub fn get_playback_position(&self) -> f64 {
        if let Ok(pos_guard) = self.playback_position.lock() {
            *pos_guard
        } else {
            0.0
        }
    }
    
    /// Set the selection
    pub fn set_selection(&self, selection: Option<Selection>) {
        if let Ok(mut selection_guard) = self.selection.lock() {
            *selection_guard = selection.clone();
            
            // Update the selection info label
            if let Some(ref sel) = selection {
                if let Ok(data_guard) = self.waveform_data.lock() {
                    if let Some(ref data) = *data_guard {
                        let duration_secs = data.duration.as_secs() as f64;
                        let selection_duration = sel.duration() * duration_secs;
                        let selection_start_secs = sel.start * duration_secs;
                        
                        self.selection_info_label.set_text(&format!(
                            "Selection: {:.2}s - {:.2}s (Duration: {:.2}s)",
                            selection_start_secs, selection_start_secs + selection_duration, selection_duration
                        ));
                    }
                }
            } else {
                self.selection_info_label.set_text("");
            }
        }
        
        // Redraw
        self.drawing_area.queue_draw();
    }
    
    /// Get the current selection
    pub fn get_selection(&self) -> Option<Selection> {
        if let Ok(selection_guard) = self.selection.lock() {
            selection_guard.clone()
        } else {
            None
        }
    }
    
    /// Set the zoom level
    pub fn set_zoom(&self, zoom: f64) {
        let clamped_zoom = zoom.clamp(0.1, 10.0);
        
        if let Ok(mut zoom_guard) = self.zoom_level.lock() {
            *zoom_guard = clamped_zoom;
        }
        
        self.zoom_adjustment.set_value(clamped_zoom);
        Self::update_viewport_for_zoom(clamped_zoom, &self.viewport_start, &self.viewport_end);
        
        // Redraw
        self.drawing_area.queue_draw();
    }
    
    /// Get the current zoom level
    pub fn get_zoom(&self) -> f64 {
        if let Ok(zoom_guard) = self.zoom_level.lock() {
            *zoom_guard
        } else {
            1.0
        }
    }
    
    /// Set the position changed callback
    pub fn set_position_changed_callback<F>(&self, callback: F)
    where
        F: Fn(f64) + Send + Sync + 'static,
    {
        if let Ok(mut callback_guard) = self.position_changed_callback.lock() {
            *callback_guard = Some(Box::new(callback));
        }
    }
    
    /// Set the selection changed callback
    pub fn set_selection_changed_callback<F>(&self, callback: F)
    where
        F: Fn(Option<Selection>) + Send + Sync + 'static,
    {
        if let Ok(mut callback_guard) = self.selection_changed_callback.lock() {
            *callback_guard = Some(Box::new(callback));
        }
    }
    
    /// Set the application window for dialogs
    pub fn set_application_window(&mut self, window: ApplicationWindow) {
        self.app_window = Some(window);
    }
    
    /// Clear the waveform
    pub fn clear(&self) {
        // Clear the waveform data
        if let Ok(mut data_guard) = self.waveform_data.lock() {
            *data_guard = None;
        }
        
        // Reset view
        self.reset_view();
        
        // Clear the info label
        self.info_label.set_text("No audio loaded");
        
        // Redraw
        self.drawing_area.queue_draw();
    }
    
    /// Get the main container widget
    pub fn get_widget(&self) -> &Box {
        &self.container
    }
    
    /// Update viewport for zoom level
    fn update_viewport_for_zoom(
        zoom: f64,
        viewport_start: &Arc<Mutex<f64>>,
        viewport_end: &Arc<Mutex<f64>>,
    ) {
        let viewport_width = 1.0 / zoom;
        let center = if let Ok(start_guard) = viewport_start.lock() {
            if let Ok(end_guard) = viewport_end.lock() {
                (*start_guard + *end_guard) / 2.0
            } else {
                0.5
            }
        } else {
            0.5
        };
        
        let new_start = (center - viewport_width / 2.0).clamp(0.0, 1.0 - viewport_width);
        let new_end = (center + viewport_width / 2.0).clamp(viewport_width, 1.0);
        
        if let Ok(mut start_guard) = viewport_start.lock() {
            *start_guard = new_start;
        }
        
        if let Ok(mut end_guard) = viewport_end.lock() {
            *end_guard = new_end;
        }
    }
    
    /// Convert screen position to waveform position
    fn screen_to_waveform_position(
        x: f64,
        width: f64,
        viewport_start: &Arc<Mutex<f64>>,
        viewport_end: &Arc<Mutex<f64>>,
    ) -> f64 {
        let screen_pos = (x / width).clamp(0.0, 1.0);
        
        if let Ok(start_guard) = viewport_start.lock() {
            if let Ok(end_guard) = viewport_end.lock() {
                let viewport_width = *end_guard - *start_guard;
                *start_guard + screen_pos * viewport_width
            } else {
                screen_pos
            }
        } else {
            screen_pos
        }
    }
    
    /// Draw the waveform
    fn draw_waveform(
        context: &cairo::Context,
        data: &WaveformData,
        width: i32,
        height: i32,
        viewport_start: &Arc<Mutex<f64>>,
        viewport_end: &Arc<Mutex<f64>>,
    ) {
        let width = width as f64;
        let height = height as f64;
        
        // Get viewport
        let (viewport_start, viewport_end) = if let Ok(start_guard) = viewport_start.lock() {
            if let Ok(end_guard) = viewport_end.lock() {
                (*start_guard, *end_guard)
            } else {
                (0.0, 1.0)
            }
        } else {
            (0.0, 1.0)
        };
        
        let viewport_width = viewport_end - viewport_start;
        
        // Calculate the range of samples to display
        let sample_count = data.samples.len();
        let start_sample = (viewport_start * sample_count as f64) as usize;
        let end_sample = (viewport_end * sample_count as f64).min(sample_count as f64) as usize;
        
        if start_sample >= end_sample {
            return;
        }
        
        // Draw the waveform
        context.set_source_rgb(0.2, 0.5, 0.8);
        context.set_line_width(1.0);
        
        let samples_per_pixel = (end_sample - start_sample) as f64 / width;
        let mid_y = height / 2.0;
        
        for x in 0..width as i32 {
            let sample_x = x as f64;
            let sample_index = start_sample + (sample_x * samples_per_pixel) as usize;
            
            if sample_index < sample_count {
                let sample = data.samples[sample_index] * (height / 2.0 - 10.0);
                let y = mid_y - sample;
                
                if x == 0 {
                    context.move_to(sample_x, y);
                } else {
                    context.line_to(sample_x, y);
                }
            }
        }
        
        context.stroke();
    }
    
    /// Draw the selection
    fn draw_selection(
        context: &cairo::Context,
        selection: &Selection,
        width: i32,
        height: i32,
        viewport_start: &Arc<Mutex<f64>>,
        viewport_end: &Arc<Mutex<f64>>,
    ) {
        let width = width as f64;
        let height = height as f64;
        
        // Get viewport
        let (viewport_start, viewport_end) = if let Ok(start_guard) = viewport_start.lock() {
            if let Ok(end_guard) = viewport_end.lock() {
                (*start_guard, *end_guard)
            } else {
                (0.0, 1.0)
            }
        } else {
            (0.0, 1.0)
        };
        
        let viewport_width = viewport_end - viewport_start;
        
        // Calculate selection position and width in screen coordinates
        if selection.end < viewport_start || selection.start > viewport_end {
            // Selection is not visible
            return;
        }
        
        let visible_start = selection.start.max(viewport_start);
        let visible_end = selection.end.min(viewport_end);
        
        let start_x = ((visible_start - viewport_start) / viewport_width) * width;
        let end_x = ((visible_end - viewport_start) / viewport_width) * width;
        
        // Draw selection rectangle
        context.set_source_rgba(0.2, 0.5, 0.8, 0.2);
        context.rectangle(start_x, 0.0, end_x - start_x, height);
        context.fill();
        
        // Draw selection border
        context.set_source_rgb(0.2, 0.5, 0.8);
        context.set_line_width(2.0);
        context.move_to(start_x, 0.0);
        context.line_to(start_x, height);
        context.move_to(end_x, 0.0);
        context.line_to(end_x, height);
        context.stroke();
    }
    
    /// Draw the playback position
    fn draw_playback_position(
        context: &cairo::Context,
        position: f64,
        width: i32,
        height: i32,
        viewport_start: &Arc<Mutex<f64>>,
        viewport_end: &Arc<Mutex<f64>>,
    ) {
        let width = width as f64;
        let height = height as f64;
        
        // Get viewport
        let (viewport_start, viewport_end) = if let Ok(start_guard) = viewport_start.lock() {
            if let Ok(end_guard) = viewport_end.lock() {
                (*start_guard, *end_guard)
            } else {
                (0.0, 1.0)
            }
        } else {
            (0.0, 1.0)
        };
        
        let viewport_width = viewport_end - viewport_start;
        
        // Check if position is visible
        if position < viewport_start || position > viewport_end {
            return;
        }
        
        // Calculate position in screen coordinates
        let x = ((position - viewport_start) / viewport_width) * width;
        
        // Draw position line
        context.set_source_rgb(1.0, 0.0, 0.0);
        context.set_line_width(2.0);
        context.move_to(x, 0.0);
        context.line_to(x, height);
        context.stroke();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_creation() {
        let selection = Selection::new(0.2, 0.8);
        assert_eq!(selection.start, 0.2);
        assert_eq!(selection.end, 0.8);
        assert_eq!(selection.duration(), 0.6);
        assert!(!selection.is_empty());
        assert!(selection.contains(0.5));
        assert!(!selection.contains(0.1));
    }

    #[test]
    fn test_selection_clamping() {
        let selection = Selection::new(-0.5, 1.5);
        assert_eq!(selection.start, 0.0);
        assert_eq!(selection.end, 1.0);
    }

    #[test]
    fn test_selection_reversed() {
        let selection = Selection::new(0.8, 0.2);
        assert_eq!(selection.start, 0.2);
        assert_eq!(selection.end, 0.8);
    }

    #[test]
    fn test_selection_empty() {
        let selection = Selection::new(0.5, 0.5005);
        assert!(selection.is_empty());
    }

    #[test]
    fn test_waveform_widget_creation() {
        // Initialize GTK
        gtk4::init().expect("Failed to initialize GTK");
        
        // Create a new waveform widget
        let widget = WaveformWidget::new();
        assert!(widget.is_ok());
    }

    #[test]
    fn test_screen_to_waveform_position() {
        let viewport_start = Arc::new(Mutex::new(0.2));
        let viewport_end = Arc::new(Mutex::new(0.8));
        
        // Middle of the screen should map to middle of the viewport
        let waveform_pos = WaveformWidget::screen_to_waveform_position(
            400.0, 800.0, &viewport_start, &viewport_end
        );
        assert!((waveform_pos - 0.5).abs() < 0.001);
    }
}