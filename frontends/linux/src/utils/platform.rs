//! Platform integration utilities for Linux desktop environment
//! 
//! This module provides functionality for integrating with the Linux desktop,
//! including desktop environment detection, notifications, system tray, and file associations.

use std::process::Command;
use std::path::Path;
use anyhow::{Result, anyhow};
use gio::prelude::*;
use gtk4 as gtk;

/// Desktop environment types
#[derive(Debug, Clone, PartialEq)]
pub enum DesktopEnvironment {
    GNOME,
    KDE,
    XFCE,
    LXDE,
    LXQt,
    MATE,
    Cinnamon,
    Budgie,
    Unknown,
}

/// Platform integration manager
pub struct PlatformIntegration {
    desktop_env: DesktopEnvironment,
    has_system_tray: bool,
    has_notifications: bool,
}

impl PlatformIntegration {
    /// Create a new platform integration instance
    pub fn new() -> Result<Self> {
        let desktop_env = detect_desktop_environment();
        let has_system_tray = check_system_tray_support();
        let has_notifications = check_notification_support();
        
        Ok(Self {
            desktop_env,
            has_system_tray,
            has_notifications,
        })
    }
    
    /// Get the detected desktop environment
    pub fn desktop_environment(&self) -> &DesktopEnvironment {
        &self.desktop_env
    }
    
    /// Check if system tray is supported
    pub fn has_system_tray(&self) -> bool {
        self.has_system_tray
    }
    
    /// Check if notifications are supported
    pub fn has_notifications(&self) -> bool {
        self.has_notifications
    }
    
    /// Send a desktop notification
    pub fn send_notification(&self, title: &str, body: &str) -> Result<()> {
        if !self.has_notifications {
            return Err(anyhow!("Notifications not supported"));
        }
        
        match self.desktop_env {
            DesktopEnvironment::GNOME | DesktopEnvironment::Cinnamon | DesktopEnvironment::Budgie => {
                send_notify_notification(title, body)
            }
            DesktopEnvironment::KDE => {
                send_kde_notification(title, body)
            }
            DesktopEnvironment::XFCE | DesktopEnvironment::LXDE | DesktopEnvironment::LXQt | 
            DesktopEnvironment::MATE => {
                send_generic_notification(title, body)
            }
            DesktopEnvironment::Unknown => {
                // Try generic method
                send_generic_notification(title, body)
            }
        }
    }
    
