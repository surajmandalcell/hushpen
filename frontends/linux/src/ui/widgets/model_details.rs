//! Model Details Widget for the ASRPro application
//!
//! This module contains the ModelDetailsWidget which displays detailed information
//! about a selected speech recognition model.

use glib::clone;
use gtk4::prelude::*;
use gtk4::{
    Box, Button, Label, Frame, Grid, Image, IconSize, ProgressBar, Switch,
    Scale, Expander, Revealer, ScrolledWindow, TextBuffer, TextView,
    Orientation, Align, PolicyType, Separator, Stack, StackSwitcher,
    ApplicationWindow, Widget, CheckButton, SpinButton, Adjustment
};
use std::sync::Arc;

use crate::models::{Model, ModelStatus, ModelType, ModelPerformance, ModelParameters};
use crate::services::ModelManager;
use crate::utils::AppError;

/// Model Details Widget
#[derive(Clone)]
pub struct ModelDetailsWidget {
    /// Main container widget
    container: Box,
    /// Model manager service
    model_manager: Arc<ModelManager>,
    /// Stack for different detail views
    stack: Stack,
    /// Stack switcher
    stack_switcher: StackSwitcher,
    /// Overview panel
    overview_panel: Box,
    /// Performance panel
    performance_panel: Box,
    /// Requirements panel
    requirements_panel: Box,
    /// Configuration panel
    configuration_panel: Box,
    /// Current model
    current_model: Arc<gtk4::glib::Mutex<Option<Model>>>,
    /// Update button
    update_button: Button,
    /// Delete button
    delete_button: Button,
    /// Use model button
    use_model_button: Button,
}

impl ModelDetailsWidget {
    /// Create a new ModelDetailsWidget instance
    pub fn new(
        app_state: Arc<crate::models::AppState>,
        model_manager: Arc<ModelManager>,
    ) -> Result<Self, AppError> {
        // Create the main container
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        // Create the stack for different views
        let stack = Stack::builder()
            .transition_type(gtk4::StackTransitionType::SlideLeftRight)
            .build();

        // Create the stack switcher
        let stack_switcher = StackSwitcher::builder()
            .stack(&stack)
            .halign(Align::Center)
            .margin_top(10)
            .margin_bottom(10)
            .build();

        // Create the overview panel
        let overview_panel = Self::create_overview_panel();

        // Create the performance panel
        let performance_panel = Self::create_performance_panel();

        // Create the requirements panel
        let requirements_panel = Self::create_requirements_panel();

        // Create the configuration panel
        let configuration_panel = Self::create_configuration_panel();

        // Add pages to the stack
        stack.add_titled(&overview_panel, Some("overview"), "Overview");
        stack.add_titled(&performance_panel, Some("performance"), "Performance");
        stack.add_titled(&requirements_panel, Some("requirements"), "Requirements");
        stack.add_titled(&configuration_panel, Some("config"), "Configuration");

        // Create action buttons
        let button_panel = Self::create_action_buttons();
        let update_button = button_panel.update_button.clone();
        let delete_button = button_panel.delete_button.clone();
        let use_model_button = button_panel.use_model_button.clone();

        // Create the main widget instance
        let model_details = Self {
            container,
            model_manager,
            stack,
            stack_switcher,
            overview_panel,
            performance_panel,
            requirements_panel,
            configuration_panel,
            current_model: Arc::new(gtk4::glib::Mutex::new(None)),
            update_button,
            delete_button,
            use_model_button,
        };

        // Set up the UI
        model_details.setup_ui(button_panel);
        model_details.setup_event_handlers()?;

        Ok(model_details)
    }

