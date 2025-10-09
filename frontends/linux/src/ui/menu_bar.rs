//! Menu bar component for the ASRPro application
//! 
//! This module contains the MenuBar struct which provides the application menu
//! with File, Edit, View, and Help menus.

use gtk4::prelude::*;
use gtk4::{
    ApplicationWindow, gio::Menu, gio::MenuItem, gio::SimpleAction,
    gio::SimpleActionGroup, Widget, Dialog, MessageDialog, ButtonsType,
    MessageType, AboutDialog, License, ResponseType
};
use gtk4::glib::{Variant, VariantTy};
use std::sync::Arc;

use crate::models::AppState;
use crate::utils::AppError;

/// Menu bar for the application
pub struct MenuBar {
    window: ApplicationWindow,
    app_state: Arc<AppState>,
    menu_model: Menu,
    action_group: SimpleActionGroup,
}

impl MenuBar {
    /// Create a new MenuBar instance
    pub fn new(window: &ApplicationWindow, app_state: Arc<AppState>) -> Result<Self, AppError> {
        // Create the menu model
        let menu_model = Menu::new();
        
        // Create the action group
        let action_group = SimpleActionGroup::new();
        
        // Create the menu bar instance
        let mut menu_bar = Self {
            window: window.clone(),
            app_state,
            menu_model,
            action_group,
        };
        
        // Set up the menu structure
        menu_bar.setup_menu_structure()?;
        
        // Set up menu actions
        menu_bar.setup_actions()?;
        
        // Insert the action group into the window
        window.insert_action_group("app", Some(&menu_bar.action_group));
        
        Ok(menu_bar)
    }
    
    /// Set up the menu structure
    fn setup_menu_structure(&mut self) -> Result<(), AppError> {
        // Create File menu
        let file_menu = Menu::new();
        file_menu.append(Some("New"), Some("app.new"));
        file_menu.append(Some("Open"), Some("app.open"));
        file_menu.append(Some("Save"), Some("app.save"));
        file_menu.append(Some("Save As"), Some("app.save_as"));
        file_menu.append(None, None);
        file_menu.append(Some("Preferences"), Some("app.preferences"));
        file_menu.append(None, None);
        file_menu.append(Some("Exit"), Some("app.exit"));
        
        // Create Edit menu
        let edit_menu = Menu::new();
        edit_menu.append(Some("Undo"), Some("app.undo"));
        edit_menu.append(Some("Redo"), Some("app.redo"));
        edit_menu.append(None, None);
        edit_menu.append(Some("Cut"), Some("app.cut"));
        edit_menu.append(Some("Copy"), Some("app.copy"));
        edit_menu.append(Some("Paste"), Some("app.paste"));
        edit_menu.append(Some("Delete"), Some("app.delete"));
        edit_menu.append(None, None);
        edit_menu.append(Some("Select All"), Some("app.select_all"));
        edit_menu.append(None, None);
        edit_menu.append(Some("Clear History"), Some("app.clear_history"));
        
        // Create View menu
        let view_menu = Menu::new();
        view_menu.append(Some("Zoom In"), Some("app.zoom_in"));
        view_menu.append(Some("Zoom Out"), Some("app.zoom_out"));
        view_menu.append(Some("Normal Size"), Some("app.normal_size"));
        view_menu.append(None, None);
        view_menu.append(Some("Fullscreen"), Some("app.fullscreen"));
        view_menu.append(Some("Show Sidebar"), Some("app.show_sidebar"));
        view_menu.append(Some("Show Status Bar"), Some("app.show_status_bar"));
        
        // Create Help menu
        let help_menu = Menu::new();
        help_menu.append(Some("Documentation"), Some("app.documentation"));
        help_menu.append(Some("Keyboard Shortcuts"), Some("app.shortcuts"));
        help_menu.append(Some("Report a Bug"), Some("app.report_bug"));
        help_menu.append(None, None);
        help_menu.append(Some("About"), Some("app.about"));
        
        // Add submenus to the main menu
        self.menu_model.append_submenu(Some("File"), &file_menu);
        self.menu_model.append_submenu(Some("Edit"), &edit_menu);
        self.menu_model.append_submenu(Some("View"), &view_menu);
        self.menu_model.append_submenu(Some("Help"), &help_menu);
        
        Ok(())
    }
    
