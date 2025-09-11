"""General settings page."""

from ..layouts.setting_row import SettingRow
from .base_page import BasePage


class GeneralPage(BasePage):
    """General application settings."""
    
    def __init__(self, parent=None):
        super().__init__("General", parent)
        self._create_content()
    
    def _create_content(self):
        """Create general settings content."""
        # Launch at Login setting
        launch_setting = SettingRow(
            title="Launch at Login",
            description="Automatically start ASR Pro when you log in to your computer",
            setting_name="launch_at_login",
            control_type="toggle",
            current_value=False
        )
        launch_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(launch_setting)
        
        # Start Minimized setting
        minimized_setting = SettingRow(
            title="Start Minimized",
            description="Launch the application minimized to the system tray",
            setting_name="start_minimized", 
            control_type="toggle",
            current_value=True
        )
        minimized_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(minimized_setting)
        
        # Theme setting
        theme_setting = SettingRow(
            title="Theme",
            description="Choose the application theme",
            setting_name="theme",
            control_type="dropdown",
            options=[
                {"label": "Dark", "value": "dark"},
                {"label": "Light", "value": "light"},
                {"label": "System", "value": "system"}
            ],
            current_value="dark"
        )
        theme_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(theme_setting)
        
        # Language setting
        language_setting = SettingRow(
            title="Language",
            description="Select the application language",
            setting_name="language",
            control_type="dropdown",
            options=[
                {"label": "English", "value": "en"},
                {"label": "Spanish", "value": "es"},
                {"label": "French", "value": "fr"},
                {"label": "German", "value": "de"}
            ],
            current_value="en"
        )
        language_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(language_setting)
        
        # Notifications setting
        notifications_setting = SettingRow(
            title="Show Notifications",
            description="Display desktop notifications for important events",
            setting_name="show_notifications",
            control_type="toggle",
            current_value=True
        )
        notifications_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(notifications_setting)
        
        # Sound Feedback setting
        sound_setting = SettingRow(
            title="Sound Feedback",
            description="Play sounds when recording starts and stops",
            setting_name="sound_feedback",
            control_type="toggle", 
            current_value=True
        )
        sound_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(sound_setting)
        
        # Auto-save Location setting
        autosave_setting = SettingRow(
            title="Auto-save Location",
            description="Choose where transcribed text should be automatically saved",
            setting_name="autosave_location",
            control_type="folder",
            current_value=""
        )
        autosave_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(autosave_setting)
        
        # Add stretch to push content to top
        self.add_stretch()