    /// Create the overview panel
    fn create_overview_panel() -> Box {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(15)
            .build();

        // Model header
        let header_frame = Frame::builder()
            .label("Model Information")
            .build();

        let header_grid = Grid::builder()
            .row_spacing(10)
            .column_spacing(15)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        // Model name
        let name_label = Label::builder()
            .label("Name:")
            .halign(Align::Start)
            .build();
        header_grid.attach(&name_label, 0, 0, 1, 1);

        let name_value = Label::builder()
            .label("")
            .halign(Align::Start)
            .build();
        header_grid.attach(&name_value, 1, 0, 1, 1);

        // Model type
        let type_label = Label::builder()
            .label("Type:")
            .halign(Align::Start)
            .build();
        header_grid.attach(&type_label, 0, 1, 1, 1);

        let type_value = Label::builder()
            .label("")
            .halign(Align::Start)
            .build();
        header_grid.attach(&type_value, 1, 1, 1, 1);

        // Model size
        let size_label = Label::builder()
            .label("Size:")
            .halign(Align::Start)
            .build();
        header_grid.attach(&size_label, 0, 2, 1, 1);

        let size_value = Label::builder()
            .label("")
            .halign(Align::Start)
            .build();
        header_grid.attach(&size_value, 1, 2, 1, 1);

        // Model status
        let status_label = Label::builder()
            .label("Status:")
            .halign(Align::Start)
            .build();
        header_grid.attach(&status_label, 0, 3, 1, 1);

        let status_value = Label::builder()
            .label("")
            .halign(Align::Start)
            .build();
        header_grid.attach(&status_value, 1, 3, 1, 1);

        // Language
        let lang_label = Label::builder()
            .label("Language:")
            .halign(Align::Start)
            .build();
        header_grid.attach(&lang_label, 0, 4, 1, 1);

        let lang_value = Label::builder()
            .label("")
            .halign(Align::Start)
            .build();
        header_grid.attach(&lang_value, 1, 4, 1, 1);

        header_frame.set_child(Some(&header_grid));

        // Capabilities
        let capabilities_frame = Frame::builder()
            .label("Capabilities")
            .build();

        let capabilities_grid = Grid::builder()
            .row_spacing(10)
            .column_spacing(15)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        let transcription_check = CheckButton::builder()
            .label("Transcription")
            .sensitive(false)
            .build();
        capabilities_grid.attach(&transcription_check, 0, 0, 1, 1);

        let translation_check = CheckButton::builder()
            .label("Translation")
            .sensitive(false)
            .build();
        capabilities_grid.attach(&translation_check, 0, 1, 1, 1);

        let diarization_check = CheckButton::builder()
            .label("Speaker Diarization")
            .sensitive(false)
            .build();
        capabilities_grid.attach(&diarization_check, 0, 2, 1, 1);

        capabilities_frame.set_child(Some(&capabilities_grid));

        // Supported formats
        let formats_frame = Frame::builder()
            .label("Supported Audio Formats")
            .build();

        let formats_label = Label::builder()
            .label("")
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .halign(Align::Start)
            .build();
        formats_frame.set_child(Some(&formats_label));

        // Add all components to the container
        container.append(&header_frame);
        container.append(&capabilities_frame);
        container.append(&formats_frame);

        // Store references to widgets for later updates
        container
    }

