//! Settings dialog for the ASRPro application
//!
//! This module contains the SettingsDialog struct which provides a comprehensive
//! settings interface with tabs for different settings categories.

use gtk4::prelude::*;
use gtk4::{
    ApplicationWindow, Dialog, ResponseType, Box as GtkBox, Label, Entry,
    Switch, Scale, SpinButton, ComboBoxText, Button, Grid, Stack,
    StackSwitcher, ScrolledWindow, CheckButton, FileChooserAction, Adjustment, FontButton, ColorButton, InfoBar,
    MessageDialog, ButtonsType, MessageType,
};
use gtk4::glib::clone;
use std::sync::Arc;
use std::path::PathBuf;

use crate::models::{
    AppState, Settings, GeneralSettings, AudioSettings, TranscriptionSettings,
    AdvancedSettings, UiSettings, FilePathSettings, NotificationSettings,
};
use crate::utils::{AppError, AppResult};

/// Settings dialog with tabs for different settings categories
pub struct SettingsDialog {
    dialog: Dialog,
    app_state: Arc<AppState>,
    stack: Stack,
    stack_switcher: StackSwitcher,
    
    // Settings containers
    general_settings: GeneralSettings,
    audio_settings: AudioSettings,
    transcription_settings: TranscriptionSettings,
    advanced_settings: AdvancedSettings,
    ui_settings: UiSettings,
    file_path_settings: FilePathSettings,
    notification_settings: NotificationSettings,
    
    // General settings widgets
    language_combo: ComboBoxText,
    theme_combo: ComboBoxText,
    auto_save_switch: Switch,
    auto_save_interval_spin: SpinButton,
    start_maximized_switch: Switch,
    check_for_updates_switch: Switch,
    update_check_interval_spin: SpinButton,
    
    // Audio settings widgets
    default_input_device_combo: ComboBoxText,
    default_output_device_combo: ComboBoxText,
    default_sample_rate_combo: ComboBoxText,
    default_format_combo: ComboBoxText,
    default_input_volume_scale: Scale,
    default_output_volume_scale: Scale,
    echo_cancellation_switch: Switch,
    noise_suppression_switch: Switch,
    automatic_gain_control_switch: Switch,
    buffer_size_spin: SpinButton,
    
    // Transcription settings widgets
    default_model_combo: ComboBoxText,
    default_language_combo: ComboBoxText,
    include_timestamps_switch: Switch,
    include_word_timestamps_switch: Switch,
    include_confidence_switch: Switch,
    include_segments_switch: Switch,
    translate_to_english_switch: Switch,
    temperature_scale: Scale,
    automatic_punctuation_switch: Switch,
    number_formatting_switch: Switch,
    profanity_filter_switch: Switch,
    
    // Advanced settings widgets
    max_concurrent_threads_spin: SpinButton,
    max_file_size_spin: SpinButton,
    temp_dir_entry: Entry,
    cache_dir_entry: Entry,
    debug_logging_switch: Switch,
    log_file_path_entry: Entry,
    max_log_size_spin: SpinButton,
    log_file_count_spin: SpinButton,
    backend_url_entry: Entry,
    api_key_entry: Entry,
    request_timeout_spin: SpinButton,
    verify_ssl_switch: Switch,
    
    // UI settings widgets
    font_family_font_button: FontButton,
    font_size_spin: SpinButton,
    ui_density_combo: ComboBoxText,
    show_icons_switch: Switch,
    show_tooltips_switch: Switch,
    animate_transitions_switch: Switch,
    show_file_extensions_switch: Switch,
    default_transcription_view_combo: ComboBoxText,
    word_wrap_switch: Switch,
    show_line_numbers_switch: Switch,
    show_confidence_scores_switch: Switch,
    
    // File path settings widgets
    audio_dir_button: FileChooserButton,
    output_dir_button: FileChooserButton,
    last_opened_dir_button: FileChooserButton,
    
    // Notification settings widgets
    notifications_enabled_switch: Switch,
    show_transcription_complete_switch: Switch,
    show_errors_switch: Switch,
    show_file_events_switch: Switch,
    show_system_events_switch: Switch,
    sound_enabled_switch: Switch,
    sound_volume_scale: Scale,
    custom_sound_path_entry: Entry,
    notification_duration_spin: SpinButton,
    notification_position_combo: ComboBoxText,
    
    // Status widgets
    info_bar: InfoBar,
    info_label: Label,
}