    /// Register file associations for this application
    pub fn register_file_associations(&self) -> Result<()> {
        // Update desktop database
        let output = Command::new("update-desktop-database")
            .arg("/usr/share/applications")
            .output();
            
        match output {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!("Failed to update desktop database: {}", e))
        }
    }
    
    /// Register MIME types for this application
    pub fn register_mime_types(&self) -> Result<()> {
        // Update MIME database
        let output = Command::new("update-mime-database")
            .arg("/usr/share/mime")
            .output();
            
        match output {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!("Failed to update MIME database: {}", e))
        }
    }
    
    /// Update icon cache
    pub fn update_icon_cache(&self) -> Result<()> {
        let output = Command::new("gtk-update-icon-cache")
            .args(["-f", "-t", "/usr/share/icons/hicolor"])
            .output();
            
        match output {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!("Failed to update icon cache: {}", e))
        }
    }
    
    /// Get the default terminal emulator for this desktop environment
    pub fn get_default_terminal(&self) -> String {
        match self.desktop_env {
            DesktopEnvironment::GNOME | DesktopEnvironment::Cinnamon => "gnome-terminal".to_string(),
            DesktopEnvironment::KDE => "konsole".to_string(),
            DesktopEnvironment::XFCE => "xfce4-terminal".to_string(),
            DesktopEnvironment::LXDE => "lxterminal".to_string(),
            DesktopEnvironment::LXQt => "qterminal".to_string(),
            DesktopEnvironment::MATE => "mate-terminal".to_string(),
            DesktopEnvironment::Budgie => "gnome-terminal".to_string(),
            DesktopEnvironment::Unknown => "xterm".to_string(),
        }
    }
    
    /// Get the default file manager for this desktop environment
    pub fn get_default_file_manager(&self) -> String {
        match self.desktop_env {
            DesktopEnvironment::GNOME | DesktopEnvironment::Cinnamon | DesktopEnvironment::Budgie => "nautilus".to_string(),
            DesktopEnvironment::KDE => "dolphin".to_string(),
            DesktopEnvironment::XFCE => "thunar".to_string(),
            DesktopEnvironment::LXDE => "pcmanfm".to_string(),
            DesktopEnvironment::LXQt => "pcmanfm-qt".to_string(),
            DesktopEnvironment::MATE => "caja".to_string(),
            DesktopEnvironment::Unknown => "xdg-open".to_string(),
        }
    }
    
    /// Open a file or URL with the default application
    pub fn open_with_default(&self, path: &str) -> Result<()> {
        let output = Command::new("xdg-open")
            .arg(path)
            .output();
            
        match output {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(anyhow!("Failed to open {}: {}", path, stderr))
            }
            Err(e) => Err(anyhow!("Failed to execute xdg-open: {}", e))
        }
    }
    
    /// Show an error dialog using the desktop environment's native dialog
    pub fn show_error_dialog(&self, title: &str, message: &str) -> Result<()> {
        match self.desktop_env {
            DesktopEnvironment::GNOME | DesktopEnvironment::Cinnamon | DesktopEnvironment::Budgie => {
                show_gtk_dialog(title, message, gtk::MessageType::Error)
            }
            DesktopEnvironment::KDE => {
                show_kdialog(title, message, "error")
            }
            DesktopEnvironment::XFCE | DesktopEnvironment::LXDE | DesktopEnvironment::LXQt | 
            DesktopEnvironment::MATE => {
                show_zenity_dialog(title, message, "error")
            }
            DesktopEnvironment::Unknown => {
                // Try GTK dialog as fallback
                show_gtk_dialog(title, message, gtk::MessageType::Error)
            }
        }
    }
    
    /// Show an information dialog using the desktop environment's native dialog
    pub fn show_info_dialog(&self, title: &str, message: &str) -> Result<()> {
        match self.desktop_env {
            DesktopEnvironment::GNOME | DesktopEnvironment::Cinnamon | DesktopEnvironment::Budgie => {
                show_gtk_dialog(title, message, gtk::MessageType::Info)
            }
            DesktopEnvironment::KDE => {
                show_kdialog(title, message, "information")
            }
            DesktopEnvironment::XFCE | DesktopEnvironment::LXDE | DesktopEnvironment::LXQt | 
            DesktopEnvironment::MATE => {
                show_zenity_dialog(title, message, "info")
            }
            DesktopEnvironment::Unknown => {
                // Try GTK dialog as fallback
                show_gtk_dialog(title, message, gtk::MessageType::Info)
            }
        }
    }
}

