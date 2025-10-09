//! Audio Preview Widget
//!
//! This module provides a comprehensive audio preview widget with GStreamer integration,
//! playback controls, volume control, time display, and keyboard shortcuts.

use glib::clone;
use gtk4::prelude::*;
use gtk4::{
    Box, Button, Label, Scale, Adjustment, Switch, Orientation, Align,
    ApplicationWindow, Image, Separator, Grid, SpinButton, ToggleButton,
    EventControllerKey, KeyEvent, EventControllerMotion, EventControllerScroll,
    ScrollEvent, EventSequenceState
};
use gstreamer as gst;
use gstreamer_app as gst_app;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::models::AudioFile;
use crate::utils::{AppError, AppResult, audio_processor::AudioProcessor};

/// Playback state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

/// Audio preview widget
pub struct AudioPreviewWidget {
    /// Main container
    container: Box,
    /// Application window for dialogs
    app_window: Option<ApplicationWindow>,
    /// Current audio file
    current_file: Arc<Mutex<Option<AudioFile>>>,
    /// Playback state
    playback_state: Arc<Mutex<PlaybackState>>,
    /// GStreamer pipeline
    pipeline: Arc<Mutex<Option<gst::Pipeline>>>,
    /// GStreamer bus
    bus: Arc<Mutex<Option<gst::Bus>>>,
    
    /// UI Components
    /// Main controls box
    controls_box: Box,
    /// Play/pause button
    play_button: ToggleButton,
    /// Stop button
    stop_button: Button,
    /// Position scale
    position_scale: Scale,
    /// Position adjustment
    position_adjustment: Adjustment,
    /// Current time label
    current_time_label: Label,
    /// Total time label
    total_time_label: Label,
    /// Volume scale
    volume_scale: Scale,
    /// Mute button
    mute_button: ToggleButton,
    /// Volume label
    volume_label: Label,
    /// Playback speed spin button
    speed_spin_button: SpinButton,
    /// Loop toggle
    loop_toggle: Switch,
    /// Info grid
    info_grid: Grid,
    /// File name label
    file_name_label: Label,
    /// Format label
    format_label: Label,
    /// Duration label
    duration_label: Label,
    /// Sample rate label
    sample_rate_label: Label,
    /// Channels label
    channels_label: Label,
    
    /// Update timeout ID
    update_timeout_id: Arc<Mutex<Option<glib::SourceId>>>,
}