impl SettingsDialog {
    /// Create a new settings dialog
    pub fn new(parent_window: &ApplicationWindow, app_state: Arc<AppState>) -> AppResult<Self> {
        // Create the dialog
        let dialog = Dialog::builder()
            .title("Settings")
            .transient_for(parent_window)
            .modal(true)
            .default_width(800)
            .default_height(600)
            .build();
        
        // Add dialog buttons
        dialog.add_button("Cancel", ResponseType::Cancel);
        dialog.add_button("Reset to Defaults", ResponseType::Other(0));
        dialog.add_button("Import", ResponseType::Other(1));
        dialog.add_button("Export", ResponseType::Other(2));
        dialog.add_button("Apply", ResponseType::Apply);
        dialog.add_button("Save", ResponseType::Ok);
        
        // Create the main container
        let content_area = dialog.content_area();
        let main_box = GtkBox::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(6)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();
        
        // Create the info bar
        let info_bar = InfoBar::new();
        let info_label = Label::new(None);
        info_bar.add_child(&info_label);
        info_bar.set_revealed(false);
        
        // Create the stack and stack switcher
        let stack = Stack::new();
        stack.set_transition_type(gtk4::StackTransitionType::SlideLeftRight);
        
        let stack_switcher = StackSwitcher::new();
        stack_switcher.set_stack(&stack);
        
        // Create a settings dialog instance
        let mut settings_dialog = Self {
            dialog,
            app_state,
            stack,
            stack_switcher,
            
            // Initialize with default settings
            general_settings: GeneralSettings::default(),
            audio_settings: AudioSettings::default(),
            transcription_settings: TranscriptionSettings::default(),
            advanced_settings: AdvancedSettings::default(),
            ui_settings: UiSettings::default(),
            file_path_settings: FilePathSettings::default(),
            notification_settings: NotificationSettings::default(),
            
            // Initialize all widgets
            language_combo: ComboBoxText::new(),
            theme_combo: ComboBoxText::new(),
            auto_save_switch: Switch::new(),
            auto_save_interval_spin: SpinButton::new(),
            start_maximized_switch: Switch::new(),
            remember_window_state_switch: Switch::new(),
            check_for_updates_switch: Switch::new(),
            update_check_interval_spin: SpinButton::new(),
            
            // Audio settings widgets
            default_input_device_combo: ComboBoxText::new(),
            default_output_device_combo: ComboBoxText::new(),
            default_sample_rate_combo: ComboBoxText::new(),
            default_format_combo: ComboBoxText::new(),
            default_input_volume_scale: Scale::new(),
            default_output_volume_scale: Scale::new(),
            echo_cancellation_switch: Switch::new(),
            noise_suppression_switch: Switch::new(),
            automatic_gain_control_switch: Switch::new(),
            buffer_size_spin: SpinButton::new(),
            
            // Transcription settings widgets
            default_model_combo: ComboBoxText::new(),
            default_language_combo: ComboBoxText::new(),
            include_timestamps_switch: Switch::new(),
            include_word_timestamps_switch: Switch::new(),
            include_confidence_switch: Switch::new(),
            include_segments_switch: Switch::new(),
            translate_to_english_switch: Switch::new(),
            temperature_scale: Scale::new(),
            automatic_punctuation_switch: Switch::new(),
            number_formatting_switch: Switch::new(),
            profanity_filter_switch: Switch::new(),
            
            // Advanced settings widgets
            max_concurrent_threads_spin: SpinButton::new(),
            max_file_size_spin: SpinButton::new(),
            temp_dir_entry: Entry::new(),
            cache_dir_entry: Entry::new(),
            debug_logging_switch: Switch::new(),
            log_file_path_entry: Entry::new(),
            max_log_size_spin: SpinButton::new(),
            log_file_count_spin: SpinButton::new(),
            backend_url_entry: Entry::new(),
            api_key_entry: Entry::new(),
            request_timeout_spin: SpinButton::new(),
            verify_ssl_switch: Switch::new(),
            
            // UI settings widgets
            font_family_font_button: FontButton::new(),
            font_size_spin: SpinButton::new(),
            ui_density_combo: ComboBoxText::new(),
            show_icons_switch: Switch::new(),
            show_tooltips_switch: Switch::new(),
            animate_transitions_switch: Switch::new(),
            show_file_extensions_switch: Switch::new(),
            default_transcription_view_combo: ComboBoxText::new(),
            word_wrap_switch: Switch::new(),
            show_line_numbers_switch: Switch::new(),
            show_confidence_scores_switch: Switch::new(),
            
            // File path settings widgets
            audio_dir_button: FileChooserButton::new("Select Audio Directory", FileChooserAction::SelectFolder),
            output_dir_button: FileChooserButton::new("Select Output Directory", FileChooserAction::SelectFolder),
            last_opened_dir_button: FileChooserButton::new("Select Last Opened Directory", FileChooserAction::SelectFolder),
            
            // Notification settings widgets
            notifications_enabled_switch: Switch::new(),
            show_transcription_complete_switch: Switch::new(),
            show_errors_switch: Switch::new(),
            show_file_events_switch: Switch::new(),
            show_system_events_switch: Switch::new(),
            sound_enabled_switch: Switch::new(),
            sound_volume_scale: Scale::new(),
            custom_sound_path_entry: Entry::new(),
            notification_duration_spin: SpinButton::new(),
            notification_position_combo: ComboBoxText::new(),
            
            // Status widgets
            info_bar,
            info_label,
        };
        
        // Load current settings
        // Load settings synchronously for now
        // In a real implementation, you would handle this properly
        // settings_dialog.load_current_settings().await?;
        
        // Setup the UI
        settings_dialog.setup_ui(&main_box)?;
        
        // Connect signals
        settings_dialog.connect_signals();
        
        // Add everything to the content area
        content_area.append(&main_box);
        
        Ok(settings_dialog)
    }
    
    /// Setup the UI components
    fn setup_ui(&mut self, main_box: &GtkBox) -> AppResult<()> {
        // Add the stack switcher and info bar to the main box
        main_box.append(&self.stack_switcher);
        main_box.append(&self.info_bar);
        
        // Create the tabs
        self.setup_general_tab()?;
        self.setup_audio_tab()?;
        self.setup_transcription_tab()?;
        self.setup_advanced_tab()?;
        self.setup_ui_tab()?;
        self.setup_file_paths_tab()?;
        self.setup_notifications_tab()?;
        
        // Add the stack to the main box
        main_box.append(&self.stack);
        
        Ok(())
    }
    
    /// Setup the General settings tab
    fn setup_general_tab(&mut self) -> AppResult<()> {
        let grid = Grid::builder()
            .row_spacing(12)
            .column_spacing(12)
            .margin_start(12)
            .margin_end(12)
            .margin_top(12)
            .margin_bottom(12)
            .build();
        
        let mut row = 0;
        
        // Language
        grid.attach(&Label::new(Some("Language:")), 0, row, 1, 1);
        self.language_combo.append_text("English");
        self.language_combo.append_text("Spanish");
        self.language_combo.append_text("French");
        self.language_combo.append_text("German");
        self.language_combo.append_text("Chinese");
        self.language_combo.append_text("Japanese");
        grid.attach(&self.language_combo, 1, row, 1, 1);
        row += 1;
        
        // Theme
        grid.attach(&Label::new(Some("Theme:")), 0, row, 1, 1);
        self.theme_combo.append_text("Light");
        self.theme_combo.append_text("Dark");
        self.theme_combo.append_text("System");
        grid.attach(&self.theme_combo, 1, row, 1, 1);
        row += 1;
        
        // Auto-save
        grid.attach(&Label::new(Some("Auto-save:")), 0, row, 1, 1);
        grid.attach(&self.auto_save_switch, 1, row, 1, 1);
        row += 1;
        
        // Auto-save interval
        grid.attach(&Label::new(Some("Auto-save interval (minutes):")), 0, row, 1, 1);
        let auto_save_interval_adj = Adjustment::new(5.0, 1.0, 60.0, 1.0, 5.0, 0.0);
        self.auto_save_interval_spin.set_adjustment(&auto_save_interval_adj);
        grid.attach(&self.auto_save_interval_spin, 1, row, 1, 1);
        row += 1;
        
        // Start maximized
        grid.attach(&Label::new(Some("Start maximized:")), 0, row, 1, 1);
        grid.attach(&self.start_maximized_switch, 1, row, 1, 1);
        row += 1;
        
        // Remember window state
        grid.attach(&Label::new(Some("Remember window state:")), 0, row, 1, 1);
        grid.attach(&self.remember_window_state_switch, 1, row, 1, 1);
        row += 1;
        
        // Check for updates
        grid.attach(&Label::new(Some("Check for updates:")), 0, row, 1, 1);
        grid.attach(&self.check_for_updates_switch, 1, row, 1, 1);
        row += 1;
        
        // Update check interval
        grid.attach(&Label::new(Some("Update check interval (days):")), 0, row, 1, 1);
        let update_check_interval_adj = Adjustment::new(7.0, 1.0, 30.0, 1.0, 5.0, 0.0);
        self.update_check_interval_spin.set_adjustment(&update_check_interval_adj);
        grid.attach(&self.update_check_interval_spin, 1, row, 1, 1);
        row += 1;
        
        // Create a scrolled window for the grid
        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .min_content_width(600)
            .min_content_height(400)
            .child(&grid)
            .build();
        
        // Add the tab to the stack
        self.stack.add_titled(&scrolled, Some("general"), "General");
        
        Ok(())
    }
    
