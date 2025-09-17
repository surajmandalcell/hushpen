"""General settings page."""

from ..layouts.setting_row import SettingRow
from .base_page import BasePage


class GeneralPage(BasePage):
    """General application settings."""
    
    def __init__(self, parent=None):
        super().__init__("General", parent)
        self._create_content()
    
    def _create_content(self):
        """Create minimal general settings content."""
        # Launch at Login setting
        launch_setting = SettingRow(
            title="Launch at Login",
            description="Start ASR Pro when you log in",
            setting_name="launch_at_login",
            control_type="toggle",
            current_value=False
        )
        launch_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(launch_setting)
        
        # Start Minimized setting
        minimized_setting = SettingRow(
            title="Start Minimized",
            description="Start in system tray",
            setting_name="start_minimized", 
            control_type="toggle",
            current_value=True
        )
        minimized_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(minimized_setting)
        
        # Model auto-unload setting
        autounload_setting = SettingRow(
            title="Auto-unload Model",
            description="Unload Whisper model after 30 minutes of inactivity",
            setting_name="model_auto_unload",
            control_type="toggle",
            current_value=True
        )
        autounload_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(autounload_setting)
        
        # Add stretch to push content to top
        self.add_stretch()