impl AudioPreviewWidget {
    /// Create a new audio preview widget
    pub fn new() -> Result<Self, AppError> {
        // Initialize GStreamer
        if let Err(e) = gst::init() {
            return Err(AppError::generic(format!("Failed to initialize GStreamer: {}", e)));
        }
        
        // Create the main container
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .css_classes(vec!["audio-preview".to_string()])
            .build();
        
        // Create the controls box
        let controls_box = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(5)
            .margin_bottom(10)
            .build();
        
        // Create play/pause button
        let play_button = ToggleButton::builder()
            .icon_name("media-playback-start")
            .tooltip_text("Play/Pause (Space)")
            .css_classes(vec!["audio-play-button".to_string()])
            .build();
        
        // Create stop button
        let stop_button = Button::builder()
            .icon_name("media-playback-stop")
            .tooltip_text("Stop")
            .css_classes(vec!["audio-stop-button".to_string()])
            .sensitive(false)
            .build();
        
        // Create position adjustment and scale
        let position_adjustment = Adjustment::new(0.0, 0.0, 100.0, 1.0, 10.0, 0.0);
        let position_scale = Scale::builder()
            .orientation(Orientation::Horizontal)
            .adjustment(&position_adjustment)
            .hexpand(true)
            .draw_value(false)
            .has_origin(false)
            .css_classes(vec!["audio-position-scale".to_string()])
            .build();
        
        // Create time display labels
        let current_time_label = Label::builder()
            .label("00:00")
            .css_classes(vec!["audio-time-label".to_string()])
            .build();
        
        let total_time_label = Label::builder()
            .label("00:00")
            .css_classes(vec!["audio-time-label".to_string()])
            .build();
        
        // Create volume controls
        let volume_adjustment = Adjustment::new(0.7, 0.0, 1.0, 0.05, 0.1, 0.0);
        let volume_scale = Scale::builder()
            .orientation(Orientation::Horizontal)
            .adjustment(&volume_adjustment)
            .hexpand(false)
            .width_request(100)
            .draw_value(false)
            .has_origin(false)
            .css_classes(vec!["audio-volume-scale".to_string()])
            .build();
        
        let volume_label = Label::builder()
            .label("70%")
            .css_classes(vec!["audio-volume-label".to_string()])
            .build();
        
        let mute_button = ToggleButton::builder()
            .icon_name("audio-volume-high")
            .tooltip_text("Mute/Unmute (M)")
            .css_classes(vec!["audio-mute-button".to_string()])
            .build();
        
        // Create playback speed control
        let speed_adjustment = Adjustment::new(1.0, 0.25, 4.0, 0.25, 0.5, 0.0);
        let speed_spin_button = SpinButton::builder()
            .adjustment(&speed_adjustment)
            .climb_rate(0.25)
            .digits(2)
            .tooltip_text("Playback Speed")
            .css_classes(vec!["audio-speed-control".to_string()])
            .build();
        
        // Create loop toggle
        let loop_toggle = Switch::builder()
            .active(false)
            .tooltip_text("Loop/Repeat (L)")
            .css_classes(vec!["audio-loop-toggle".to_string()])
            .build();
        
        // Create info grid
        let info_grid = Grid::builder()
            .row_spacing(5)
            .column_spacing(10)
            .margin_top(5)
            .css_classes(vec!["audio-info-grid".to_string()])
            .build();
        
        // Create info labels
        let file_name_label = Label::builder()
            .label("No file loaded")
            .halign(Align::Start)
            .css_classes(vec!["audio-info-label".to_string()])
            .build();
        
        let format_label = Label::builder()
            .label("-")
            .halign(Align::Start)
            .css_classes(vec!["audio-info-value".to_string()])
            .build();
        
        let duration_label = Label::builder()
            .label("-")
            .halign(Align::Start)
            .css_classes(vec!["audio-info-value".to_string()])
            .build();
        
        let sample_rate_label = Label::builder()
            .label("-")
            .halign(Align::Start)
            .css_classes(vec!["audio-info-value".to_string()])
            .build();
        
        let channels_label = Label::builder()
            .label("-")
            .halign(Align::Start)
            .css_classes(vec!["audio-info-value".to_string()])
            .build();
        
        // Add labels to info grid
        info_grid.attach(&Label::builder().label("File:").halign(Align::Start).css_classes(vec!["audio-info-label".to_string()]).build(), 0, 0, 1, 1);
        info_grid.attach(&file_name_label, 1, 0, 1, 1);
        
        info_grid.attach(&Label::builder().label("Format:").halign(Align::Start).css_classes(vec!["audio-info-label".to_string()]).build(), 0, 1, 1, 1);
        info_grid.attach(&format_label, 1, 1, 1, 1);
        
        info_grid.attach(&Label::builder().label("Duration:").halign(Align::Start).css_classes(vec!["audio-info-label".to_string()]).build(), 0, 2, 1, 1);
        info_grid.attach(&duration_label, 1, 2, 1, 1);
        
        info_grid.attach(&Label::builder().label("Sample Rate:").halign(Align::Start).css_classes(vec!["audio-info-label".to_string()]).build(), 0, 3, 1, 1);
        info_grid.attach(&sample_rate_label, 1, 3, 1, 1);
        
        info_grid.attach(&Label::builder().label("Channels:").halign(Align::Start).css_classes(vec!["audio-info-label".to_string()]).build(), 0, 4, 1, 1);
        info_grid.attach(&channels_label, 1, 4, 1, 1);
        
        // Assemble the controls
        controls_box.append(&play_button);
        controls_box.append(&stop_button);
        controls_box.append(&Separator::new(Orientation::Vertical));
        controls_box.append(&position_scale);
        controls_box.append(&Separator::new(Orientation::Vertical));
        controls_box.append(&current_time_label);
        controls_box.append(&Label::builder().label("/").build());
        controls_box.append(&total_time_label);
        controls_box.append(&Separator::new(Orientation::Vertical));
        controls_box.append(&volume_label);
        controls_box.append(&volume_scale);
        controls_box.append(&mute_button);
        controls_box.append(&Separator::new(Orientation::Vertical));
        controls_box.append(&Label::builder().label("Speed:").build());
        controls_box.append(&speed_spin_button);
        controls_box.append(&Separator::new(Orientation::Vertical));
        controls_box.append(&Label::builder().label("Loop:").build());
        controls_box.append(&loop_toggle);
        
        // Add a separator
        let separator = Separator::builder()
            .orientation(Orientation::Horizontal)
            .margin_top(5)
            .margin_bottom(5)
            .build();
        
        // Add components to main container
        container.append(&controls_box);
        container.append(&separator);
        container.append(&info_grid);
        
        // Create the widget
        let widget = Self {
            container,
            app_window: None,
            current_file: Arc::new(Mutex::new(None)),
            playback_state: Arc::new(Mutex::new(PlaybackState::Stopped)),
            pipeline: Arc::new(Mutex::new(None)),
            bus: Arc::new(Mutex::new(None)),
            controls_box,
            play_button,
            stop_button,
            position_scale,
            position_adjustment,
            current_time_label,
            total_time_label,
            volume_scale,
            mute_button,
            volume_label,
            speed_spin_button,
            loop_toggle,
            info_grid,
            file_name_label,
            format_label,
            duration_label,
            sample_rate_label,
            channels_label,
            update_timeout_id: Arc::new(Mutex::new(None)),
        };
        
        // Set up event handlers
        widget.setup_events()?;
        
        Ok(widget)
    }
    