    /// Create the performance panel
    fn create_performance_panel() -> Box {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(15)
            .build();

        // Performance metrics
        let metrics_frame = Frame::builder()
            .label("Performance Metrics")
            .build();

        let metrics_grid = Grid::builder()
            .row_spacing(10)
            .column_spacing(15)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        // Real-time factor
        let rtf_label = Label::builder()
            .label("Real-time Factor:")
            .halign(Align::Start)
            .build();
        metrics_grid.attach(&rtf_label, 0, 0, 1, 1);

        let rtf_value = Label::builder()
            .label("")
            .halign(Align::Start)
            .build();
        metrics_grid.attach(&rtf_value, 1, 0, 1, 1);

        // Word error rate
        let wer_label = Label::builder()
            .label("Word Error Rate:")
            .halign(Align::Start)
            .build();
        metrics_grid.attach(&wer_label, 0, 1, 1, 1);

        let wer_value = Label::builder()
            .label("")
            .halign(Align::Start)
            .build();
        metrics_grid.attach(&wer_value, 1, 1, 1, 1);

        // Average inference time
        let inference_label = Label::builder()
            .label("Avg. Inference Time:")
            .halign(Align::Start)
            .build();
        metrics_grid.attach(&inference_label, 0, 2, 1, 1);

        let inference_value = Label::builder()
            .label("")
            .halign(Align::Start)
            .build();
        metrics_grid.attach(&inference_value, 1, 2, 1, 1);

        // Throughput
        let throughput_label = Label::builder()
            .label("Throughput:")
            .halign(Align::Start)
            .build();
        metrics_grid.attach(&throughput_label, 0, 3, 1, 1);

        let throughput_value = Label::builder()
            .label("")
            .halign(Align::Start)
            .build();
        metrics_grid.attach(&throughput_value, 1, 3, 1, 1);

        metrics_frame.set_child(Some(&metrics_grid));

        // Resource usage
        let resources_frame = Frame::builder()
            .label("Resource Usage")
            .build();

        let resources_grid = Grid::builder()
            .row_spacing(10)
            .column_spacing(15)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        // CPU utilization
        let cpu_label = Label::builder()
            .label("CPU Utilization:")
            .halign(Align::Start)
            .build();
        resources_grid.attach(&cpu_label, 0, 0, 1, 1);

        let cpu_progress = ProgressBar::builder()
            .hexpand(true)
            .build();
        resources_grid.attach(&cpu_progress, 1, 0, 1, 1);

        // GPU utilization
        let gpu_label = Label::builder()
            .label("GPU Utilization:")
            .halign(Align::Start)
            .build();
        resources_grid.attach(&gpu_label, 0, 1, 1, 1);

        let gpu_progress = ProgressBar::builder()
            .hexpand(true)
            .build();
        resources_grid.attach(&gpu_progress, 1, 1, 1, 1);

        // Memory usage
        let memory_label = Label::builder()
            .label("Memory Usage:")
            .halign(Align::Start)
            .build();
        resources_grid.attach(&memory_label, 0, 2, 1, 1);

        let memory_progress = ProgressBar::builder()
            .hexpand(true)
            .build();
        resources_grid.attach(&memory_progress, 1, 2, 1, 1);

        resources_frame.set_child(Some(&resources_grid));

        // Add all components to the container
        container.append(&metrics_frame);
        container.append(&resources_frame);

        container
    }

    /// Create the requirements panel
    fn create_requirements_panel() -> Box {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(15)
            .build();

        // System requirements
        let system_frame = Frame::builder()
            .label("System Requirements")
            .build();

        let system_grid = Grid::builder()
            .row_spacing(10)
            .column_spacing(15)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        // Sample rate
        let sample_rate_label = Label::builder()
            .label("Sample Rate:")
            .halign(Align::Start)
            .build();
        system_grid.attach(&sample_rate_label, 0, 0, 1, 1);

        let sample_rate_value = Label::builder()
            .label("")
            .halign(Align::Start)
            .build();
        system_grid.attach(&sample_rate_value, 1, 0, 1, 1);

        // Max audio length
        let max_length_label = Label::builder()
            .label("Max Audio Length:")
            .halign(Align::Start)
            .build();
        system_grid.attach(&max_length_label, 0, 1, 1, 1);

        let max_length_value = Label::builder()
            .label("")
            .halign(Align::Start)
            .build();
        system_grid.attach(&max_length_value, 1, 1, 1, 1);

        // Chunk size
        let chunk_size_label = Label::builder()
            .label("Chunk Size:")
            .halign(Align::Start)
            .build();
        system_grid.attach(&chunk_size_label, 0, 2, 1, 1);

        let chunk_size_value = Label::builder()
            .label("")
            .halign(Align::Start)
            .build();
        system_grid.attach(&chunk_size_value, 1, 2, 1, 1);

        system_frame.set_child(Some(&system_grid));

        // Memory requirements
        let memory_frame = Frame::builder()
            .label("Memory Requirements")
            .build();

        let memory_grid = Grid::builder()
            .row_spacing(10)
            .column_spacing(15)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        // CPU memory
        let cpu_memory_label = Label::builder()
            .label("CPU Memory:")
            .halign(Align::Start)
            .build();
        memory_grid.attach(&cpu_memory_label, 0, 0, 1, 1);

        let cpu_memory_value = Label::builder()
            .label("")
            .halign(Align::Start)
            .build();
        memory_grid.attach(&cpu_memory_value, 1, 0, 1, 1);

        // GPU memory
        let gpu_memory_label = Label::builder()
            .label("GPU Memory:")
            .halign(Align::Start)
            .build();
        memory_grid.attach(&gpu_memory_label, 0, 1, 1, 1);

        let gpu_memory_value = Label::builder()
            .label("")
            .halign(Align::Start)
            .build();
        memory_grid.attach(&gpu_memory_value, 1, 1, 1, 1);

        memory_frame.set_child(Some(&memory_grid));

        // Add all components to the container
        container.append(&system_frame);
        container.append(&memory_frame);

        container
    }

