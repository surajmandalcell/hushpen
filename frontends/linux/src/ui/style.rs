//! Application styling for ASRPro
//! 
//! This module contains CSS styling for the application, including color schemes,
//! typography, and styles for common UI elements.

use gtk4::prelude::*;
use gtk4::{StyleContext, CssProvider, Application};
use gtk4::glib::{Bytes, MainContext};
use std::sync::Once;

static INIT: Once = Once::new();

/// Application styling manager
pub struct ApplicationStyle {
    css_provider: CssProvider,
    theme: Theme,
}

#[derive(Debug, Clone, Copy)]
pub enum Theme {
    Light,
    Dark,
    System,
}

impl ApplicationStyle {
    /// Create a new ApplicationStyle instance
    pub fn new(theme: Theme) -> Self {
        let css_provider = CssProvider::new();
        
        let mut style = Self {
            css_provider,
            theme,
        };
        
        // Initialize the styling
        style.load_styles();
        
        style
    }
    
    /// Load and apply the CSS styles
    fn load_styles(&mut self) {
        let css = self.generate_css();
        
        // Load the CSS
        self.css_provider.load_from_data(&css);
        
        // Apply the CSS to all widgets
        INIT.call_once(|| {
            if let Some(display) = gtk4::gdk::Display::default() {
                StyleContext::add_provider_for_display(
                    &display,
                    &self.css_provider,
                    gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
        });
    }
    
    /// Generate CSS based on the current theme
    fn generate_css(&self) -> String {
        match self.theme {
            Theme::Light => self.generate_light_theme(),
            Theme::Dark => self.generate_dark_theme(),
            Theme::System => {
                // For now, default to light theme for system
                // In a real implementation, we would check the system theme
                self.generate_light_theme()
            }
        }
    }
    
    /// Generate light theme CSS
    fn generate_light_theme(&self) -> String {
        r#"
/* ASRPro GTK4 Light Theme */

/* Window and main containers */
window {
    background-color: #fafafa;
    color: #333333;
}

headerbar {
    background-color: #ffffff;
    border-bottom: 1px solid #e0e0e0;
    min-height: 48px;
    padding: 0 12px;
}

headerbar > box > * {
    margin: 0 6px;
}

/* Menu styling */
menubar {
    background-color: #ffffff;
    color: #333333;
    border-bottom: 1px solid #e0e0e0;
}

menu {
    background-color: #ffffff;
    color: #333333;
    border: 1px solid #d0d0d0;
    border-radius: 6px;
    padding: 4px;
}

menuitem {
    padding: 8px 12px;
    border-radius: 4px;
    transition: background-color 0.2s ease;
}

menuitem:hover {
    background-color: #f0f0f0;
}

menuitem:active {
    background-color: #e0e0e0;
}

menu separator {
    margin: 4px 0;
    background-color: #e0e0e0;
    min-height: 1px;
}

/* Button styling */
button {
    background-color: #4a90e2;
    color: white;
    border: none;
    border-radius: 6px;
    padding: 8px 16px;
    font-weight: 500;
    transition: all 0.2s ease;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

button:hover {
    background-color: #357abd;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.15);
}

button:active {
    background-color: #2968a3;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

button:disabled {
    background-color: #e0e0e0;
    color: #999999;
    box-shadow: none;
}

button.suggested-action {
    background-color: #4caf50;
}

button.suggested-action:hover {
    background-color: #45a049;
}

button.destructive-action {
    background-color: #f44336;
}

button.destructive-action:hover {
    background-color: #d32f2f;
}

/* Entry (text input) styling */
entry {
    background-color: #ffffff;
    border: 1px solid #d0d0d0;
    border-radius: 4px;
    padding: 8px 12px;
    color: #333333;
    transition: all 0.2s ease;
}

entry:focus {
    border-color: #4a90e2;
    box-shadow: 0 0 0 2px rgba(74, 144, 226, 0.2);
}

entry:disabled {
    background-color: #f5f5f5;
    color: #999999;
}

/* Label styling */
label {
    color: #333333;
}

label.title {
    font-size: 1.2em;
    font-weight: 600;
}

label.subtitle {
    font-size: 1.0em;
    font-weight: 500;
    color: #666666;
}

label.status {
    font-size: 0.9em;
    color: #666666;
}

/* Progress bar styling */
progressbar {
    font-size: 0.9em;
}

progressbar trough {
    background-color: #e0e0e0;
    border-radius: 3px;
    min-height: 6px;
}

progressbar progress {
    background-color: #4a90e2;
    border-radius: 3px;
    min-height: 6px;
    transition: width 0.3s ease;
}

/* Status bar styling */
statusbar {
    background-color: #f5f5f5;
    border-top: 1px solid #e0e0e0;
    color: #666666;
    font-size: 0.9em;
    padding: 4px 8px;
}

/* Paned (split view) styling */
paned > separator {
    background-color: #d0d0d0;
    min-width: 1px;
    min-height: 1px;
}

paned.horizontal > separator {
    background-image: linear-gradient(to right, #d0d0d0, #e0e0e0, #d0d0d0);
}

paned.vertical > separator {
    background-image: linear-gradient(to bottom, #d0d0d0, #e0e0e0, #d0d0d0);
}

/* Box and container styling */
box.sidebar {
    background-color: #f8f8f8;
    border-right: 1px solid #e0e0e0;
}

box.content {
    background-color: #ffffff;
}

/* List box styling */
list {
    background-color: #ffffff;
    border: 1px solid #d0d0d0;
    border-radius: 6px;
}

list row {
    padding: 8px 12px;
    border-bottom: 1px solid #f0f0f0;
    transition: background-color 0.2s ease;
}

list row:hover {
    background-color: #f8f8f8;
}

list row:selected {
    background-color: #e3f2fd;
    color: #1976d2;
}

list row:last-child {
    border-bottom: none;
}

/* Scrolled window styling */
scrolledwindow {
    border: 1px solid #d0d0d0;
    border-radius: 6px;
}

/* Text view styling */
textview {
    background-color: #ffffff;
    border: 1px solid #d0d0d0;
    border-radius: 4px;
    padding: 8px;
}

textview text {
    background-color: transparent;
}

/* Switch styling */
switch {
    background-color: #d0d0d0;
    border: 1px solid #b0b0b0;
    border-radius: 12px;
}

switch:checked {
    background-color: #4a90e2;
    border-color: #357abd;
}

switch slider {
    background-color: #ffffff;
    border: 1px solid #b0b0b0;
    border-radius: 50%;
    min-width: 16px;
    min-height: 16px;
    margin: 2px;
}

switch:checked slider {
    border-color: #357abd;
}

/* Check button styling */
checkbutton check {
    background-color: #ffffff;
    border: 2px solid #d0d0d0;
    border-radius: 3px;
    min-width: 16px;
    min-height: 16px;
    color: transparent;
    transition: all 0.2s ease;
}

checkbutton:checked check {
    background-color: #4a90e2;
    border-color: #357abd;
    color: white;
}

/* Radio button styling */
radiobutton radio {
    background-color: #ffffff;
    border: 2px solid #d0d0d0;
    border-radius: 50%;
    min-width: 16px;
    min-height: 16px;
    color: transparent;
    transition: all 0.2s ease;
}

radiobutton:checked radio {
    background-color: #4a90e2;
    border-color: #357abd;
    color: white;
}

/* Scale (slider) styling */
scale trough {
    background-color: #e0e0e0;
    border-radius: 3px;
    min-height: 6px;
}

scale trough highlight {
    background-color: #4a90e2;
    border-radius: 3px;
    min-height: 6px;
}

scale slider {
    background-color: #ffffff;
    border: 1px solid #d0d0d0;
    border-radius: 50%;
    min-width: 16px;
    min-height: 16px;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
    transition: all 0.2s ease;
}

scale slider:hover {
    border-color: #4a90e2;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.15);
}

/* Spinner styling */
spinner {
    color: #4a90e2;
}

/* Info bar styling */
infobar {
    border: 1px solid #d0d0d0;
    border-radius: 4px;
}

infobar.info {
    background-color: #e3f2fd;
    color: #1976d2;
}

infobar.warning {
    background-color: #fff3cd;
    color: #856404;
}

infobar.error {
    background-color: #f8d7da;
    color: #721c24;
}

infobar.question {
    background-color: #d1ecf1;
    color: #0c5460;
}

/* Dialog styling */
dialog {
    background-color: #ffffff;
    border: 1px solid #d0d0d0;
    border-radius: 8px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}

messagedialog {
    background-color: #ffffff;
    border: 1px solid #d0d0d0;
    border-radius: 8px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}

/* About dialog styling */
aboutdialog {
    background-color: #ffffff;
}

/* Custom application-specific styles */
.sidebar-button {
    padding: 12px 16px;
    margin: 4px;
    border-radius: 6px;
    background-color: transparent;
    color: #333333;
    font-weight: 500;
    text-align: left;
}

.sidebar-button:hover {
    background-color: #f0f0f0;
}

.sidebar-button:active {
    background-color: #e0e0e0;
}

.sidebar-button.active {
    background-color: #e3f2fd;
    color: #1976d2;
}

.content-card {
    background-color: #ffffff;
    border: 1px solid #e0e0e0;
    border-radius: 8px;
    padding: 16px;
    margin: 8px;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
}

.content-card-title {
    font-size: 1.1em;
    font-weight: 600;
    color: #333333;
    margin-bottom: 8px;
}

.content-card-content {
    color: #666666;
    font-size: 0.95em;
}

.transcription-result {
    background-color: #f8f8f8;
    border: 1px solid #e0e0e0;
    border-radius: 6px;
    padding: 16px;
    margin: 8px 0;
    font-family: monospace;
    font-size: 0.95em;
    line-height: 1.4;
}

.recording-indicator {
    color: #f44336;
    font-weight: 600;
    animation: pulse 1.5s infinite;
}

@keyframes pulse {
    0% { opacity: 1; }
    50% { opacity: 0.5; }
    100% { opacity: 1; }
}
        "#.to_string()
    }
    
    /// Generate dark theme CSS
    fn generate_dark_theme(&self) -> String {
        r#"
/* ASRPro GTK4 Dark Theme */

/* Window and main containers */
window {
    background-color: #2d2d2d;
    color: #e0e0e0;
}

headerbar {
    background-color: #383838;
    border-bottom: 1px solid #505050;
    min-height: 48px;
    padding: 0 12px;
    color: #e0e0e0;
}

headerbar > box > * {
    margin: 0 6px;
}

/* Menu styling */
menubar {
    background-color: #383838;
    color: #e0e0e0;
    border-bottom: 1px solid #505050;
}

menu {
    background-color: #383838;
    color: #e0e0e0;
    border: 1px solid #505050;
    border-radius: 6px;
    padding: 4px;
}

menuitem {
    padding: 8px 12px;
    border-radius: 4px;
    transition: background-color 0.2s ease;
}

menuitem:hover {
    background-color: #505050;
}

menuitem:active {
    background-color: #606060;
}

menu separator {
    margin: 4px 0;
    background-color: #505050;
    min-height: 1px;
}

/* Button styling */
button {
    background-color: #4a90e2;
    color: white;
    border: none;
    border-radius: 6px;
    padding: 8px 16px;
    font-weight: 500;
    transition: all 0.2s ease;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
}

button:hover {
    background-color: #357abd;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.4);
}

button:active {
    background-color: #2968a3;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
}

button:disabled {
    background-color: #505050;
    color: #999999;
    box-shadow: none;
}

button.suggested-action {
    background-color: #4caf50;
}

button.suggested-action:hover {
    background-color: #45a049;
}

button.destructive-action {
    background-color: #f44336;
}

button.destructive-action:hover {
    background-color: #d32f2f;
}

/* Entry (text input) styling */
entry {
    background-color: #404040;
    border: 1px solid #505050;
    border-radius: 4px;
    padding: 8px 12px;
    color: #e0e0e0;
    transition: all 0.2s ease;
}

entry:focus {
    border-color: #4a90e2;
    box-shadow: 0 0 0 2px rgba(74, 144, 226, 0.3);
}

entry:disabled {
    background-color: #333333;
    color: #999999;
}

/* Label styling */
label {
    color: #e0e0e0;
}

label.title {
    font-size: 1.2em;
    font-weight: 600;
}

label.subtitle {
    font-size: 1.0em;
    font-weight: 500;
    color: #b0b0b0;
}

label.status {
    font-size: 0.9em;
    color: #b0b0b0;
}

/* Progress bar styling */
progressbar {
    font-size: 0.9em;
}

progressbar trough {
    background-color: #505050;
    border-radius: 3px;
    min-height: 6px;
}

progressbar progress {
    background-color: #4a90e2;
    border-radius: 3px;
    min-height: 6px;
    transition: width 0.3s ease;
}

/* Status bar styling */
statusbar {
    background-color: #333333;
    border-top: 1px solid #505050;
    color: #b0b0b0;
    font-size: 0.9em;
    padding: 4px 8px;
}

/* Paned (split view) styling */
paned > separator {
    background-color: #505050;
    min-width: 1px;
    min-height: 1px;
}

paned.horizontal > separator {
    background-image: linear-gradient(to right, #505050, #606060, #505050);
}

paned.vertical > separator {
    background-image: linear-gradient(to bottom, #505050, #606060, #505050);
}

/* Box and container styling */
box.sidebar {
    background-color: #333333;
    border-right: 1px solid #505050;
}

box.content {
    background-color: #2d2d2d;
}

/* List box styling */
list {
    background-color: #383838;
    border: 1px solid #505050;
    border-radius: 6px;
}

list row {
    padding: 8px 12px;
    border-bottom: 1px solid #505050;
    transition: background-color 0.2s ease;
}

list row:hover {
    background-color: #505050;
}

list row:selected {
    background-color: #1e3a5f;
    color: #4a90e2;
}

list row:last-child {
    border-bottom: none;
}

/* Scrolled window styling */
scrolledwindow {
    border: 1px solid #505050;
    border-radius: 6px;
}

/* Text view styling */
textview {
    background-color: #383838;
    border: 1px solid #505050;
    border-radius: 4px;
    padding: 8px;
    color: #e0e0e0;
}

textview text {
    background-color: transparent;
}

/* Switch styling */
switch {
    background-color: #505050;
    border: 1px solid #606060;
    border-radius: 12px;
}

switch:checked {
    background-color: #4a90e2;
    border-color: #357abd;
}

switch slider {
    background-color: #e0e0e0;
    border: 1px solid #606060;
    border-radius: 50%;
    min-width: 16px;
    min-height: 16px;
    margin: 2px;
}

switch:checked slider {
    border-color: #357abd;
}

/* Check button styling */
checkbutton check {
    background-color: #404040;
    border: 2px solid #505050;
    border-radius: 3px;
    min-width: 16px;
    min-height: 16px;
    color: transparent;
    transition: all 0.2s ease;
}

checkbutton:checked check {
    background-color: #4a90e2;
    border-color: #357abd;
    color: white;
}

/* Radio button styling */
radiobutton radio {
    background-color: #404040;
    border: 2px solid #505050;
    border-radius: 50%;
    min-width: 16px;
    min-height: 16px;
    color: transparent;
    transition: all 0.2s ease;
}

radiobutton:checked radio {
    background-color: #4a90e2;
    border-color: #357abd;
    color: white;
}

/* Scale (slider) styling */
scale trough {
    background-color: #505050;
    border-radius: 3px;
    min-height: 6px;
}

scale trough highlight {
    background-color: #4a90e2;
    border-radius: 3px;
    min-height: 6px;
}

scale slider {
    background-color: #e0e0e0;
    border: 1px solid #505050;
    border-radius: 50%;
    min-width: 16px;
    min-height: 16px;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
    transition: all 0.2s ease;
}

scale slider:hover {
    border-color: #4a90e2;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.4);
}

/* Spinner styling */
spinner {
    color: #4a90e2;
}

/* Info bar styling */
infobar {
    border: 1px solid #505050;
    border-radius: 4px;
}

infobar.info {
    background-color: #1e3a5f;
    color: #4a90e2;
}

infobar.warning {
    background-color: #5d4e02;
    color: #f9c74f;
}

infobar.error {
    background-color: #5d1a1a;
    color: #f94144;
}

infobar.question {
    background-color: #1a4d5d;
    color: #4cc9f0;
}

/* Dialog styling */
dialog {
    background-color: #383838;
    border: 1px solid #505050;
    border-radius: 8px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
    color: #e0e0e0;
}

messagedialog {
    background-color: #383838;
    border: 1px solid #505050;
    border-radius: 8px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
    color: #e0e0e0;
}

/* About dialog styling */
aboutdialog {
    background-color: #383838;
    color: #e0e0e0;
}

/* Custom application-specific styles */
.sidebar-button {
    padding: 12px 16px;
    margin: 4px;
    border-radius: 6px;
    background-color: transparent;
    color: #e0e0e0;
    font-weight: 500;
    text-align: left;
}

.sidebar-button:hover {
    background-color: #505050;
}

.sidebar-button:active {
    background-color: #606060;
}

.sidebar-button.active {
    background-color: #1e3a5f;
    color: #4a90e2;
}

.content-card {
    background-color: #383838;
    border: 1px solid #505050;
    border-radius: 8px;
    padding: 16px;
    margin: 8px;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
}

.content-card-title {
    font-size: 1.1em;
    font-weight: 600;
    color: #e0e0e0;
    margin-bottom: 8px;
}

.content-card-content {
    color: #b0b0b0;
    font-size: 0.95em;
}

.transcription-result {
    background-color: #333333;
    border: 1px solid #505050;
    border-radius: 6px;
    padding: 16px;
    margin: 8px 0;
    font-family: monospace;
    font-size: 0.95em;
    line-height: 1.4;
    color: #e0e0e0;
}

.recording-indicator {
    color: #f44336;
    font-weight: 600;
    animation: pulse 1.5s infinite;
}

@keyframes pulse {
    0% { opacity: 1; }
    50% { opacity: 0.5; }
    100% { opacity: 1; }
}
        "#.to_string()
    }
    
    /// Set the theme
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.load_styles();
    }
    
    /// Get the current theme
    pub fn get_theme(&self) -> Theme {
        self.theme
    }
    
    /// Load CSS from a file
    pub fn load_from_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let css = std::fs::read_to_string(path)?;
        self.css_provider.load_from_data(&css);
        Ok(())
    }
    
    /// Load CSS from a resource
    pub fn load_from_resource(&self, resource_path: &str) {
        let resource_bytes = gtk4::gio::resources_lookup_data(
            resource_path,
            gtk4::gio::ResourceLookupFlags::NONE,
        );
        
        if let Ok(bytes) = resource_bytes {
            let css = String::from_utf8_lossy(&bytes);
            self.css_provider.load_from_data(&css);
        }
    }
}

/// Initialize the application styling
pub fn initialize_styling(theme: Theme) -> ApplicationStyle {
    ApplicationStyle::new(theme)
}

/// Apply styling to a specific widget
pub fn apply_widget_class(widget: &impl IsA<gtk4::Widget>, class_name: &str) {
    let context = widget.style_context();
    context.add_class(class_name);
}

/// Remove styling from a specific widget
pub fn remove_widget_class(widget: &impl IsA<gtk4::Widget>, class_name: &str) {
    let context = widget.style_context();
    context.remove_class(class_name);
}