    /// Set up event handlers
    fn setup_events(&self) -> AppResult<()> {
        // Play/pause button
        let playback_state = self.playback_state.clone();
        let stop_button = self.stop_button.clone();
        let play_button = self.play_button.clone();
        let pipeline = self.pipeline.clone();
        let position_adjustment = self.position_adjustment.clone();
        
        self.play_button.connect_clicked(move |button| {
            let is_playing = button.is_active();
            
            if is_playing {
                // Start playing
                if let Ok(mut state_guard) = playback_state.lock() {
                    *state_guard = PlaybackState::Playing;
                }
                
                stop_button.set_sensitive(true);
                
                // Update button icon
                button.set_icon_name("media-playback-pause");
                
                // Start playback in GStreamer
                if let Ok(pipeline_guard) = pipeline.lock() {
                    if let Some(ref pipeline) = *pipeline_guard {
                        let _ = pipeline.set_state(gst::State::Playing);
                    }
                }
            } else {
                // Pause playing
                if let Ok(mut state_guard) = playback_state.lock() {
                    *state_guard = PlaybackState::Paused;
                }
                
                // Update button icon
                button.set_icon_name("media-playback-start");
                
                // Pause playback in GStreamer
                if let Ok(pipeline_guard) = pipeline.lock() {
                    if let Some(ref pipeline) = *pipeline_guard {
                        let _ = pipeline.set_state(gst::State::Paused);
                    }
                }
            }
        });
        
        // Stop button
        let playback_state = self.playback_state.clone();
        let play_button = self.play_button.clone();
        let pipeline = self.pipeline.clone();
        let position_adjustment = self.position_adjustment.clone();
        
        self.stop_button.connect_clicked(move |_| {
            // Stop playing
            if let Ok(mut state_guard) = playback_state.lock() {
                *state_guard = PlaybackState::Stopped;
            }
            
            play_button.set_active(false);
            play_button.set_icon_name("media-playback-start");
            position_adjustment.set_value(0.0);
            
            // Stop playback in GStreamer
            if let Ok(pipeline_guard) = pipeline.lock() {
                if let Some(ref pipeline) = *pipeline_guard {
                    let _ = pipeline.set_state(gst::State::Ready);
                }
            }
        });
        
        // Position scale
        let pipeline = self.pipeline.clone();
        
        self.position_scale.connect_value_changed(move |scale| {
            let position = scale.value() as f64 / 100.0;
            
            // Seek to the new position
            if let Ok(pipeline_guard) = pipeline.lock() {
                if let Some(ref pipeline) = *pipeline_guard {
                    // Get the duration
                    if let Ok(duration) = pipeline.query_duration::<gst::ClockTime>() {
                        let target_pos = duration.mul_scalar(position);
                        let _ = pipeline.seek_simple(
                            gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                            target_pos
                        );
                    }
                }
            }
        });
        
        // Volume scale
        let volume_label = self.volume_label.clone();
        let pipeline = self.pipeline.clone();
        
        self.volume_scale.connect_value_changed(move |scale| {
            let volume = scale.value();
            let volume_percent = (volume * 100.0) as i32;
            volume_label.set_text(&format!("{}%", volume_percent));
            
            // Set the volume in GStreamer
            if let Ok(pipeline_guard) = pipeline.lock() {
                if let Some(ref pipeline) = *pipeline_guard {
                    // Find the volume element and set its property
                    if let Some(volume_element) = pipeline.by_name("volume").and_then(|e| e.downcast::<gst::Element>().ok()) {
                        let _ = volume_element.set_property("volume", &volume);
                    }
                }
            }
        });
        
        // Mute button
        let mute_button = self.mute_button.clone();
        let volume_scale = self.volume_scale.clone();
        let pipeline = self.pipeline.clone();
        
        self.mute_button.connect_toggled(move |button| {
            let is_muted = button.is_active();
            
            // Update button icon
            if is_muted {
                button.set_icon_name("audio-volume-muted");
                volume_scale.set_sensitive(false);
            } else {
                button.set_icon_name("audio-volume-high");
                volume_scale.set_sensitive(true);
            }
            
            // Set the mute state in GStreamer
            if let Ok(pipeline_guard) = pipeline.lock() {
                if let Some(ref pipeline) = *pipeline_guard {
                    // Find the volume element and set its property
                    if let Some(volume_element) = pipeline.by_name("volume").and_then(|e| e.downcast::<gst::Element>().ok()) {
                        let _ = volume_element.set_property("mute", &is_muted);
                    }
                }
            }
        });
        
        // Playback speed
        let pipeline = self.pipeline.clone();
        
        self.speed_spin_button.connect_value_changed(move |spin_button| {
            let speed = spin_button.value();
            
            // Set the playback speed in GStreamer
            if let Ok(pipeline_guard) = pipeline.lock() {
                if let Some(ref pipeline) = *pipeline_guard {
                    // Find the rate element and set its property
                    if let Some(rate_element) = pipeline.by_name("rate").and_then(|e| e.downcast::<gst::Element>().ok()) {
                        let _ = rate_element.set_property("rate", &speed);
                    }
                }
            }
        });
        
        // Loop toggle
        let loop_toggle = self.loop_toggle.clone();
        let pipeline = self.pipeline.clone();
        
        self.loop_toggle.connect_state_changed(move |toggle| {
            let is_looping = toggle.is_active();
            
            // Set the loop state in GStreamer
            if let Ok(pipeline_guard) = pipeline.lock() {
                if let Some(ref pipeline) = *pipeline_guard {
                    // In a real implementation, you would handle looping differently
                    // GStreamer doesn't have a simple loop property, so you would
                    // need to implement this by listening for EOS messages and
                    // seeking back to the beginning
                }
            }
        });
        
        // Set up keyboard shortcuts
        self.setup_keyboard_shortcuts()?;
        
        Ok(())
    }
    