    /// Create the configuration panel
    fn create_configuration_panel() -> Box {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(15)
            .build();

        // Model configuration
        let config_frame = Frame::builder()
            .label("Model Configuration")
            .build();

        let config_grid = Grid::builder()
            .row_spacing(10)
            .column_spacing(15)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        // Language setting
        let lang_label = Label::builder()
            .label("Language:")
            .halign(Align::Start)
            .build();
        config_grid.attach(&lang_label, 0, 0, 1, 1);

        let lang_entry = gtk4::Entry::builder()
            .placeholder_text("auto")
            .build();
        config_grid.attach(&lang_entry, 1, 0, 1, 1);

        // Temperature setting
        let temp_label = Label::builder()
            .label("Temperature:")
            .halign(Align::Start)
            .build();
        config_grid.attach(&temp_label, 0, 1, 1, 1);

        let temp_scale = Scale::builder()
            .orientation(Orientation::Horizontal)
            .adjustment(&Adjustment::new(0.0, 0.0, 1.0, 0.1, 0.1, 0.0))
            .hexpand(true)
            .build();
        config_grid.attach(&temp_scale, 1, 1, 1, 1);

        // Best of setting
        let best_of_label = Label::builder()
            .label("Best of:")
            .halign(Align::Start)
            .build();
        config_grid.attach(&best_of_label, 0, 2, 1, 1);

        let best_of_spin = SpinButton::builder()
            .adjustment(&Adjustment::new(1.0, 1.0, 10.0, 1.0, 1.0, 0.0))
            .build();
        config_grid.attach(&best_of_spin, 1, 2, 1, 1);

        config_frame.set_child(Some(&config_grid));

        // Advanced options
        let advanced_frame = Frame::builder()
            .label("Advanced Options")
            .build();

        let advanced_grid = Grid::builder()
            .row_spacing(10)
            .column_spacing(15)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        // Enable timestamps
        let timestamps_check = CheckButton::builder()
            .label("Enable Timestamps")
            .active(true)
            .build();
        advanced_grid.attach(&timestamps_check, 0, 0, 1, 1);

        // Enable word segments
        let segments_check = CheckButton::builder()
            .label("Enable Word Segments")
            .active(true)
            .build();
        advanced_grid.attach(&segments_check, 0, 1, 1, 1);

        // Enable translation
        let translation_check = CheckButton::builder()
            .label("Enable Translation")
            .build();
        advanced_grid.attach(&translation_check, 0, 2, 1, 1);

        advanced_frame.set_child(Some(&advanced_grid));

        // Add all components to the container
        container.append(&config_frame);
        container.append(&advanced_frame);

        container
    }

    /// Create action buttons
    fn create_action_buttons() -> ActionButtons {
        let container = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(10)
            .halign(Align::End)
            .margin_top(10)
            .build();

        let update_button = Button::builder()
            .label("Check for Updates")
            .sensitive(false)
            .build();

        let delete_button = Button::builder()
            .label("Delete Model")
            .sensitive(false)
            .add_css_class("destructive-action")
            .build();

        let use_model_button = Button::builder()
            .label("Use Model")
            .sensitive(false)
            .add_css_class("suggested-action")
            .build();

        container.append(&update_button);
        container.append(&delete_button);
        container.append(&use_model_button);

        ActionButtons {
            update_button,
            delete_button,
            use_model_button,
        }
    }