/// Detect the current desktop environment
fn detect_desktop_environment() -> DesktopEnvironment {
    // Check XDG_CURRENT_DESKTOP first (most reliable)
    if let Ok(xdg_current) = std::env::var("XDG_CURRENT_DESKTOP") {
        let desktop = xdg_current.to_lowercase();
        if desktop.contains("gnome") {
            return DesktopEnvironment::GNOME;
        } else if desktop.contains("kde") {
            return DesktopEnvironment::KDE;
        } else if desktop.contains("xfce") {
            return DesktopEnvironment::XFCE;
        } else if desktop.contains("lxde") {
            return DesktopEnvironment::LXDE;
        } else if desktop.contains("lxqt") {
            return DesktopEnvironment::LXQt;
        } else if desktop.contains("mate") {
            return DesktopEnvironment::MATE;
        } else if desktop.contains("cinnamon") {
            return DesktopEnvironment::Cinnamon;
        } else if desktop.contains("budgie") {
            return DesktopEnvironment::Budgie;
        }
    }
    
    // Fall back to checking DESKTOP_SESSION
    if let Ok(desktop_session) = std::env::var("DESKTOP_SESSION") {
        let session = desktop_session.to_lowercase();
        if session.contains("gnome") {
            return DesktopEnvironment::GNOME;
        } else if session.contains("kde") || session.contains("plasma") {
            return DesktopEnvironment::KDE;
        } else if session.contains("xfce") {
            return DesktopEnvironment::XFCE;
        } else if session.contains("lxde") {
            return DesktopEnvironment::LXDE;
        } else if session.contains("lxqt") {
            return DesktopEnvironment::LXQt;
        } else if session.contains("mate") {
            return DesktopEnvironment::MATE;
        } else if session.contains("cinnamon") {
            return DesktopEnvironment::Cinnamon;
        } else if session.contains("budgie") {
            return DesktopEnvironment::Budgie;
        }
    }
    
    // Check for running processes as last resort
    if let Ok(output) = Command::new("ps").args(["-e"]).output() {
        let processes = String::from_utf8_lossy(&output.stdout);
        if processes.contains("gnome-shell") {
            return DesktopEnvironment::GNOME;
        } else if processes.contains("plasmashell") {
            return DesktopEnvironment::KDE;
        } else if processes.contains("xfce4-session") {
            return DesktopEnvironment::XFCE;
        } else if processes.contains("lxsession") {
            return DesktopEnvironment::LXDE;
        } else if processes.contains("lxqt-session") {
            return DesktopEnvironment::LXQt;
        } else if processes.contains("mate-session") {
            return DesktopEnvironment::MATE;
        } else if processes.contains("cinnamon") {
            return DesktopEnvironment::Cinnamon;
        } else if processes.contains("budgie") {
            return DesktopEnvironment::Budgie;
        }
    }
    
    DesktopEnvironment::Unknown
}

/// Check if system tray is supported
fn check_system_tray_support() -> bool {
    // Check for StatusNotifierItem support (modern)
    if let Ok(output) = Command::new("dbus-send")
        .args(["--session", "--dest=org.freedesktop.DBus", "--type=method_call", 
               "--print-reply", "/org/freedesktop/DBus", 
               "org.freedesktop.DBus.ListNames"])
        .output() 
    {
        let services = String::from_utf8_lossy(&output.stdout);
        if services.contains("org.kde.StatusNotifierWatcher") {
            return true;
        }
    }
    
    // Check for legacy system tray support
    if let Ok(output) = Command::new("xprop")
        .args(["-root", "_NET_SYSTEM_TRAY_S0"])
        .output() 
    {
        return output.status.success();
    }
    
    false
}

/// Check if notifications are supported
fn check_notification_support() -> bool {
    // Check for notification daemon via D-Bus
    if let Ok(output) = Command::new("dbus-send")
        .args(["--session", "--dest=org.freedesktop.DBus", "--type=method_call", 
               "--print-reply", "/org/freedesktop/DBus", 
               "org.freedesktop.DBus.ListNames"])
        .output() 
    {
        let services = String::from_utf8_lossy(&output.stdout);
        return services.contains("org.freedesktop.Notifications") ||
               services.contains("org.kde.NotificationDaemon");
    }
    
    false
}

/// Send notification using notify-send
fn send_notify_notification(title: &str, body: &str) -> Result<()> {
    let output = Command::new("notify-send")
        .args([title, body])
        .output();
        
    match output {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("Failed to send notification: {}", stderr))
        }
        Err(e) => Err(anyhow!("Failed to execute notify-send: {}", e))
    }
}

/// Send notification using KDE's kdialog
fn send_kde_notification(title: &str, body: &str) -> Result<()> {
    let output = Command::new("kdialog")
        .args(["--title", title, "--passivepopup", body, "5"])
        .output();
        
    match output {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("Failed to send KDE notification: {}", stderr))
        }
        Err(e) => Err(anyhow!("Failed to execute kdialog: {}", e))
    }
}