    /// Set up menu actions
    fn setup_actions(&mut self) -> Result<(), AppError> {
        // File menu actions
        self.setup_file_actions()?;
        
        // Edit menu actions
        self.setup_edit_actions()?;
        
        // View menu actions
        self.setup_view_actions()?;
        
        // Help menu actions
        self.setup_help_actions()?;
        
        Ok(())
    }
    
    /// Set up File menu actions
    fn setup_file_actions(&mut self) -> Result<(), AppError> {
        // New action
        let new_action = SimpleAction::new("new", None);
        let app_state = self.app_state.clone();
        new_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Creating new file...".to_string()).await;
            });
        });
        self.action_group.add_action(&new_action);
        
        // Open action
        let open_action = SimpleAction::new("open", None);
        let app_state = self.app_state.clone();
        open_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Opening file...".to_string()).await;
            });
        });
        self.action_group.add_action(&open_action);
        
        // Save action
        let save_action = SimpleAction::new("save", None);
        let app_state = self.app_state.clone();
        save_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Saving file...".to_string()).await;
            });
        });
        self.action_group.add_action(&save_action);
        
        // Save As action
        let save_as_action = SimpleAction::new("save_as", None);
        let app_state = self.app_state.clone();
        save_as_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Saving file as...".to_string()).await;
            });
        });
        self.action_group.add_action(&save_as_action);
        
        // Preferences action
        let preferences_action = SimpleAction::new("preferences", None);
        let app_state = self.app_state.clone();
        preferences_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Opening preferences...".to_string()).await;
            });
        });
        self.action_group.add_action(&preferences_action);
        
        // Exit action
        let exit_action = SimpleAction::new("exit", None);
        let window = self.window.clone();
        exit_action.connect_activate(move |_, _| {
            window.close();
        });
        self.action_group.add_action(&exit_action);
        
        Ok(())
    }
    
    /// Set up Edit menu actions
    fn setup_edit_actions(&mut self) -> Result<(), AppError> {
        // Undo action
        let undo_action = SimpleAction::new("undo", None);
        let app_state = self.app_state.clone();
        undo_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Undo".to_string()).await;
            });
        });
        self.action_group.add_action(&undo_action);
        
        // Redo action
        let redo_action = SimpleAction::new("redo", None);
        let app_state = self.app_state.clone();
        redo_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Redo".to_string()).await;
            });
        });
        self.action_group.add_action(&redo_action);
        
        // Cut action
        let cut_action = SimpleAction::new("cut", None);
        let app_state = self.app_state.clone();
        cut_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Cut".to_string()).await;
            });
        });
        self.action_group.add_action(&cut_action);
        
        // Copy action
        let copy_action = SimpleAction::new("copy", None);
        let app_state = self.app_state.clone();
        copy_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Copy".to_string()).await;
            });
        });
        self.action_group.add_action(&copy_action);
        
        // Paste action
        let paste_action = SimpleAction::new("paste", None);
        let app_state = self.app_state.clone();
        paste_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Paste".to_string()).await;
            });
        });
        self.action_group.add_action(&paste_action);
        
        // Delete action
        let delete_action = SimpleAction::new("delete", None);
        let app_state = self.app_state.clone();
        delete_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Delete".to_string()).await;
            });
        });
        self.action_group.add_action(&delete_action);
        
        // Select All action
        let select_all_action = SimpleAction::new("select_all", None);
        let app_state = self.app_state.clone();
        select_all_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Select all".to_string()).await;
            });
        });
        self.action_group.add_action(&select_all_action);
        
        // Clear History action
        let clear_history_action = SimpleAction::new("clear_history", None);
        let app_state = self.app_state.clone();
        clear_history_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Clearing history...".to_string()).await;
            });
        });
        self.action_group.add_action(&clear_history_action);
        
        Ok(())
    }
    
    /// Set up View menu actions
    fn setup_view_actions(&mut self) -> Result<(), AppError> {
        // Zoom In action
        let zoom_in_action = SimpleAction::new("zoom_in", None);
        let app_state = self.app_state.clone();
        zoom_in_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Zooming in".to_string()).await;
            });
        });
        self.action_group.add_action(&zoom_in_action);
        
        // Zoom Out action
        let zoom_out_action = SimpleAction::new("zoom_out", None);
        let app_state = self.app_state.clone();
        zoom_out_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Zooming out".to_string()).await;
            });
        });
        self.action_group.add_action(&zoom_out_action);
        
        // Normal Size action
        let normal_size_action = SimpleAction::new("normal_size", None);
        let app_state = self.app_state.clone();
        normal_size_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Normal size".to_string()).await;
            });
        });
        self.action_group.add_action(&normal_size_action);
        
        // Fullscreen action
        let fullscreen_action = SimpleAction::new("fullscreen", None);
        let window = self.window.clone();
        fullscreen_action.connect_activate(move |_, _| {
            if window.is_fullscreen() {
                window.unfullscreen();
            } else {
                window.fullscreen();
            }
        });
        self.action_group.add_action(&fullscreen_action);
        
        // Show Sidebar action
        let show_sidebar_action = SimpleAction::new("show_sidebar", None);
        let app_state = self.app_state.clone();
        show_sidebar_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Toggling sidebar".to_string()).await;
            });
        });
        self.action_group.add_action(&show_sidebar_action);
        
        // Show Status Bar action
        let show_status_bar_action = SimpleAction::new("show_status_bar", None);
        let app_state = self.app_state.clone();
        show_status_bar_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Toggling status bar".to_string()).await;
            });
        });
        self.action_group.add_action(&show_status_bar_action);
        
        Ok(())
    }
    
    /// Set up Help menu actions
    fn setup_help_actions(&mut self) -> Result<(), AppError> {
        // Documentation action
        let documentation_action = SimpleAction::new("documentation", None);
        let app_state = self.app_state.clone();
        documentation_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Opening documentation...".to_string()).await;
            });
        });
        self.action_group.add_action(&documentation_action);
        
        // Keyboard Shortcuts action
        let shortcuts_action = SimpleAction::new("shortcuts", None);
        let app_state = self.app_state.clone();
        shortcuts_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Showing keyboard shortcuts...".to_string()).await;
            });
        });
        self.action_group.add_action(&shortcuts_action);
        
        // Report a Bug action
        let report_bug_action = SimpleAction::new("report_bug", None);
        let app_state = self.app_state.clone();
        report_bug_action.connect_activate(move |_, _| {
            let app_state_clone = app_state.clone();
            gtk4::glib::spawn_future_local(async move {
                app_state_clone.set_status_message("Opening bug report...".to_string()).await;
            });
        });
        self.action_group.add_action(&report_bug_action);
        
        // About action
        let about_action = SimpleAction::new("about", None);
        let window = self.window.clone();
        about_action.connect_activate(move |_, _| {
            Self::show_about_dialog(&window);
        });
        self.action_group.add_action(&about_action);
        
        Ok(())
    }
    
    /// Show the about dialog
    fn show_about_dialog(window: &ApplicationWindow) {
        let about_dialog = AboutDialog::builder()
            .transient_for(window)
            .modal(true)
            .program_name("ASRPro")
            .version("1.0.0")
            .copyright("© 2023 ASRPro Team")
            .comments("A speech recognition application with GTK4 frontend")
            .website("https://github.com/asrpro/asrpro")
            .website_label("ASRPro on GitHub")
            .license_type(License::MitX11)
            .authors(vec!["ASRPro Development Team"])
            .artists(vec!["ASRPro Design Team"])
            .logo_icon_name("audio-input-microphone")
            .build();
        
        about_dialog.connect_close_request(move |dialog| {
            gtk4::glib::Propagation::Proceed
        });
        
        about_dialog.show();
    }
    
    /// Get the menu model
    pub fn get_menu_model(&self) -> &Menu {
        &self.menu_model
    }
    
    /// Get the action group
    pub fn get_action_group(&self) -> &SimpleActionGroup {
        &self.action_group
    }
}