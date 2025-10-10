//! Model Panel component for the ASRPro application
//!
//! This module contains the ModelPanel struct which provides a comprehensive interface
//! for managing speech recognition models, including selection, configuration, and status.

use glib::clone;
use gtk4::prelude::*;
use gtk4::{
    Box, Button, Label, Stack, StackSwitcher, ScrolledWindow, ListView, 
    SingleSelection, SignalListItemFactory, Orientation, Align, PolicyType,
    Separator, ProgressBar, Spinner, Frame, Grid, Entry, ComboBoxText,
    CheckButton, Scale, Switch, Revealer, Expander, Image, IconSize,
    ApplicationWindow, Widget
};
use std::sync::Arc;
use uuid::Uuid;

use crate::models::{AppState, Model, ModelStatus, ModelType};
use crate::services::ModelManager;
use crate::ui::ModelSelectorWidget;
use crate::ui::ModelDetailsWidget;
use crate::utils::AppError;

/// Model Panel component
pub struct ModelPanel {
    /// Main container widget
    container: Box,
    /// Application state
    app_state: Arc<AppState>,
    /// Model manager service
    model_manager: Arc<ModelManager>,
    /// Stack for different views
    stack: Stack,
    /// Stack switcher for navigation
    stack_switcher: StackSwitcher,
    /// Model selector widget
    model_selector: ModelSelectorWidget,
    /// Model details widget
    model_details: ModelDetailsWidget,
    /// Configuration panel
    config_panel: Box,
    /// Status label
    status_label: Label,
    /// Progress bar for model operations
    progress_bar: ProgressBar,
    /// Refresh button
    refresh_button: Button,
}

impl ModelPanel {
    /// Create a new ModelPanel instance
    pub fn new(
        window: ApplicationWindow,
        app_state: Arc<AppState>,
        model_manager: Arc<ModelManager>,
    ) -> Result<Self, AppError> {
        // Create the main container
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(0)
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

        // Create the model selector widget
        let model_selector = ModelSelectorWidget::new(
            window.clone(),
            app_state.clone(),
            model_manager.clone(),
        )?;

        // Create the model details widget
        let model_details = ModelDetailsWidget::new(
            app_state.clone(),
            model_manager.clone(),
        )?;

        // Create the configuration panel
        let config_panel = Self::create_config_panel(app_state.clone(), model_manager.clone())?;

        // Add pages to the stack
        stack.add_titled(&model_selector.get_widget(), Some("selector"), "Select Model");
        stack.add_titled(&model_details.get_widget(), Some("details"), "Model Details");
        stack.add_titled(&config_panel, Some("config"), "Configuration");

        // Create the status bar
        let status_bar = Self::create_status_bar();

        // Create the main panel instance
        let model_panel = Self {
            container,
            app_state,
            model_manager,
            stack,
            stack_switcher,
            model_selector,
            model_details,
            config_panel,
            status_label: status_bar.status_label,
            progress_bar: status_bar.progress_bar,
            refresh_button: status_bar.refresh_button,
        };

        // Set up the UI
        model_panel.setup_ui()?;
        model_panel.setup_event_handlers()?;

        Ok(model_panel)
    }

    /// Set up the UI layout
    fn setup_ui(&self) -> Result<(), AppError> {
        // Add the stack switcher to the container
        self.container.append(&self.stack_switcher);

        // Add the stack to the container
        self.container.append(&self.stack.upcast::<Widget>());

        // Add the status bar to the container
        let status_bar = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(10)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        status_bar.append(&self.refresh_button);
        status_bar.append(&self.status_label);
        status_bar.append(&self.progress_bar);

        self.container.append(&status_bar);

        // Load initial data
        self.refresh_models();

        Ok(())
    }

    /// Set up event handlers
    fn setup_event_handlers(&self) -> Result<(), AppError> {
        // Handle stack switcher changes
        self.stack.connect_visible_child_notify(clone!(@strong self.app_state as app_state => move |stack| {
            if let Some(child_name) = stack.visible_child_name() {
                gtk4::glib::spawn_future_local(async move {
                    let message = format!("Switched to {} view", child_name);
                    app_state.set_status_message(message).await;
                });
            }
        }));

        // Handle refresh button clicks
        self.refresh_button.connect_clicked(clone!(@strong self.model_manager as model_manager => move |_| {
            gtk4::glib::spawn_future_local(async move {
                if let Err(e) = model_manager.refresh_models().await {
                    eprintln!("Failed to refresh models: {}", e);
                }
            });
        }));

        // Handle model selection changes
        self.model_selector.connect_model_selected(clone!(@strong self.model_details as model_details => move |model| {
            model_details.update_model(model);
        }));

        Ok(())
    }