    /// Set up keyboard shortcuts
    fn setup_keyboard_shortcuts(&self) -> AppResult<()> {
        let key_controller = EventControllerKey::new();
        
        let play_button = self.play_button.clone();
        let stop_button = self.stop_button.clone();
        let mute_button = self.mute_button.clone();
        let loop_toggle = self.loop_toggle.clone();
        let volume_scale = self.volume_scale.clone();
        let position_scale = self.position_scale.clone();
        
        key_controller.connect_key_pressed(move |_, key, _, _| {
            match key {
                gdk::Key::space => {
                    // Toggle play/pause
                    play_button.clicked();
                    EventSequenceState::Claimed
                }
                gdk::Key::s | gdk::Key::S => {
                    // Stop
                    stop_button.clicked();
                    EventSequenceState::Claimed
                }
                gdk::Key::m | gdk::Key::M => {
                    // Toggle mute
                    mute_button.set_active(!mute_button.is_active());
                    EventSequenceState::Claimed
                }
                gdk::Key::l | gdk::Key::L => {
                    // Toggle loop
                    loop_toggle.set_active(!loop_toggle.is_active());
                    EventSequenceState::Claimed
                }
                gdk::Key::Left => {
                    // Seek backward 5 seconds
                    let current_pos = position_scale.value();
                    position_scale.set_value((current_pos - 5.0).max(0.0));
                    EventSequenceState::Claimed
                }
                gdk::Key::Right => {
                    // Seek forward 5 seconds
                    let current_pos = position_scale.value();
                    position_scale.set_value((current_pos + 5.0).min(100.0));
                    EventSequenceState::Claimed
                }
                gdk::Key::Up => {
                    // Increase volume
                    let current_volume = volume_scale.value();
                    volume_scale.set_value((current_volume + 0.1).min(1.0));
                    EventSequenceState::Claimed
                }
                gdk::Key::Down => {
                    // Decrease volume
                    let current_volume = volume_scale.value();
                    volume_scale.set_value((current_volume - 0.1).max(0.0));
                    EventSequenceState::Claimed
                }
                _ => EventSequenceState::None,
            }
        });
        
        self.container.add_controller(key_controller);
        
        Ok(())
    }
    