/// Send notification using zenity
fn send_generic_notification(title: &str, body: &str) -> Result<()> {
    let output = Command::new("zenity")
        .args(["--info", "--title", title, "--text", body, "--timeout=5"])
        .output();
        
    match output {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("Failed to send notification: {}", stderr))
        }
        Err(e) => Err(anyhow!("Failed to execute zenity: {}", e))
    }
}

/// Show a GTK dialog
fn show_gtk_dialog(title: &str, message: &str, msg_type: gtk::MessageType) -> Result<()> {
    let dialog = gtk::MessageDialog::builder()
        .title(title)
        .text(message)
        .message_type(msg_type)
        .buttons(gtk::ButtonsType::Ok)
        .modal(true)
        .build();
        
    dialog.connect_response(|dialog, _| {
        dialog.close();
    });
    
    dialog.show();
    Ok(())
}

/// Show a KDE dialog using kdialog
fn show_kdialog(title: &str, message: &str, dialog_type: &str) -> Result<()> {
    let output = Command::new("kdialog")
        .args(["--title", title, &format!("--{}", dialog_type), message])
        .output();
        
    match output {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("Failed to show KDE dialog: {}", stderr))
        }
        Err(e) => Err(anyhow!("Failed to execute kdialog: {}", e))
    }
}

/// Show a dialog using zenity
fn show_zenity_dialog(title: &str, message: &str, dialog_type: &str) -> Result<()> {
    let output = Command::new("zenity")
        .args(["--title", title, &format!("--{}", dialog_type), "--text", message])
        .output();
        
    match output {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("Failed to show zenity dialog: {}", stderr))
        }
        Err(e) => Err(anyhow!("Failed to execute zenity: {}", e))
    }
}

/// Get supported audio MIME types
pub fn get_supported_audio_types() -> Vec<&'static str> {
    vec![
        "audio/mpeg",
        "audio/mp4",
        "audio/ogg",
        "audio/wav",
        "audio/flac",
        "audio/x-wav",
        "audio/x-flac",
        "audio/x-m4a",
        "audio/aac",
        "audio/x-aac",
    ]
}

/// Check if a MIME type is supported
pub fn is_mime_type_supported(mime_type: &str) -> bool {
    get_supported_audio_types().contains(&mime_type)
}

/// Get file extensions for supported audio types
pub fn get_supported_extensions() -> Vec<&'static str> {
    vec![
        "mp3", "mp4", "ogg", "wav", "flac", "m4a", "aac"
    ]
}

/// Check if a file extension is supported
pub fn is_extension_supported(extension: &str) -> bool {
    get_supported_extensions().contains(&extension.to_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_desktop_environment_detection() {
        let env = detect_desktop_environment();
        // Should not panic and should return a valid enum
        match env {
            DesktopEnvironment::Unknown => {}, // OK for CI environments
            _ => {}, // Any specific environment is OK
        }
    }
    
    #[test]
    fn test_supported_mime_types() {
        let types = get_supported_audio_types();
        assert!(!types.is_empty());
        assert!(types.contains(&"audio/mpeg"));
        assert!(types.contains(&"audio/wav"));
    }
    
    #[test]
    fn test_supported_extensions() {
        let extensions = get_supported_extensions();
        assert!(!extensions.is_empty());
        assert!(extensions.contains(&"mp3"));
        assert!(extensions.contains(&"wav"));
    }
    
    #[test]
    fn test_mime_type_support() {
        assert!(is_mime_type_supported("audio/mpeg"));
        assert!(is_mime_type_supported("audio/wav"));
        assert!(!is_mime_type_supported("video/mp4"));
    }
    
    #[test]
    fn test_extension_support() {
        assert!(is_extension_supported("mp3"));
        assert!(is_extension_supported("wav"));
        assert!(!is_extension_supported("avi"));
    }
}