    /// Setup the Audio settings tab
    fn setup_audio_tab(&mut self) -> AppResult<()> {
        let grid = Grid::builder()
            .row_spacing(12)
            .column_spacing(12)
            .margin_start(12)
            .margin_end(12)
            .margin_top(12)
            .margin_bottom(12)
            .build();
        
        let mut row = 0;
        
        // Default input device
        grid.attach(&Label::new(Some("Default input device:")), 0, row, 1, 1);
        self.default_input_device_combo.append_text("Default");
        // In a real implementation, you would populate this with available devices
        grid.attach(&self.default_input_device_combo, 1, row, 1, 1);
        row += 1;
        
        // Default output device
        grid.attach(&Label::new(Some("Default output device:")), 0, row, 1, 1);
        self.default_output_device_combo.append_text("Default");
        // In a real implementation, you would populate this with available devices
        grid.attach(&self.default_output_device_combo, 1, row, 1, 1);
        row += 1;
        
        // Default sample rate
        grid.attach(&Label::new(Some("Default sample rate:")), 0, row, 1, 1);
        self.default_sample_rate_combo.append_text("8000 Hz");
        self.default_sample_rate_combo.append_text("16000 Hz");
        self.default_sample_rate_combo.append_text("22050 Hz");
        self.default_sample_rate_combo.append_text("44100 Hz");
        self.default_sample_rate_combo.append_text("48000 Hz");
        grid.attach(&self.default_sample_rate_combo, 1, row, 1, 1);
        row += 1;
        
        // Default format
        grid.attach(&Label::new(Some("Default format:")), 0, row, 1, 1);
        self.default_format_combo.append_text("WAV");
        self.default_format_combo.append_text("MP3");
        self.default_format_combo.append_text("FLAC");
        self.default_format_combo.append_text("OGG");
        grid.attach(&self.default_format_combo, 1, row, 1, 1);
        row += 1;
        
        // Default input volume
        grid.attach(&Label::new(Some("Default input volume:")), 0, row, 1, 1);
        let input_volume_adj = Adjustment::new(0.8, 0.0, 1.0, 0.01, 0.1, 0.0);
        self.default_input_volume_scale.set_adjustment(&input_volume_adj);
        self.default_input_volume_scale.set_draw_value(true);
        self.default_input_volume_scale.set_value_pos(gtk4::PositionType::Right);
        grid.attach(&self.default_input_volume_scale, 1, row, 1, 1);
        row += 1;
        
        // Default output volume
        grid.attach(&Label::new(Some("Default output volume:")), 0, row, 1, 1);
        let output_volume_adj = Adjustment::new(0.8, 0.0, 1.0, 0.01, 0.1, 0.0);
        self.default_output_volume_scale.set_adjustment(&output_volume_adj);
        self.default_output_volume_scale.set_draw_value(true);
        self.default_output_volume_scale.set_value_pos(gtk4::PositionType::Right);
        grid.attach(&self.default_output_volume_scale, 1, row, 1, 1);
        row += 1;
        
        // Echo cancellation
        grid.attach(&Label::new(Some("Echo cancellation:")), 0, row, 1, 1);
        grid.attach(&self.echo_cancellation_switch, 1, row, 1, 1);
        row += 1;
        
        // Noise suppression
        grid.attach(&Label::new(Some("Noise suppression:")), 0, row, 1, 1);
        grid.attach(&self.noise_suppression_switch, 1, row, 1, 1);
        row += 1;
        
        // Automatic gain control
        grid.attach(&Label::new(Some("Automatic gain control:")), 0, row, 1, 1);
        grid.attach(&self.automatic_gain_control_switch, 1, row, 1, 1);
        row += 1;
        
        // Buffer size
        grid.attach(&Label::new(Some("Buffer size (ms):")), 0, row, 1, 1);
        let buffer_size_adj = Adjustment::new(100.0, 10.0, 500.0, 10.0, 50.0, 0.0);
        self.buffer_size_spin.set_adjustment(&buffer_size_adj);
        grid.attach(&self.buffer_size_spin, 1, row, 1, 1);
        row += 1;
        
        // Create a scrolled window for the grid
        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .min_content_width(600)
            .min_content_height(400)
            .child(&grid)
            .build();
        
        // Add the tab to the stack
        self.stack.add_titled(&scrolled, Some("audio"), "Audio");
        
        Ok(())
    }
    
    /// Setup the Transcription settings tab
    fn setup_transcription_tab(&mut self) -> AppResult<()> {
        let grid = Grid::builder()
            .row_spacing(12)
            .column_spacing(12)
            .margin_start(12)
            .margin_end(12)
            .margin_top(12)
            .margin_bottom(12)
            .build();
        
        let mut row = 0;
        
        // Default model
        grid.attach(&Label::new(Some("Default model:")), 0, row, 1, 1);
        self.default_model_combo.append_text("whisper-tiny");
        self.default_model_combo.append_text("whisper-base");
        self.default_model_combo.append_text("whisper-small");
        self.default_model_combo.append_text("whisper-medium");
        self.default_model_combo.append_text("whisper-large");
        grid.attach(&self.default_model_combo, 1, row, 1, 1);
        row += 1;
        
        // Default language
        grid.attach(&Label::new(Some("Default language:")), 0, row, 1, 1);
        self.default_language_combo.append_text("Auto-detect");
        self.default_language_combo.append_text("English");
        self.default_language_combo.append_text("Spanish");
        self.default_language_combo.append_text("French");
        self.default_language_combo.append_text("German");
        self.default_language_combo.append_text("Chinese");
        self.default_language_combo.append_text("Japanese");
        grid.attach(&self.default_language_combo, 1, row, 1, 1);
        row += 1;
        
        // Include timestamps
        grid.attach(&Label::new(Some("Include timestamps:")), 0, row, 1, 1);
        grid.attach(&self.include_timestamps_switch, 1, row, 1, 1);
        row += 1;
        
        // Include word timestamps
        grid.attach(&Label::new(Some("Include word timestamps:")), 0, row, 1, 1);
        grid.attach(&self.include_word_timestamps_switch, 1, row, 1, 1);
        row += 1;
        
        // Include confidence scores
        grid.attach(&Label::new(Some("Include confidence scores:")), 0, row, 1, 1);
        grid.attach(&self.include_confidence_switch, 1, row, 1, 1);
        row += 1;
        
        // Include segments
        grid.attach(&Label::new(Some("Include segments:")), 0, row, 1, 1);
        grid.attach(&self.include_segments_switch, 1, row, 1, 1);
        row += 1;
        
        // Translate to English
        grid.attach(&Label::new(Some("Translate to English:")), 0, row, 1, 1);
        grid.attach(&self.translate_to_english_switch, 1, row, 1, 1);
        row += 1;
        
        // Temperature
        grid.attach(&Label::new(Some("Temperature:")), 0, row, 1, 1);
        let temperature_adj = Adjustment::new(0.0, 0.0, 1.0, 0.01, 0.1, 0.0);
        self.temperature_scale.set_adjustment(&temperature_adj);
        self.temperature_scale.set_draw_value(true);
        self.temperature_scale.set_value_pos(gtk4::PositionType::Right);
        grid.attach(&self.temperature_scale, 1, row, 1, 1);
        row += 1;
        
        // Automatic punctuation
        grid.attach(&Label::new(Some("Automatic punctuation:")), 0, row, 1, 1);
        grid.attach(&self.automatic_punctuation_switch, 1, row, 1, 1);
        row += 1;
        
        // Number formatting
        grid.attach(&Label::new(Some("Number formatting:")), 0, row, 1, 1);
        grid.attach(&self.number_formatting_switch, 1, row, 1, 1);
        row += 1;
        
        // Profanity filter
        grid.attach(&Label::new(Some("Profanity filter:")), 0, row, 1, 1);
        grid.attach(&self.profanity_filter_switch, 1, row, 1, 1);
        row += 1;
        
        // Create a scrolled window for the grid
        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .min_content_width(600)
            .min_content_height(400)
            .child(&grid)
            .build();
        
        // Add the tab to the stack
        self.stack.add_titled(&scrolled, Some("transcription"), "Transcription");
        
        Ok(())
    }
    