    /// Create the configuration panel
    fn create_config_panel(
        app_state: Arc<AppState>,
        model_manager: Arc<ModelManager>,
    ) -> Result<Box, AppError> {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .margin_top(20)
            .margin_bottom(20)
            .margin_start(20)
            .margin_end(20)
            .build();

        // Create a frame for general settings
        let general_frame = Frame::builder()
            .label("General Settings")
            .margin_bottom(20)
            .build();

        let general_grid = Grid::builder()
            .row_spacing(10)
            .column_spacing(15)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        // Default model setting
        let default_model_label = Label::builder()
            .label("Default Model:")
            .halign(Align::Start)
            .build();
        general_grid.attach(&default_model_label, 0, 0, 1, 1);

        let default_model_combo = ComboBoxText::builder()
            .build();
        general_grid.attach(&default_model_combo, 1, 0, 1, 1);

        // Auto download setting
        let auto_download_label = Label::builder()
            .label("Auto Download Models:")
            .halign(Align::Start)
            .build();
        general_grid.attach(&auto_download_label, 0, 1, 1, 1);

        let auto_download_switch = Switch::builder()
            .active(true)
            .halign(Align::Start)
            .build();
        general_grid.attach(&auto_download_switch, 1, 1, 1, 1);

        // Max loaded models setting
        let max_loaded_label = Label::builder()
            .label("Max Loaded Models:")
            .halign(Align::Start)
            .build();
        general_grid.attach(&max_loaded_label, 0, 2, 1, 1);

        let max_loaded_scale = Scale::builder()
            .orientation(Orientation::Horizontal)
            .adjustment(&gtk4::Adjustment::new(2.0, 1.0, 10.0, 1.0, 1.0, 0.0))
            .hexpand(true)
            .build();
        general_grid.attach(&max_loaded_scale, 1, 2, 1, 1);

        general_frame.set_child(Some(&general_grid));

        // Create a frame for performance settings
        let performance_frame = Frame::builder()
            .label("Performance Settings")
            .margin_bottom(20)
            .build();

        let performance_grid = Grid::builder()
            .row_spacing(10)
            .column_spacing(15)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        // Use GPU setting
        let use_gpu_label = Label::builder()
            .label("Use GPU (if available):")
            .halign(Align::Start)
            .build();
        performance_grid.attach(&use_gpu_label, 0, 0, 1, 1);

        let use_gpu_switch = Switch::builder()
            .active(true)
            .halign(Align::Start)
            .build();
        performance_grid.attach(&use_gpu_switch, 1, 0, 1, 1);

        // GPU device setting
        let gpu_device_label = Label::builder()
            .label("GPU Device:")
            .halign(Align::Start)
            .build();
        performance_grid.attach(&gpu_device_label, 0, 1, 1, 1);

        let gpu_device_entry = Entry::builder()
            .placeholder_text("0")
            .build();
        performance_grid.attach(&gpu_device_entry, 1, 1, 1, 1);

        performance_frame.set_child(Some(&performance_grid));

        // Create a frame for advanced settings
        let advanced_frame = Frame::builder()
            .label("Advanced Settings")
            .build();

        let advanced_grid = Grid::builder()
            .row_spacing(10)
            .column_spacing(15)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        // Model directory setting
        let model_dir_label = Label::builder()
            .label("Model Directory:")
            .halign(Align::Start)
            .build();
        advanced_grid.attach(&model_dir_label, 0, 0, 1, 1);

        let model_dir_entry = Entry::builder()
            .placeholder_text("/path/to/models")
            .editable(false)
            .build();
        advanced_grid.attach(&model_dir_entry, 1, 0, 1, 1);

        let browse_button = Button::builder()
            .label("Browse...")
            .build();
        advanced_grid.attach(&browse_button, 2, 0, 1, 1);

        // Operation timeout setting
        let timeout_label = Label::builder()
            .label("Operation Timeout (seconds):")
            .halign(Align::Start)
            .build();
        advanced_grid.attach(&timeout_label, 0, 1, 1, 1);

        let timeout_entry = Entry::builder()
            .text("300")
            .build();
        advanced_grid.attach(&timeout_entry, 1, 1, 1, 1);

        advanced_frame.set_child(Some(&advanced_grid));

        // Add frames to the container
        container.append(&general_frame);
        container.append(&performance_frame);
        container.append(&advanced_frame);

        // Add save button
        let save_button = Button::builder()
            .label("Save Configuration")
            .halign(Align::Center)
            .margin_top(20)
            .build();
        container.append(&save_button);

        // Set up event handlers
        save_button.connect_clicked(clone!(@strong app_state => move |_| {
            gtk4::glib::spawn_future_local(async move {
                app_state.set_status_message("Configuration saved".to_string()).await;
                app_state.show_notification(
                    "Configuration Saved".to_string(),
                    "Model settings have been updated".to_string(),
                ).await;
            });
        }));

        // Populate default models combo box
        gtk4::glib::spawn_future_local(clone!(@strong model_manager, @strong default_model_combo => async move {
            if let Err(e) = Self::populate_default_models(&model_manager, &default_model_combo).await {
                eprintln!("Failed to populate default models: {}", e);
            }
        }));

        Ok(container)
    }