    /// Load an audio file for preview
    pub fn load_file(&self, audio_file: AudioFile) -> AppResult<()> {
        // Stop any current playback
        self.stop();
        
        // Update the current file
        if let Ok(mut current_file_guard) = self.current_file.lock() {
            *current_file_guard = Some(audio_file.clone());
        }
        
        // Update the UI
        self.file_name_label.set_text(&audio_file.file_name);
        
        // Extract and display audio metadata
        if let Ok(metadata) = AudioProcessor::extract_metadata(&audio_file.file_path) {
            self.format_label.set_text(&metadata.format);
            self.sample_rate_label.set_text(&format!("{} Hz", metadata.sample_rate));
            self.channels_label.set_text(&metadata.channels.to_string());
            
            let duration_secs = metadata.duration.as_secs();
            let minutes = duration_secs / 60;
            let seconds = duration_secs % 60;
            self.duration_label.set_text(&format!("{}:{:02}", minutes, seconds));
            self.total_time_label.set_text(&format!("{}:{:02}", minutes, seconds));
        }
        
        // Create GStreamer pipeline
        self.create_pipeline(&audio_file.file_path)?;
        
        // Enable playback controls
        self.play_button.set_sensitive(true);
        
        Ok(())
    }
    
    /// Create a GStreamer pipeline for audio playback
    fn create_pipeline(&self, file_path: &Path) -> AppResult<()> {
        // Create a new pipeline
        let pipeline_desc = format!(
            "filesrc location=\"{}\" ! decodebin ! audioconvert ! audioresample ! volume name=volume ! audioconvert ! audioresample ! audioconvert ! audioresample ! autoaudiosink",
            file_path.to_string_lossy()
        );
        
        let pipeline = gst::parse_launch(&pipeline_desc)
            .map_err(|e| AppError::generic(format!("Failed to create pipeline: {}", e)))?
            .downcast::<gst::Pipeline>()
            .map_err(|e| AppError::generic(format!("Pipeline is not a pipeline: {}", e)))?;
        
        // Set the pipeline to ready state
        pipeline.set_state(gst::State::Ready)
            .map_err(|e| AppError::generic(format!("Failed to set pipeline to ready state: {}", e)))?;
        
        // Connect to bus messages
        let bus = pipeline.bus().unwrap();
        let playback_state = self.playback_state.clone();
        let play_button = self.play_button.clone();
        let position_adjustment = self.position_adjustment.clone();
        let pipeline_clone = pipeline.clone();
        
        bus.add_watch(move |_, msg| {
            match msg.view() {
                gst::MessageView::Eos(..) => {
                    // End of stream
                    if let Ok(mut state_guard) = playback_state.lock() {
                        *state_guard = PlaybackState::Stopped;
                    }
                    
                    play_button.set_active(false);
                    play_button.set_icon_name("media-playback-start");
                    position_adjustment.set_value(0.0);
                    
                    // Check if looping is enabled
                    // In a real implementation, you would check the loop toggle state
                    // and seek back to the beginning if enabled
                }
                gst::MessageView::Error(err) => {
                    // Handle error
                    eprintln!("Error from GStreamer: {:?}", err);
                    
                    if let Ok(mut state_guard) = playback_state.lock() {
                        *state_guard = PlaybackState::Stopped;
                    }
                    
                    play_button.set_active(false);
                    play_button.set_icon_name("media-playback-start");
                }
                _ => (),
            }
            glib::ControlFlow::Continue
        });
        
        // Store the pipeline and bus
        if let Ok(mut pipeline_guard) = self.pipeline.lock() {
            *pipeline_guard = Some(pipeline.clone());
        }
        
        if let Ok(mut bus_guard) = self.bus.lock() {
            *bus_guard = Some(bus);
        }
        
        // Set up position update
        self.setup_position_update()?;
        
        Ok(())
    }
    