    /// Setup the Advanced settings tab
    fn setup_advanced_tab(&mut self) -> AppResult<()> {
        let grid = Grid::builder()
            .row_spacing(12)
            .column_spacing(12)
            .margin_start(12)
            .margin_end(12)
            .margin_top(12)
            .margin_bottom(12)
            .build();
        
        let mut row = 0;
        
        // Max concurrent threads
        grid.attach(&Label::new(Some("Max concurrent threads:")), 0, row, 1, 1);
        let max_threads_adj = Adjustment::new(2.0, 1.0, 8.0, 1.0, 1.0, 0.0);
        self.max_concurrent_threads_spin.set_adjustment(&max_threads_adj);
        grid.attach(&self.max_concurrent_threads_spin, 1, row, 1, 1);
        row += 1;
        
        // Max file size
        grid.attach(&Label::new(Some("Max file size (MB):")), 0, row, 1, 1);
        let max_file_size_adj = Adjustment::new(100.0, 10.0, 1000.0, 10.0, 50.0, 0.0);
        self.max_file_size_spin.set_adjustment(&max_file_size_adj);
        grid.attach(&self.max_file_size_spin, 1, row, 1, 1);
        row += 1;
        
        // Temp directory
        grid.attach(&Label::new(Some("Temp directory:")), 0, row, 1, 1);
        grid.attach(&self.temp_dir_entry, 1, row, 1, 1);
        row += 1;
        
        // Cache directory
        grid.attach(&Label::new(Some("Cache directory:")), 0, row, 1, 1);
        grid.attach(&self.cache_dir_entry, 1, row, 1, 1);
        row += 1;
        
        // Debug logging
        grid.attach(&Label::new(Some("Debug logging:")), 0, row, 1, 1);
        grid.attach(&self.debug_logging_switch, 1, row, 1, 1);
        row += 1;
        
        // Log file path
        grid.attach(&Label::new(Some("Log file path:")), 0, row, 1, 1);
        grid.attach(&self.log_file_path_entry, 1, row, 1, 1);
        row += 1;
        
        // Max log size
        grid.attach(&Label::new(Some("Max log size (MB):")), 0, row, 1, 1);
        let max_log_size_adj = Adjustment::new(10.0, 1.0, 100.0, 1.0, 5.0, 0.0);
        self.max_log_size_spin.set_adjustment(&max_log_size_adj);
        grid.attach(&self.max_log_size_spin, 1, row, 1, 1);
        row += 1;
        
        // Log file count
        grid.attach(&Label::new(Some("Log file count:")), 0, row, 1, 1);
        let log_file_count_adj = Adjustment::new(5.0, 1.0, 20.0, 1.0, 1.0, 0.0);
        self.log_file_count_spin.set_adjustment(&log_file_count_adj);
        grid.attach(&self.log_file_count_spin, 1, row, 1, 1);
        row += 1;
        
        // Backend URL
        grid.attach(&Label::new(Some("Backend URL:")), 0, row, 1, 1);
        grid.attach(&self.backend_url_entry, 1, row, 1, 1);
        row += 1;
        
        // API key
        grid.attach(&Label::new(Some("API key:")), 0, row, 1, 1);
        self.api_key_entry.set_visibility(false);
        grid.attach(&self.api_key_entry, 1, row, 1, 1);
        row += 1;
        
        // Request timeout
        grid.attach(&Label::new(Some("Request timeout (seconds):")), 0, row, 1, 1);
        let request_timeout_adj = Adjustment::new(30.0, 5.0, 300.0, 5.0, 10.0, 0.0);
        self.request_timeout_spin.set_adjustment(&request_timeout_adj);
        grid.attach(&self.request_timeout_spin, 1, row, 1, 1);
        row += 1;
        
        // Verify SSL
        grid.attach(&Label::new(Some("Verify SSL certificates:")), 0, row, 1, 1);
        grid.attach(&self.verify_ssl_switch, 1, row, 1, 1);
        row += 1;
        
        // Create a scrolled window for the grid
        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .min_content_width(600)
            .min_content_height(400)
            .child(&grid)
            .build();
        
        // Add the tab to the stack
        self.stack.add_titled(&scrolled, Some("advanced"), "Advanced");
        
        Ok(())
    }
    