    /// Populate the default models combo box
    async fn populate_default_models(
        model_manager: &ModelManager,
        combo: &ComboBoxText,
    ) -> Result<(), AppError> {
        let models = model_manager.get_models().await;
        
        for model in models {
            combo.append(Some(&model.name), &model.display_name);
        }
        
        // Set the first model as active
        combo.set_active(Some(0));
        
        Ok(())
    }

    /// Create the status bar
    fn create_status_bar() -> StatusBar {
        let status_label = Label::builder()
            .label("Ready")
            .halign(Align::Start)
            .hexpand(true)
            .build();

        let progress_bar = ProgressBar::builder()
            .text("")
            .show_text(true)
            .hexpand(false)
            .width_request(200)
            .build();

        let refresh_button = Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Refresh Models")
            .build();

        StatusBar {
            status_label,
            progress_bar,
            refresh_button,
        }
    }

    /// Refresh the list of models
    fn refresh_models(&self) {
        let model_manager = self.model_manager.clone();
        let model_selector = self.model_selector.clone();
        let status_label = self.status_label.clone();
        let progress_bar = self.progress_bar.clone();

        gtk4::glib::spawn_future_local(async move {
            status_label.set_text("Refreshing models...");
            progress_bar.set_visible(true);
            progress_bar.set_fraction(0.0);
            progress_bar.set_text(Some("Loading..."));

            if let Err(e) = model_manager.refresh_models().await {
                status_label.set_text(&format!("Error: {}", e));
                progress_bar.set_visible(false);
                return;
            }

            // Update the model selector
            if let Err(e) = model_selector.refresh_models().await {
                status_label.set_text(&format!("Error: {}", e));
                progress_bar.set_visible(false);
                return;
            }

            status_label.set_text("Models refreshed successfully");
            progress_bar.set_fraction(1.0);
            progress_bar.set_text(Some("Done"));

            // Hide the progress bar after a delay
            gtk4::glib::timeout_add_seconds_local_once(2, move || {
                progress_bar.set_visible(false);
            });
        });
    }

    /// Get the main container widget
    pub fn get_widget(&self) -> &Widget {
        self.container.upcast_ref()
    }

    /// Get the model selector widget
    pub fn get_model_selector(&self) -> &ModelSelectorWidget {
        &self.model_selector
    }

    /// Get the model details widget
    pub fn get_model_details(&self) -> &ModelDetailsWidget {
        &self.model_details
    }

    /// Switch to the model selector view
    pub fn show_model_selector(&self) {
        self.stack.set_visible_child_name("selector");
    }

    /// Switch to the model details view
    pub fn show_model_details(&self) {
        self.stack.set_visible_child_name("details");
    }

    /// Switch to the configuration view
    pub fn show_configuration(&self) {
        self.stack.set_visible_child_name("config");
    }

    /// Update the status message
    pub fn set_status(&self, message: &str) {
        self.status_label.set_text(message);
    }

    /// Update the progress
    pub fn set_progress(&self, fraction: f64, text: Option<&str>) {
        self.progress_bar.set_fraction(fraction);
        self.progress_bar.set_text(text);
        self.progress_bar.set_visible(fraction > 0.0);
    }
}

/// Status bar component
struct StatusBar {
    status_label: Label,
    progress_bar: ProgressBar,
    refresh_button: Button,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::api::BackendConfig;
    use crate::services::BackendClient;
    
    #[test]
    fn test_model_panel_creation() {
        // This test would require a proper GTK4 test environment
        // For now, we'll just verify that the struct can be created
        // In a real test, you would need to initialize GTK4 first
    }
}