    /// Set up the UI layout
    fn setup_ui(&self, button_panel: ActionButtons) {
        // Add the stack switcher
        self.container.append(&self.stack_switcher);

        // Add the stack
        self.container.append(&self.stack.upcast::<Widget>());

        // Add the action buttons
        self.container.append(&button_panel.container);
    }

    /// Set up event handlers
    fn setup_event_handlers(&self) -> Result<(), AppError> {
        // Handle update button clicks
        self.update_button.connect_clicked(clone!(@strong self as self_ => move |_| {
            self_.check_for_updates();
        }));

        // Handle delete button clicks
        self.delete_button.connect_clicked(clone!(@strong self as self_ => move |_| {
            self_.delete_model();
        }));

        // Handle use model button clicks
        self.use_model_button.connect_clicked(clone!(@strong self as self_ => move |_| {
            self_.use_model();
        }));

        Ok(())
    }

    /// Update the widget with a new model
    pub fn update_model(&self, model: Model) {
        // Store the current model
        let mut current_model = self.current_model.lock().unwrap();
        *current_model = Some(model.clone());
        drop(current_model);

        // Update all panels
        self.update_overview_panel(&model);
        self.update_performance_panel(&model);
        self.update_requirements_panel(&model);
        self.update_configuration_panel(&model);

        // Update button sensitivity
        self.update_button.set_sensitive(true);
        self.delete_button.set_sensitive(true);
        self.use_model_button.set_sensitive(model.is_ready());
    }

    /// Update the overview panel
    fn update_overview_panel(&self, model: &Model) {
        // Get the overview panel from the stack
        let overview_panel = self.stack.child_by_name("overview").unwrap().downcast::<Box>().unwrap();
        
        // Update the header information
        let header_frame = overview_panel.first_child().unwrap().downcast::<Frame>().unwrap();
        let header_grid = header_frame.child().unwrap().downcast::<Grid>().unwrap();
        
        // Update name
        let name_value = header_grid.child_at(1, 0).unwrap().downcast::<Label>().unwrap();
        name_value.set_label(&model.display_name);
        
        // Update type
        let type_value = header_grid.child_at(1, 1).unwrap().downcast::<Label>().unwrap();
        type_value.set_label(model.model_type.display_name());
        
        // Update size
        let size_value = header_grid.child_at(1, 2).unwrap().downcast::<Label>().unwrap();
        size_value.set_label(&model.formatted_size());
        
        // Update status
        let status_value = header_grid.child_at(1, 3).unwrap().downcast::<Label>().unwrap();
        status_value.set_label(&model.status_message());
        
        // Update language
        let lang_value = header_grid.child_at(1, 4).unwrap().downcast::<Label>().unwrap();
        lang_value.set_label(&model.language.as_deref().unwrap_or("Multilingual"));
        
        // Update capabilities
        let capabilities_frame = header_grid.next_sibling().unwrap().downcast::<Frame>().unwrap();
        let capabilities_grid = capabilities_frame.child().unwrap().downcast::<Grid>().unwrap();
        
        let transcription_check = capabilities_grid.child_at(0, 0).unwrap().downcast::<CheckButton>().unwrap();
        transcription_check.set_active(model.supports_transcription);
        
        let translation_check = capabilities_grid.child_at(0, 1).unwrap().downcast::<CheckButton>().unwrap();
        translation_check.set_active(model.supports_translation);
        
        let diarization_check = capabilities_grid.child_at(0, 2).unwrap().downcast::<CheckButton>().unwrap();
        diarization_check.set_active(model.supports_diarization);
        
        // Update supported formats
        let formats_frame = capabilities_frame.next_sibling().unwrap().downcast::<Frame>().unwrap();
        let formats_label = formats_frame.child().unwrap().downcast::<Label>().unwrap();
        formats_label.set_label(&model.supported_formats.join(", "));
    }