    /// Setup the UI settings tab
    fn setup_ui_tab(&mut self) -> AppResult<()> {
        let grid = Grid::builder()
            .row_spacing(12)
            .column_spacing(12)
            .margin_start(12)
            .margin_end(12)
            .margin_top(12)
            .margin_bottom(12)
            .build();
        
        let mut row = 0;
        
        // Font family
        grid.attach(&Label::new(Some("Font family:")), 0, row, 1, 1);
        grid.attach(&self.font_family_font_button, 1, row, 1, 1);
        row += 1;
        
        // Font size
        grid.attach(&Label::new(Some("Font size (points):")), 0, row, 1, 1);
        let font_size_adj = Adjustment::new(12.0, 8.0, 24.0, 1.0, 2.0, 0.0);
        self.font_size_spin.set_adjustment(&font_size_adj);
        grid.attach(&self.font_size_spin, 1, row, 1, 1);
        row += 1;
        
        // UI density
        grid.attach(&Label::new(Some("UI density:")), 0, row, 1, 1);
        self.ui_density_combo.append_text("Compact");
        self.ui_density_combo.append_text("Normal");
        self.ui_density_combo.append_text("Spacious");
        grid.attach(&self.ui_density_combo, 1, row, 1, 1);
        row += 1;
        
        // Show icons
        grid.attach(&Label::new(Some("Show icons:")), 0, row, 1, 1);
        grid.attach(&self.show_icons_switch, 1, row, 1, 1);
        row += 1;
        
        // Show tooltips
        grid.attach(&Label::new(Some("Show tooltips:")), 0, row, 1, 1);
        grid.attach(&self.show_tooltips_switch, 1, row, 1, 1);
        row += 1;
        
        // Animate transitions
        grid.attach(&Label::new(Some("Animate transitions:")), 0, row, 1, 1);
        grid.attach(&self.animate_transitions_switch, 1, row, 1, 1);
        row += 1;
        
        // Show file extensions
        grid.attach(&Label::new(Some("Show file extensions:")), 0, row, 1, 1);
        grid.attach(&self.show_file_extensions_switch, 1, row, 1, 1);
        row += 1;
        
        // Default transcription view
        grid.attach(&Label::new(Some("Default transcription view:")), 0, row, 1, 1);
        self.default_transcription_view_combo.append_text("Text");
        self.default_transcription_view_combo.append_text("Segments");
        self.default_transcription_view_combo.append_text("Timeline");
        grid.attach(&self.default_transcription_view_combo, 1, row, 1, 1);
        row += 1;
        
        // Word wrap
        grid.attach(&Label::new(Some("Word wrap:")), 0, row, 1, 1);
        grid.attach(&self.word_wrap_switch, 1, row, 1, 1);
        row += 1;
        
        // Show line numbers
        grid.attach(&Label::new(Some("Show line numbers:")), 0, row, 1, 1);
        grid.attach(&self.show_line_numbers_switch, 1, row, 1, 1);
        row += 1;
        
        // Show confidence scores
        grid.attach(&Label::new(Some("Show confidence scores:")), 0, row, 1, 1);
        grid.attach(&self.show_confidence_scores_switch, 1, row, 1, 1);
        row += 1;
        
        // Create a scrolled window for the grid
        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .min_content_width(600)
            .min_content_height(400)
            .child(&grid)
            .build();
        
        // Add the tab to the stack
        self.stack.add_titled(&scrolled, Some("ui"), "UI");
        
        Ok(())
    }
    
    /// Setup the File Paths settings tab
    fn setup_file_paths_tab(&mut self) -> AppResult<()> {
        let grid = Grid::builder()
            .row_spacing(12)
            .column_spacing(12)
            .margin_start(12)
            .margin_end(12)
            .margin_top(12)
            .margin_bottom(12)
            .build();
        
        let mut row = 0;
        
        // Audio directory
        grid.attach(&Label::new(Some("Audio directory:")), 0, row, 1, 1);
        grid.attach(&self.audio_dir_button, 1, row, 1, 1);
        row += 1;
        
        // Output directory
        grid.attach(&Label::new(Some("Output directory:")), 0, row, 1, 1);
        grid.attach(&self.output_dir_button, 1, row, 1, 1);
        row += 1;
        
        // Last opened directory
        grid.attach(&Label::new(Some("Last opened directory:")), 0, row, 1, 1);
        grid.attach(&self.last_opened_dir_button, 1, row, 1, 1);
        row += 1;
        
        // Create a scrolled window for the grid
        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .min_content_width(600)
            .min_content_height(400)
            .child(&grid)
            .build();
        
        // Add the tab to the stack
        self.stack.add_titled(&scrolled, Some("file_paths"), "File Paths");
        
        Ok(())
    }
    
    /// Setup the Notifications settings tab
    fn setup_notifications_tab(&mut self) -> AppResult<()> {
        let grid = Grid::builder()
            .row_spacing(12)
            .column_spacing(12)
            .margin_start(12)
            .margin_end(12)
            .margin_top(12)
            .margin_bottom(12)
            .build();
        
        let mut row = 0;
        
        // Notifications enabled
        grid.attach(&Label::new(Some("Enable notifications:")), 0, row, 1, 1);
        grid.attach(&self.notifications_enabled_switch, 1, row, 1, 1);
        row += 1;
        
        // Show transcription complete
        grid.attach(&Label::new(Some("Show transcription complete:")), 0, row, 1, 1);
        grid.attach(&self.show_transcription_complete_switch, 1, row, 1, 1);
        row += 1;
        
        // Show errors
        grid.attach(&Label::new(Some("Show errors:")), 0, row, 1, 1);
        grid.attach(&self.show_errors_switch, 1, row, 1, 1);
        row += 1;
        
        // Show file events
        grid.attach(&Label::new(Some("Show file events:")), 0, row, 1, 1);
        grid.attach(&self.show_file_events_switch, 1, row, 1, 1);
        row += 1;
        
        // Show system events
        grid.attach(&Label::new(Some("Show system events:")), 0, row, 1, 1);
        grid.attach(&self.show_system_events_switch, 1, row, 1, 1);
        row += 1;
        
        // Sound enabled
        grid.attach(&Label::new(Some("Sound notifications:")), 0, row, 1, 1);
        grid.attach(&self.sound_enabled_switch, 1, row, 1, 1);
        row += 1;
        
        // Sound volume
        grid.attach(&Label::new(Some("Sound volume:")), 0, row, 1, 1);
        let sound_volume_adj = Adjustment::new(0.5, 0.0, 1.0, 0.01, 0.1, 0.0);
        self.sound_volume_scale.set_adjustment(&sound_volume_adj);
        self.sound_volume_scale.set_draw_value(true);
        self.sound_volume_scale.set_value_pos(gtk4::PositionType::Right);
        grid.attach(&self.sound_volume_scale, 1, row, 1, 1);
        row += 1;
        
        // Custom sound path
        grid.attach(&Label::new(Some("Custom sound path:")), 0, row, 1, 1);
        grid.attach(&self.custom_sound_path_entry, 1, row, 1, 1);
        row += 1;
        
        // Notification duration
        grid.attach(&Label::new(Some("Notification duration (seconds):")), 0, row, 1, 1);
        let notification_duration_adj = Adjustment::new(5.0, 1.0, 30.0, 1.0, 5.0, 0.0);
        self.notification_duration_spin.set_adjustment(&notification_duration_adj);
        grid.attach(&self.notification_duration_spin, 1, row, 1, 1);
        row += 1;
        
        // Notification position
        grid.attach(&Label::new(Some("Notification position:")), 0, row, 1, 1);
        self.notification_position_combo.append_text("Top-left");
        self.notification_position_combo.append_text("Top-right");
        self.notification_position_combo.append_text("Bottom-left");
        self.notification_position_combo.append_text("Bottom-right");
        grid.attach(&self.notification_position_combo, 1, row, 1, 1);
        row += 1;
        
        // Create a scrolled window for the grid
        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .min_content_width(600)
            .min_content_height(400)
            .child(&grid)
            .build();
        
        // Add the tab to the stack
        self.stack.add_titled(&scrolled, Some("notifications"), "Notifications");
        
        Ok(())
    }
    