    /// Set up the position update timer
    fn setup_position_update(&self) -> AppResult<()> {
        let pipeline = self.pipeline.clone();
        let position_adjustment = self.position_adjustment.clone();
        let current_time_label = self.current_time_label.clone();
        let playback_state = self.playback_state.clone();
        
        // Clear any existing timeout
        if let Ok(mut timeout_id_guard) = self.update_timeout_id.lock() {
            if let Some(timeout_id) = *timeout_id_guard {
                glib::source_remove(timeout_id);
            }
            
            // Set up a new timeout
            let timeout_id = glib::timeout_add_seconds_local(1, move || {
                // Only update if playing
                if let Ok(state_guard) = playback_state.lock() {
                    if *state_guard == PlaybackState::Playing {
                        // Update position
                        if let Ok(pipeline_guard) = pipeline.lock() {
                            if let Some(ref pipeline) = *pipeline_guard {
                                if let Ok(position) = pipeline.query_position::<gst::ClockTime>() {
                                    if let Ok(duration) = pipeline.query_duration::<gst::ClockTime>() {
                                        if duration > gst::ClockTime::zero() {
                                            let position_percent = position.nseconds() as f64 / duration.nseconds() as f64 * 100.0;
                                            position_adjustment.set_value(position_percent);
                                            
                                            let position_secs = position.seconds() as u64;
                                            let minutes = position_secs / 60;
                                            let seconds = position_secs % 60;
                                            current_time_label.set_text(&format!("{}:{:02}", minutes, seconds));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                glib::ControlFlow::Continue
            });
            
            *timeout_id_guard = Some(timeout_id);
        }
        
        Ok(())
    }
    
    /// Start playback
    pub fn play(&self) {
        if !self.play_button.is_sensitive() {
            return;
        }
        
        self.play_button.set_active(true);
    }
    
    /// Pause playback
    pub fn pause(&self) {
        if !self.play_button.is_sensitive() {
            return;
        }
        
        self.play_button.set_active(false);
    }
    
    /// Stop playback
    pub fn stop(&self) {
        if !self.stop_button.is_sensitive() {
            return;
        }
        
        self.stop_button.clicked();
    }
    
    /// Toggle playback
    pub fn toggle_playback(&self) {
        if !self.play_button.is_sensitive() {
            return;
        }
        
        self.play_button.clicked();
    }
    
    /// Seek to a specific position (0.0 to 1.0)
    pub fn seek(&self, position: f64) {
        let clamped_position = position.clamp(0.0, 1.0);
        self.position_adjustment.set_value(clamped_position * 100.0);
    }
    
    /// Set the volume (0.0 to 1.0)
    pub fn set_volume(&self, volume: f64) {
        let clamped_volume = volume.clamp(0.0, 1.0);
        self.volume_scale.set_value(clamped_volume);
    }
    
    /// Get the current volume (0.0 to 1.0)
    pub fn get_volume(&self) -> f64 {
        self.volume_scale.value()
    }
    
    /// Set the mute state
    pub fn set_muted(&self, muted: bool) {
        self.mute_button.set_active(muted);
    }
    
    /// Get the mute state
    pub fn is_muted(&self) -> bool {
        self.mute_button.is_active()
    }
    
    /// Set the playback speed
    pub fn set_speed(&self, speed: f64) {
        self.speed_spin_button.set_value(speed);
    }
    
    /// Get the playback speed
    pub fn get_speed(&self) -> f64 {
        self.speed_spin_button.value()
    }
    
    /// Set the loop state
    pub fn set_looping(&self, looping: bool) {
        self.loop_toggle.set_active(looping);
    }
    
    /// Get the loop state
    pub fn is_looping(&self) -> bool {
        self.loop_toggle.is_active()
    }
    
    /// Get the current playback state
    pub fn get_playback_state(&self) -> PlaybackState {
        if let Ok(state_guard) = self.playback_state.lock() {
            *state_guard
        } else {
            PlaybackState::Stopped
        }
    }
    
    /// Get the current position (0.0 to 1.0)
    pub fn get_position(&self) -> f64 {
        self.position_adjustment.value() / 100.0
    }
    
    /// Set the application window for dialogs
    pub fn set_application_window(&mut self, window: ApplicationWindow) {
        self.app_window = Some(window);
    }
    
    /// Clear the current file
    pub fn clear(&self) {
        // Stop playback
        self.stop();
        
        // Clear the pipeline
        if let Ok(mut pipeline_guard) = self.pipeline.lock() {
            if let Some(ref pipeline) = *pipeline_guard {
                let _ = pipeline.set_state(gst::State::Null);
            }
            *pipeline_guard = None;
        }
        
        // Clear the bus
        if let Ok(mut bus_guard) = self.bus.lock() {
            *bus_guard = None;
        }
        
        // Clear the timeout
        if let Ok(mut timeout_id_guard) = self.update_timeout_id.lock() {
            if let Some(timeout_id) = *timeout_id_guard {
                glib::source_remove(timeout_id);
                *timeout_id_guard = None;
            }
        }
        
        // Clear the current file
        if let Ok(mut current_file_guard) = self.current_file.lock() {
            *current_file_guard = None;
        }
        
        // Reset the UI
        self.file_name_label.set_text("No file loaded");
        self.format_label.set_text("-");
        self.duration_label.set_text("-");
        self.sample_rate_label.set_text("-");
        self.channels_label.set_text("-");
        self.current_time_label.set_text("00:00");
        self.total_time_label.set_text("00:00");
        self.position_adjustment.set_value(0.0);
        
        // Disable playback controls
        self.play_button.set_sensitive(false);
        self.stop_button.set_sensitive(false);
    }
    
    /// Get the main container widget
    pub fn get_widget(&self) -> &Box {
        &self.container
    }
}

impl Drop for AudioPreviewWidget {
    fn drop(&mut self) {
        // Clean up GStreamer resources
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk4::Application;
    use std::path::PathBuf;

    #[test]
    fn test_audio_preview_widget_creation() {
        // Initialize GTK
        gtk4::init().expect("Failed to initialize GTK");
        
        // Create a new audio preview widget
        let widget = AudioPreviewWidget::new();
        assert!(widget.is_ok());
    }

    #[test]
    fn test_playback_state() {
        assert_eq!(PlaybackState::Stopped, PlaybackState::Stopped);
        assert_ne!(PlaybackState::Playing, PlaybackState::Stopped);
    }

    #[test]
    fn test_position_clamping() {
        let position = 1.5; // Above 1.0
        let clamped = position.clamp(0.0, 1.0);
        assert_eq!(clamped, 1.0);
        
        let position = -0.5; // Below 0.0
        let clamped = position.clamp(0.0, 1.0);
        assert_eq!(clamped, 0.0);
    }

    #[test]
    fn test_volume_clamping() {
        let volume = 1.5; // Above 1.0
        let clamped = volume.clamp(0.0, 1.0);
        assert_eq!(clamped, 1.0);
        
        let volume = -0.5; // Below 0.0
        let clamped = volume.clamp(0.0, 1.0);
        assert_eq!(clamped, 0.0);
    }
}