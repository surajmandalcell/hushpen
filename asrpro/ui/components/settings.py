"""
Settings Component - Enterprise Settings Management

Handles all application settings with:
- Persistent storage
- Validation
- Performance optimization
- Real-time synchronization with UI
"""

import json
import os
from pathlib import Path
from typing import Dict, Any, Optional
import logging
from .base import BaseComponent

logger = logging.getLogger(__name__)


class SettingsManager(BaseComponent):
    """
    Enterprise-grade settings management component.
    
    Features:
    - Automatic settings persistence
    - Schema validation
    - Performance-optimized loading
    - UI synchronization
    """
    
    DEFAULT_SETTINGS = {
        # General Settings
        "output_format": "srt",
        "auto_save_location": "~/Documents/ASR Pro",
        "auto_save_enabled": True,
        
        # Dictation Engine
        "ai_model": "whisper",
        "processing_quality": "balanced",
        "language": "en",
        
        # Keyboard Controls  
        "recording_mode": "push_to_talk",
        "activation_keys": ["alt+`"],
        "global_hotkeys_enabled": True,
        
        # Performance
        "gpu_acceleration": True,
        "batch_processing": False,
        "max_concurrent_tasks": 2,
        
        # UI Preferences
        "theme": "dark",
        "window_position": None,
        "sidebar_width": 240,
    }
    
    SETTINGS_SCHEMA = {
        "output_format": str,
        "auto_save_location": str,
        "auto_save_enabled": bool,
        "ai_model": str,
        "processing_quality": str,
        "language": str,
        "recording_mode": str,
        "activation_keys": list,
        "global_hotkeys_enabled": bool,
        "gpu_acceleration": bool,
        "batch_processing": bool,
        "max_concurrent_tasks": int,
        "theme": str,
        "sidebar_width": int,
    }
    
    def __init__(self, main_window=None):
        super().__init__("settings", main_window)
        
        # Settings file path
        self.settings_dir = Path.home() / ".asrpro"
        self.settings_file = self.settings_dir / "settings.json"
        
        # Current settings
        self.settings = self.DEFAULT_SETTINGS.copy()
        
        # Performance optimization
        self._settings_cache = {}
        self._dirty_keys = set()
        
    def initialize(self) -> bool:
        """Initialize the settings manager."""
        try:
            self.start_performance_timer("settings_init")
            
            # Create settings directory
            self.settings_dir.mkdir(exist_ok=True)
            
            # Load existing settings
            self._load_settings()
            
            # Validate settings
            self._validate_and_fix_settings()
            
            self.is_initialized = True
            duration = self.end_performance_timer("settings_init")
            
            logger.info(f"[Settings] Initialized in {duration:.3f}s")
            return True
            
        except Exception as e:
            logger.error(f"[Settings] Failed to initialize: {e}")
            return False
    
    def cleanup(self) -> None:
        """Cleanup and save settings."""
        try:
            self._save_settings()
            logger.info("[Settings] Cleanup completed")
        except Exception as e:
            logger.error(f"[Settings] Cleanup failed: {e}")
    
    def _load_settings(self) -> None:
        """Load settings from disk with performance optimization."""
        try:
            if not self.settings_file.exists():
                logger.info("[Settings] No settings file found, using defaults")
                return
                
            with open(self.settings_file, 'r') as f:
                stored_settings = json.load(f)
                
            # Merge with defaults (handles new settings)
            for key, default_value in self.DEFAULT_SETTINGS.items():
                if key in stored_settings:
                    self.settings[key] = stored_settings[key]
                    
            # Update cache
            self._settings_cache = self.settings.copy()
            
            logger.info(f"[Settings] Loaded {len(stored_settings)} settings")
            
        except Exception as e:
            logger.error(f"[Settings] Failed to load settings: {e}")
            # Use defaults on error
    
    def _save_settings(self) -> None:
        """Save settings to disk with optimized I/O."""
        try:
            if not self._dirty_keys:
                return  # No changes to save
                
            # Atomic write for data integrity
            temp_file = self.settings_file.with_suffix('.tmp')
            
            with open(temp_file, 'w') as f:
                json.dump(self.settings, f, indent=2)
                
            # Atomic move
            temp_file.replace(self.settings_file)
            
            # Update cache
            self._settings_cache = self.settings.copy()
            self._dirty_keys.clear()
            
            logger.debug(f"[Settings] Saved to {self.settings_file}")
            
        except Exception as e:
            logger.error(f"[Settings] Failed to save settings: {e}")
    
    def _validate_and_fix_settings(self) -> None:
        """Validate settings and fix invalid values."""
        try:
            fixed_count = 0
            
            for key, value in self.settings.items():
                if key in self.SETTINGS_SCHEMA:
                    expected_type = self.SETTINGS_SCHEMA[key]
                    
                    if not isinstance(value, expected_type):
                        # Fix invalid type
                        default_value = self.DEFAULT_SETTINGS.get(key)
                        if default_value is not None:
                            self.settings[key] = default_value
                            self._dirty_keys.add(key)
                            fixed_count += 1
                            logger.warning(f"[Settings] Fixed invalid setting: {key}")
            
            if fixed_count > 0:
                logger.info(f"[Settings] Fixed {fixed_count} invalid settings")
                
        except Exception as e:
            logger.error(f"[Settings] Validation failed: {e}")
    
    def get_setting(self, key: str, default=None) -> Any:
        """Get a setting value with performance optimization."""
        return self.settings.get(key, default)
    
    def set_setting(self, key: str, value: Any) -> bool:
        """Set a setting value with validation."""
        try:
            # Validate type
            if key in self.SETTINGS_SCHEMA:
                expected_type = self.SETTINGS_SCHEMA[key]
                if not isinstance(value, expected_type):
                    logger.warning(f"[Settings] Invalid type for {key}: expected {expected_type.__name__}")
                    return False
            
            # Check if value actually changed
            if self.settings.get(key) == value:
                return True  # No change needed
                
            # Update setting
            old_value = self.settings.get(key)
            self.settings[key] = value
            self._dirty_keys.add(key)
            
            # Auto-save for critical settings
            if key in ['activation_keys', 'recording_mode']:
                self._save_settings()
            
            # Emit change event
            self.emit_to_frontend('setting_changed', {
                'key': key,
                'value': value,
                'old_value': old_value
            })
            
            logger.debug(f"[Settings] Updated {key}: {old_value} -> {value}")
            return True
            
        except Exception as e:
            logger.error(f"[Settings] Failed to set {key}: {e}")
            return False
    
    def get_all_settings(self) -> Dict[str, Any]:
        """Get all settings as a dictionary."""
        return self.settings.copy()
    
    def update_settings(self, settings_dict: Dict[str, Any]) -> int:
        """Update multiple settings at once. Returns number of settings updated."""
        updated_count = 0
        
        try:
            for key, value in settings_dict.items():
                if self.set_setting(key, value):
                    updated_count += 1
                    
            # Batch save
            if updated_count > 0:
                self._save_settings()
                
            logger.info(f"[Settings] Batch updated {updated_count} settings")
            return updated_count
            
        except Exception as e:
            logger.error(f"[Settings] Batch update failed: {e}")
            return 0
    
    def reset_to_defaults(self) -> bool:
        """Reset all settings to defaults."""
        try:
            self.settings = self.DEFAULT_SETTINGS.copy()
            self._dirty_keys = set(self.settings.keys())
            self._save_settings()
            
            self.emit_to_frontend('settings_reset', {})
            
            logger.info("[Settings] Reset to defaults")
            return True
            
        except Exception as e:
            logger.error(f"[Settings] Failed to reset: {e}")
            return False
    
    def export_settings(self, filepath: str) -> bool:
        """Export settings to a file."""
        try:
            with open(filepath, 'w') as f:
                json.dump(self.settings, f, indent=2)
                
            logger.info(f"[Settings] Exported to {filepath}")
            return True
            
        except Exception as e:
            logger.error(f"[Settings] Export failed: {e}")
            return False
    
    def import_settings(self, filepath: str) -> bool:
        """Import settings from a file."""
        try:
            with open(filepath, 'r') as f:
                imported_settings = json.load(f)
                
            # Validate imported settings
            if not self.validate_data(imported_settings, self.SETTINGS_SCHEMA):
                logger.warning("[Settings] Invalid settings file")
                return False
                
            updated_count = self.update_settings(imported_settings)
            
            logger.info(f"[Settings] Imported {updated_count} settings from {filepath}")
            return updated_count > 0
            
        except Exception as e:
            logger.error(f"[Settings] Import failed: {e}")
            return False
    
    # Bridge Signal Handlers
    
    def _handle_save_settings(self, data: str) -> bool:
        """Handle save settings signal from JavaScript."""
        try:
            settings_data = json.loads(data) if data else {}
            updated_count = self.update_settings(settings_data)
            
            logger.info(f"[Settings] Saved {updated_count} settings from UI")
            return True
            
        except Exception as e:
            logger.error(f"[Settings] Failed to save from UI: {e}")
            return False
    
    def _handle_load_settings(self) -> bool:
        """Handle load settings signal from JavaScript."""
        try:
            # Send current settings to UI
            self.emit_to_frontend('settings_loaded', self.get_all_settings())
            
            logger.debug("[Settings] Sent settings to UI")
            return True
            
        except Exception as e:
            logger.error(f"[Settings] Failed to load to UI: {e}")
            return False