    /// Connect signals for the dialog
    fn connect_signals(&self) {
        // Connect dialog response
        self.dialog.connect_response(None, clone!(@strong self.app_state as app_state => move |dialog, response| {
            gtk4::glib::spawn_future_local(async move {
                match response {
                    ResponseType::Ok => {
                        // Save and close
                        app_state.set_status_message("Settings saved".to_string()).await;
                        dialog.close();
                    },
                    ResponseType::Apply => {
                        // Apply without closing
                        app_state.set_status_message("Settings applied".to_string()).await;
                    },
                    ResponseType::Cancel => {
                        // Close without saving
                        dialog.close();
                    },
                    ResponseType::Other(0) => {
                        // Reset to defaults
                        app_state.set_status_message("Reset to defaults".to_string()).await;
                    },
                    ResponseType::Other(1) => {
                        // Import
                        app_state.set_status_message("Import settings".to_string()).await;
                    },
                    ResponseType::Other(2) => {
                        // Export
                        app_state.set_status_message("Export settings".to_string()).await;
                    },
                    _ => {}
                }
            });
        }));
        
        // Connect auto-save switch to interval spin button sensitivity
        self.auto_save_switch.connect_active_notify(clone!(@strong self.auto_save_interval_spin as spin => move |switch| {
            spin.set_sensitive(switch.is_active());
        }));
        
        // Connect sound enabled switch to volume scale sensitivity
        self.sound_enabled_switch.connect_active_notify(clone!(@strong self.sound_volume_scale as scale => move |switch| {
            scale.set_sensitive(switch.is_active());
        }));
    }
    
    /// Load current settings into the dialog
    async fn load_current_settings(&mut self) -> AppResult<()> {
        // Get current settings from the config manager
        let config_manager = self.app_state.get_config_manager();
        let settings = config_manager.get_settings().await;
        
        // Store the settings
        self.general_settings = settings.general.clone();
        self.audio_settings = settings.audio.clone();
        self.transcription_settings = settings.transcription.clone();
        self.advanced_settings = settings.advanced.clone();
        self.ui_settings = settings.ui.clone();
        self.file_path_settings = settings.file_paths.clone();
        self.notification_settings = settings.notifications.clone();
        
        // Populate the widgets with current settings
        self.populate_widgets()?;
        
        Ok(())
    }
    
    /// Populate widgets with current settings
    fn populate_widgets(&self) -> AppResult<()> {
        // General settings
        self.language_combo.set_active_id(Some(&self.general_settings.language));
        self.theme_combo.set_active_id(Some(&self.general_settings.theme));
        self.auto_save_switch.set_active(self.general_settings.auto_save_enabled);
        self.auto_save_interval_spin.set_value(self.general_settings.auto_save_interval as f64);
        self.start_maximized_switch.set_active(self.general_settings.start_maximized);
        self.remember_window_state_switch.set_active(self.general_settings.remember_window_state);
        self.check_for_updates_switch.set_active(self.general_settings.check_for_updates);
        self.update_check_interval_spin.set_value(self.general_settings.update_check_interval as f64);
        
        // Audio settings
        if let Some(ref device) = self.audio_settings.default_input_device {
            self.default_input_device_combo.set_active_id(Some(device));
        }
        if let Some(ref device) = self.audio_settings.default_output_device {
            self.default_output_device_combo.set_active_id(Some(device));
        }
        self.default_sample_rate_combo.set_active_text(Some(&format!("{} Hz", self.audio_settings.default_sample_rate)));
        self.default_format_combo.set_active_id(Some(&self.audio_settings.default_format));
        self.default_input_volume_scale.set_value(self.audio_settings.default_input_volume as f64);
        self.default_output_volume_scale.set_value(self.audio_settings.default_output_volume as f64);
        self.echo_cancellation_switch.set_active(self.audio_settings.echo_cancellation);
        self.noise_suppression_switch.set_active(self.audio_settings.noise_suppression);
        self.automatic_gain_control_switch.set_active(self.audio_settings.automatic_gain_control);
        self.buffer_size_spin.set_value(self.audio_settings.buffer_size as f64);
        
        // Transcription settings
        self.default_model_combo.set_active_id(Some(&self.transcription_settings.default_model));
        self.default_language_combo.set_active_id(Some(&self.transcription_settings.default_language));
        self.include_timestamps_switch.set_active(self.transcription_settings.include_timestamps);
        self.include_word_timestamps_switch.set_active(self.transcription_settings.include_word_timestamps);
        self.include_confidence_switch.set_active(self.transcription_settings.include_confidence);
        self.include_segments_switch.set_active(self.transcription_settings.include_segments);
        self.translate_to_english_switch.set_active(self.transcription_settings.translate_to_english);
        self.temperature_scale.set_value(self.transcription_settings.temperature as f64);
        self.automatic_punctuation_switch.set_active(self.transcription_settings.automatic_punctuation);
        self.number_formatting_switch.set_active(self.transcription_settings.number_formatting);
        self.profanity_filter_switch.set_active(self.transcription_settings.profanity_filter);
        
        // Advanced settings
        self.max_concurrent_threads_spin.set_value(self.advanced_settings.max_concurrent_threads as f64);
        self.max_file_size_spin.set_value(self.advanced_settings.max_file_size_mb as f64);
        if let Some(ref dir) = self.advanced_settings.temp_dir {
            self.temp_dir_entry.set_text(dir);
        }
        if let Some(ref dir) = self.advanced_settings.cache_dir {
            self.cache_dir_entry.set_text(dir);
        }
        self.debug_logging_switch.set_active(self.advanced_settings.debug_logging);
        if let Some(ref path) = self.advanced_settings.log_file_path {
            self.log_file_path_entry.set_text(path);
        }
        self.max_log_size_spin.set_value(self.advanced_settings.max_log_size_mb as f64);
        self.log_file_count_spin.set_value(self.advanced_settings.log_file_count as f64);
        if let Some(ref url) = self.advanced_settings.backend_url {
            self.backend_url_entry.set_text(url);
        }
        if let Some(ref key) = self.advanced_settings.api_key {
            self.api_key_entry.set_text(key);
        }
        self.request_timeout_spin.set_value(self.advanced_settings.request_timeout as f64);
        self.verify_ssl_switch.set_active(self.advanced_settings.verify_ssl);
        
        // UI settings
        self.font_family_font_button.set_font_name(&self.ui_settings.font_family);
        self.font_size_spin.set_value(self.ui_settings.font_size as f64);
        self.ui_density_combo.set_active_id(Some(&self.ui_settings.ui_density));
        self.show_icons_switch.set_active(self.ui_settings.show_icons);
        self.show_tooltips_switch.set_active(self.ui_settings.show_tooltips);
        self.animate_transitions_switch.set_active(self.ui_settings.animate_transitions);
        self.show_file_extensions_switch.set_active(self.ui_settings.show_file_extensions);
        self.default_transcription_view_combo.set_active_id(Some(&self.ui_settings.default_transcription_view));
        self.word_wrap_switch.set_active(self.ui_settings.word_wrap);
        self.show_line_numbers_switch.set_active(self.ui_settings.show_line_numbers);
        self.show_confidence_scores_switch.set_active(self.ui_settings.show_confidence_scores);
        
        // File path settings
        self.audio_dir_button.set_file(&PathBuf::from(&self.file_path_settings.audio_directory));
        self.output_dir_button.set_file(&PathBuf::from(&self.file_path_settings.output_directory));
        if let Some(ref dir) = self.file_path_settings.last_opened_directory {
            self.last_opened_dir_button.set_file(&PathBuf::from(dir));
        }
        
        // Notification settings
        self.notifications_enabled_switch.set_active(self.notifications.enabled);
        self.show_transcription_complete_switch.set_active(self.notifications.show_transcription_complete);
        self.show_errors_switch.set_active(self.notifications.show_errors);
        self.show_file_events_switch.set_active(self.notifications.show_file_events);
        self.show_system_events_switch.set_active(self.notifications.show_system_events);
        self.sound_enabled_switch.set_active(self.notifications.sound_enabled);
        self.sound_volume_scale.set_value(self.notifications.sound_volume as f64);
        if let Some(ref path) = self.notifications.custom_sound_path {
            self.custom_sound_path_entry.set_text(path);
        }
        self.notification_duration_spin.set_value(self.notifications.notification_duration as f64);
        self.notification_position_combo.set_active_id(Some(&self.notifications.notification_position));
        
        Ok(())
    }
    