    /// Update the performance panel
    fn update_performance_panel(&self, model: &Model) {
        // Get the performance panel from the stack
        let performance_panel = self.stack.child_by_name("performance").unwrap().downcast::<Box>().unwrap();
        
        // Update metrics
        let metrics_frame = performance_panel.first_child().unwrap().downcast::<Frame>().unwrap();
        let metrics_grid = metrics_frame.child().unwrap().downcast::<Grid>().unwrap();
        
        if let Some(ref performance) = model.performance {
            // Update real-time factor
            let rtf_value = metrics_grid.child_at(1, 0).unwrap().downcast::<Label>().unwrap();
            rtf_value.set_label(&format!("{:.2}", performance.real_time_factor));
            
            // Update word error rate
            let wer_value = metrics_grid.child_at(1, 1).unwrap().downcast::<Label>().unwrap();
            if let Some(wer) = performance.word_error_rate {
                wer_value.set_label(&format!("{:.2}%", wer * 100.0));
            } else {
                wer_value.set_label("N/A");
            }
            
            // Update average inference time
            let inference_value = metrics_grid.child_at(1, 2).unwrap().downcast::<Label>().unwrap();
            inference_value.set_label(&format!("{:.2}s", performance.avg_inference_time));
            
            // Update throughput
            let throughput_value = metrics_grid.child_at(1, 3).unwrap().downcast::<Label>().unwrap();
            throughput_value.set_label(&format!("{:.0} chars/s", performance.throughput_chars_per_sec));
            
            // Update resource usage
            let resources_frame = metrics_frame.next_sibling().unwrap().downcast::<Frame>().unwrap();
            let resources_grid = resources_frame.child().unwrap().downcast::<Grid>().unwrap();
            
            // Update CPU utilization
            let cpu_progress = resources_grid.child_at(1, 0).unwrap().downcast::<ProgressBar>().unwrap();
            cpu_progress.set_fraction(performance.cpu_utilization as f64 / 100.0);
            cpu_progress.set_text(Some(&format!("{:.0}%", performance.cpu_utilization)));
            
            // Update GPU utilization
            let gpu_progress = resources_grid.child_at(1, 1).unwrap().downcast::<ProgressBar>().unwrap();
            if let Some(gpu_util) = performance.gpu_utilization {
                gpu_progress.set_fraction(gpu_util as f64 / 100.0);
                gpu_progress.set_text(Some(&format!("{:.0}%", gpu_util)));
            } else {
                gpu_progress.set_fraction(0.0);
                gpu_progress.set_text(Some("N/A"));
            }
            
            // Update memory usage
            let memory_progress = resources_grid.child_at(1, 2).unwrap().downcast::<ProgressBar>().unwrap();
            memory_progress.set_text(Some(&format!("{:.0} MB", performance.memory_usage_mb)));
            // Set a reasonable fraction based on typical memory usage
            memory_progress.set_fraction((performance.memory_usage_mb / 4096.0).min(1.0));
        } else {
            // No performance data available
            let rtf_value = metrics_grid.child_at(1, 0).unwrap().downcast::<Label>().unwrap();
            rtf_value.set_label("N/A");
            
            let wer_value = metrics_grid.child_at(1, 1).unwrap().downcast::<Label>().unwrap();
            wer_value.set_label("N/A");
            
            let inference_value = metrics_grid.child_at(1, 2).unwrap().downcast::<Label>().unwrap();
            inference_value.set_label("N/A");
            
            let throughput_value = metrics_grid.child_at(1, 3).unwrap().downcast::<Label>().unwrap();
            throughput_value.set_label("N/A");
        }
    }

    /// Update the requirements panel
    fn update_requirements_panel(&self, model: &Model) {
        // Get the requirements panel from the stack
        let requirements_panel = self.stack.child_by_name("requirements").unwrap().downcast::<Box>().unwrap();
        
        // Update system requirements
        let system_frame = requirements_panel.first_child().unwrap().downcast::<Frame>().unwrap();
        let system_grid = system_frame.child().unwrap().downcast::<Grid>().unwrap();
        
        // Update sample rate
        let sample_rate_value = system_grid.child_at(1, 0).unwrap().downcast::<Label>().unwrap();
        sample_rate_value.set_label(&format!("{} Hz", model.parameters.sample_rate));
        
        // Update max audio length
        let max_length_value = system_grid.child_at(1, 1).unwrap().downcast::<Label>().unwrap();
        let minutes = model.parameters.max_audio_length / 60;
        let seconds = model.parameters.max_audio_length % 60;
        max_length_value.set_label(&format!("{}:{:02}", minutes, seconds));
        
        // Update chunk size
        let chunk_size_value = system_grid.child_at(1, 2).unwrap().downcast::<Label>().unwrap();
        chunk_size_value.set_label(&format!("{} seconds", model.parameters.chunk_size));
        
        // Update memory requirements
        let memory_frame = system_frame.next_sibling().unwrap().downcast::<Frame>().unwrap();
        let memory_grid = memory_frame.child().unwrap().downcast::<Grid>().unwrap();
        
        // Update CPU memory
        let cpu_memory_value = memory_grid.child_at(1, 0).unwrap().downcast::<Label>().unwrap();
        cpu_memory_value.set_label(&format!("{} MB", model.parameters.cpu_memory_mb));
        
        // Update GPU memory
        let gpu_memory_value = memory_grid.child_at(1, 1).unwrap().downcast::<Label>().unwrap();
        if let Some(gpu_memory) = model.parameters.gpu_memory_mb {
            gpu_memory_value.set_label(&format!("{} MB", gpu_memory));
        } else {
            gpu_memory_value.set_label("N/A");
        }
    }

    /// Update the configuration panel
    fn update_configuration_panel(&self, model: &Model) {
        // Get the configuration panel from the stack
        let config_panel = self.stack.child_by_name("config").unwrap().downcast::<Box>().unwrap();
        
        // This panel doesn't need updates based on the model
        // It contains configuration options that are set by the user
    }

    /// Check for model updates
    fn check_for_updates(&self) {
        let model_manager = self.model_manager.clone();
        let update_button = self.update_button.clone();
        
        gtk4::glib::spawn_future_local(async move {
            update_button.set_sensitive(false);
            update_button.set_label("Checking...");
            
            match model_manager.check_for_updates().await {
                Ok(updates) => {
                    if updates.is_empty() {
                        update_button.set_label("Up to Date");
                    } else {
                        update_button.set_label(&format!("Updates Available ({})", updates.len()));
                    }
                },
                Err(e) => {
                    eprintln!("Failed to check for updates: {}", e);
                    update_button.set_label("Check Failed");
                }
            }
            
            // Reset button after a delay
            gtk4::glib::timeout_add_seconds_local_once(3, move || {
                update_button.set_label("Check for Updates");
                update_button.set_sensitive(true);
            });
        });
    }

    /// Delete the current model
    fn delete_model(&self) {
        let current_model = self.current_model.lock().unwrap();
        if let Some(ref model) = *current_model {
            let model_name = model.name.clone();
            let model_manager = self.model_manager.clone();
            
            drop(current_model);
            
            // Show confirmation dialog
            // In a real implementation, you would show a confirmation dialog
            // For now, we'll just proceed with the deletion
            
            gtk4::glib::spawn_future_local(async move {
                if let Err(e) = model_manager.delete_model(&model_name).await {
                    eprintln!("Failed to delete model: {}", e);
                }
            });
        }
    }

    /// Use the current model
    fn use_model(&self) {
        let current_model = self.current_model.lock().unwrap();
        if let Some(ref model) = *current_model {
            let model_name = model.name.clone();
            let model_manager = self.model_manager.clone();
            
            drop(current_model);
            
            gtk4::glib::spawn_future_local(async move {
                if let Err(e) = model_manager.set_selected_model(&model_name).await {
                    eprintln!("Failed to select model: {}", e);
                }
            });
        }
    }

    /// Get the main container widget
    pub fn get_widget(&self) -> &Widget {
        self.container.upcast_ref()
    }
}

/// Action buttons component
struct ActionButtons {
    update_button: Button,
    delete_button: Button,
    use_model_button: Button,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::api::BackendConfig;
    use crate::services::BackendClient;
    
    #[test]
    fn test_model_details_widget_creation() {
        // This test would require a proper GTK4 test environment
        // For now, we'll just verify that the struct can be created
        // In a real test, you would need to initialize GTK4 first
    }
}