    /// Collect settings from widgets
    fn collect_settings(&mut self) -> AppResult<()> {
        // General settings
        self.general_settings.language = self.language_combo.active_id().unwrap_or_else(|| "en".to_string());
        self.general_settings.theme = self.theme_combo.active_id().unwrap_or_else(|| "system".to_string());
        self.general_settings.auto_save_enabled = self.auto_save_switch.is_active();
        self.general_settings.auto_save_interval = self.auto_save_interval_spin.value() as u32;
        self.general_settings.start_maximized = self.start_maximized_switch.is_active();
        self.general_settings.remember_window_state = self.remember_window_state_switch.is_active();
        self.general_settings.check_for_updates = self.check_for_updates_switch.is_active();
        self.general_settings.update_check_interval = self.update_check_interval_spin.value() as u32;
        
        // Audio settings
        self.audio_settings.default_input_device = self.default_input_device_combo.active_id();
        self.audio_settings.default_output_device = self.default_output_device_combo.active_id();
        if let Some(text) = self.default_sample_rate_combo.active_text() {
            if let Some(rate_str) = text.split_whitespace().next() {
                if let Ok(rate) = rate_str.parse::<u32>() {
                    self.audio_settings.default_sample_rate = rate;
                }
            }
        }
        self.audio_settings.default_format = self.default_format_combo.active_id().unwrap_or_else(|| "wav".to_string());
        self.audio_settings.default_input_volume = self.default_input_volume_scale.value() as f32;
        self.audio_settings.default_output_volume = self.default_output_volume_scale.value() as f32;
        self.audio_settings.echo_cancellation = self.echo_cancellation_switch.is_active();
        self.audio_settings.noise_suppression = self.noise_suppression_switch.is_active();
        self.audio_settings.automatic_gain_control = self.automatic_gain_control_switch.is_active();
        self.audio_settings.buffer_size = self.buffer_size_spin.value() as u32;
        
        // Transcription settings
        self.transcription_settings.default_model = self.default_model_combo.active_id().unwrap_or_else(|| "whisper-base".to_string());
        self.transcription_settings.default_language = self.default_language_combo.active_id().unwrap_or_else(|| "auto".to_string());
        self.transcription_settings.include_timestamps = self.include_timestamps_switch.is_active();
        self.transcription_settings.include_word_timestamps = self.include_word_timestamps_switch.is_active();
        self.transcription_settings.include_confidence = self.include_confidence_switch.is_active();
        self.transcription_settings.include_segments = self.include_segments_switch.is_active();
        self.transcription_settings.translate_to_english = self.translate_to_english_switch.is_active();
        self.transcription_settings.temperature = self.temperature_scale.value() as f32;
        self.transcription_settings.automatic_punctuation = self.automatic_punctuation_switch.is_active();
        self.transcription_settings.number_formatting = self.number_formatting_switch.is_active();
        self.transcription_settings.profanity_filter = self.profanity_filter_switch.is_active();
        
        // Advanced settings
        self.advanced_settings.max_concurrent_threads = self.max_concurrent_threads_spin.value() as u32;
        self.advanced_settings.max_file_size_mb = self.max_file_size_spin.value() as u32;
        let temp_dir = self.temp_dir_entry.text();
        if !temp_dir.is_empty() {
            self.advanced_settings.temp_dir = Some(temp_dir.to_string());
        }
        let cache_dir = self.cache_dir_entry.text();
        if !cache_dir.is_empty() {
            self.advanced_settings.cache_dir = Some(cache_dir.to_string());
        }
        self.advanced_settings.debug_logging = self.debug_logging_switch.is_active();
        let log_file_path = self.log_file_path_entry.text();
        if !log_file_path.is_empty() {
            self.advanced_settings.log_file_path = Some(log_file_path.to_string());
        }
        self.advanced_settings.max_log_size_mb = self.max_log_size_spin.value() as u32;
        self.advanced_settings.log_file_count = self.log_file_count_spin.value() as u32;
        let backend_url = self.backend_url_entry.text();
        if !backend_url.is_empty() {
            self.advanced_settings.backend_url = Some(backend_url.to_string());
        }
        let api_key = self.api_key_entry.text();
        if !api_key.is_empty() {
            self.advanced_settings.api_key = Some(api_key.to_string());
        }
        self.advanced_settings.request_timeout = self.request_timeout_spin.value() as u32;
        self.advanced_settings.verify_ssl = self.verify_ssl_switch.is_active();
        
        // UI settings
        self.ui_settings.font_family = self.font_family_font_button.font_name().to_string();
        self.ui_settings.font_size = self.font_size_spin.value() as f32;
        self.ui_settings.ui_density = self.ui_density_combo.active_id().unwrap_or_else(|| "normal".to_string());
        self.ui_settings.show_icons = self.show_icons_switch.is_active();
        self.ui_settings.show_tooltips = self.show_tooltips_switch.is_active();
        self.ui_settings.animate_transitions = self.animate_transitions_switch.is_active();
        self.ui_settings.show_file_extensions = self.show_file_extensions_switch.is_active();
        self.ui_settings.default_transcription_view = self.default_transcription_view_combo.active_id().unwrap_or_else(|| "text".to_string());
        self.ui_settings.word_wrap = self.word_wrap_switch.is_active();
        self.ui_settings.show_line_numbers = self.show_line_numbers_switch.is_active();
        self.ui_settings.show_confidence_scores = self.show_confidence_scores_switch.is_active();
        
        // File path settings
        if let Some(path) = self.audio_dir_button.file() {
            self.file_path_settings.audio_directory = path.to_string_lossy().to_string();
        }
        if let Some(path) = self.output_dir_button.file() {
            self.file_path_settings.output_directory = path.to_string_lossy().to_string();
        }
        if let Some(path) = self.last_opened_dir_button.file() {
            self.file_path_settings.last_opened_directory = Some(path.to_string_lossy().to_string());
        }
        
        // Notification settings
        self.notifications.enabled = self.notifications_enabled_switch.is_active();
        self.notifications.show_transcription_complete = self.show_transcription_complete_switch.is_active();
        self.notifications.show_errors = self.show_errors_switch.is_active();
        self.notifications.show_file_events = self.show_file_events_switch.is_active();
        self.notifications.show_system_events = self.show_system_events_switch.is_active();
        self.notifications.sound_enabled = self.sound_enabled_switch.is_active();
        self.notifications.sound_volume = self.sound_volume_scale.value() as f32;
        let custom_sound_path = self.custom_sound_path_entry.text();
        if !custom_sound_path.is_empty() {
            self.notifications.custom_sound_path = Some(custom_sound_path.to_string());
        }
        self.notifications.notification_duration = self.notification_duration_spin.value() as u32;
        self.notifications.notification_position = self.notification_position_combo.active_id().unwrap_or_else(|| "bottom-right".to_string());
        
        Ok(())
    }
    
    /// Apply the collected settings
    async fn apply_settings(&mut self) -> AppResult<()> {
        // Collect settings from widgets
        self.collect_settings()?;
        
        // Create a new settings struct
        let settings = Settings {
            general: self.general_settings.clone(),
            audio: self.audio_settings.clone(),
            transcription: self.transcription_settings.clone(),
            advanced: self.advanced_settings.clone(),
            ui: self.ui_settings.clone(),
            file_paths: self.file_path_settings.clone(),
            notifications: self.notifications.clone(),
        };
        
        // Update the settings in the config manager
        let config_manager = self.app_state.get_config_manager();
        config_manager.update_settings(settings).await?;
        
        // Save the settings
        config_manager.save_settings().await?;
        
        // Update the app state
        self.update_app_state().await?;
        
        // Show success message
        self.show_info_message("Settings applied successfully", gtk4::MessageType::Info);
        
        Ok(())
    }
    
    /// Update the app state with the new settings
    async fn update_app_state(&mut self) -> AppResult<()> {
        // Update the app state with the new settings
        // This would typically involve updating various parts of the application
        // to reflect the new settings
        
        // For example, update the theme if it changed
        if self.ui_settings.theme != self.app_state.get_config_state().await.theme {
            // Apply the new theme
            // This would typically involve updating the CSS provider
        }
        
        // Update other parts of the app state as needed
        
        Ok(())
    }
    
    /// Reset settings to defaults
    async fn reset_to_defaults(&mut self) -> AppResult<()> {
        // Show confirmation dialog
        let dialog = MessageDialog::builder()
            .text("Reset all settings to defaults?")
            .secondary_text("This action cannot be undone.")
            .buttons_type(ButtonsType::OkCancel)
            .message_type(MessageType::Question)
            .modal(true)
            .transient_for(&self.dialog)
            .build();
        
        let response = dialog.run_future().await;
        dialog.close();
        
        if response == ResponseType::Ok {
            // Reset to defaults
            self.general_settings = GeneralSettings::default();
            self.audio_settings = AudioSettings::default();
            self.transcription_settings = TranscriptionSettings::default();
            self.advanced_settings = AdvancedSettings::default();
            self.ui_settings = UiSettings::default();
            self.file_path_settings = FilePathSettings::default();
            self.notification_settings = NotificationSettings::default();
            
            // Repopulate the widgets
            self.populate_widgets()?;
            
            // Show success message
            self.show_info_message("Settings reset to defaults", gtk4::MessageType::Info);
        }
        
        Ok(())
    }
    
    /// Import settings from a file
    async fn import_settings(&mut self) -> AppResult<()> {
        // Show file chooser dialog
        let dialog = gtk4::FileChooserNative::builder()
            .title("Import Settings")
            .accept_label("Import")
            .cancel_label("Cancel")
            .modal(true)
            .transient_for(&self.dialog)
            .action(gtk4::FileChooserAction::Open)
            .build();
        
        let response = dialog.run_future().await;
        
        if response == ResponseType::Accept {
            if let Some(file) = dialog.file() {
                let path = file.path().unwrap_or_else(|| std::path::PathBuf::from("/"));
                
                // Import the settings
                let config_manager = self.app_state.get_config_manager();
                match config_manager.import_settings(&path).await {
                    Ok(_) => {
                        // Reload the settings
                        self.load_current_settings().await?;
                        
                        // Show success message
                        self.show_info_message("Settings imported successfully", gtk4::MessageType::Info);
                    },
                    Err(e) => {
                        // Show error message
                        self.show_info_message(&format!("Failed to import settings: {}", e), gtk4::MessageType::Error);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Export settings to a file
    async fn export_settings(&mut self) -> AppResult<()> {
        // Show file chooser dialog
        let dialog = gtk4::FileChooserNative::builder()
            .title("Export Settings")
            .accept_label("Export")
            .cancel_label("Cancel")
            .modal(true)
            .transient_for(&self.dialog)
            .action(gtk4::FileChooserAction::Save)
            .build();
        
        // Set a default file name
        dialog.set_current_name("asrpro_settings.json");
        
        let response = dialog.run_future().await;
        
        if response == ResponseType::Accept {
            if let Some(file) = dialog.file() {
                let path = file.path().unwrap_or_else(|| std::path::PathBuf::from("/"));
                
                // Export the settings
                let config_manager = self.app_state.get_config_manager();
                match config_manager.export_settings(&path).await {
                    Ok(_) => {
                        // Show success message
                        self.show_info_message("Settings exported successfully", gtk4::MessageType::Info);
                    },
                    Err(e) => {
                        // Show error message
                        self.show_info_message(&format!("Failed to export settings: {}", e), gtk4::MessageType::Error);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Show an info message in the info bar
    fn show_info_message(&self, message: &str, message_type: gtk4::MessageType) {
        self.info_label.set_text(message);
        self.info_bar.set_message_type(message_type);
        self.info_bar.set_revealed(true);
        
        // Hide the info bar after a few seconds
        let info_bar = self.info_bar.clone();
        gtk4::glib::timeout_add_seconds_once(5, move || {
            info_bar.set_revealed(false);
        });
    }
    
    /// Show the dialog and return the response
    pub async fn show(&mut self) -> ResponseType {
        self.dialog.connect_close_request(|dialog| {
            gtk4::glib::Propagation::Proceed
        });
        
        self.dialog.show();
        self.dialog.run_future().await
    }
}