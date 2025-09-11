"""Microphone and audio settings page."""

from PySide6.QtCore import Qt, Signal
from PySide6.QtGui import QFont, QPainter
from PySide6.QtWidgets import QWidget, QVBoxLayout, QHBoxLayout, QLabel, QPushButton, QProgressBar

from ..components.setting_row import SettingRow
from ..styles.dark_theme import DarkTheme, Dimensions, Fonts, Spacing
from .base_page import BasePage


class MicrophoneItem(QWidget):
    """Widget representing a single microphone device."""
    
    selected = Signal(str)  # device_id
    
    def __init__(self, device_id: str, name: str, is_default: bool = False, 
                 is_selected: bool = False, parent=None):
        super().__init__(parent)
        
        self.device_id = device_id
        self.name = name
        self.is_default = is_default
        self.is_selected = is_selected
        
        self._setup_ui()
        self._apply_styles()
    
    def _setup_ui(self):
        """Set up the microphone item layout."""
        layout = QHBoxLayout(self)
        layout.setContentsMargins(Spacing.CARD_PADDING, Spacing.CARD_PADDING,
                                 Spacing.CARD_PADDING, Spacing.CARD_PADDING)
        layout.setSpacing(12)
        
        # Device info section
        info_layout = QVBoxLayout()
        info_layout.setSpacing(4)
        
        # Device name
        self.name_label = QLabel(self.name)
        font = QFont()
        font.setPointSize(Fonts.SETTING_LABEL_SIZE)
        font.setWeight(Fonts.MEDIUM)
        self.name_label.setFont(font)
        info_layout.addWidget(self.name_label)
        
        # Status labels
        status_layout = QHBoxLayout()
        status_layout.setSpacing(12)
        
        if self.is_default:
            self.default_label = QLabel("System Default")
            default_font = QFont()
            default_font.setPointSize(Fonts.DESCRIPTION_SIZE)
            self.default_label.setFont(default_font)
            status_layout.addWidget(self.default_label)
        
        if self.is_selected:
            self.selected_label = QLabel("Currently Selected")
            selected_font = QFont()
            selected_font.setPointSize(Fonts.DESCRIPTION_SIZE)
            self.selected_label.setFont(selected_font)
            status_layout.addWidget(self.selected_label)
        
        status_layout.addStretch()
        info_layout.addLayout(status_layout)
        
        layout.addLayout(info_layout, 1)
        
        # Action buttons
        buttons_layout = QVBoxLayout()
        buttons_layout.setSpacing(8)
        
        # Select button
        self.select_button = QPushButton("Select")
        if self.is_selected:
            self.select_button.setText("Selected")
            self.select_button.setEnabled(False)
        else:
            self.select_button.clicked.connect(lambda: self.selected.emit(self.device_id))
        
        buttons_layout.addWidget(self.select_button)
        
        # Test button
        self.test_button = QPushButton("Test")
        buttons_layout.addWidget(self.test_button)
        
        layout.addLayout(buttons_layout)
    
    def _apply_styles(self):
        """Apply microphone item styling."""
        # Main container
        self.setStyleSheet(f"""
            MicrophoneItem {{
                background-color: {DarkTheme.CARD_BG.name()};
                border: 1px solid {DarkTheme.CARD_BORDER.name()};
                border-radius: {Dimensions.CARD_RADIUS}px;
                margin-bottom: 8px;
            }}
        """)
        
        # Text colors
        self.name_label.setStyleSheet(f"color: {DarkTheme.PRIMARY_TEXT.name()};")
        
        if hasattr(self, 'default_label'):
            self.default_label.setStyleSheet(f"color: {DarkTheme.WARNING_YELLOW.name()};")
        
        if hasattr(self, 'selected_label'):
            self.selected_label.setStyleSheet(f"color: {DarkTheme.SUCCESS_GREEN.name()};")
        
        # Button styling
        button_style = f"""
            QPushButton {{
                background-color: {DarkTheme.BUTTON_BG.name()};
                color: {DarkTheme.PRIMARY_TEXT.name()};
                border: none;
                padding: 6px 12px;
                border-radius: 4px;
                font-size: {Fonts.CONTROL_SIZE}px;
                min-width: 60px;
            }}
            QPushButton:hover {{
                background-color: {DarkTheme.BUTTON_HOVER_BG.name()};
            }}
            QPushButton:disabled {{
                background-color: {DarkTheme.SUCCESS_GREEN.name()};
                color: {DarkTheme.PRIMARY_TEXT.name()};
            }}
        """
        
        self.select_button.setStyleSheet(button_style)
        self.test_button.setStyleSheet(button_style)
    
    def set_selected(self, selected: bool):
        """Update the selected state."""
        self.is_selected = selected
        if selected:
            self.select_button.setText("Selected")
            self.select_button.setEnabled(False)
            if not hasattr(self, 'selected_label'):
                self.selected_label = QLabel("Currently Selected")
                selected_font = QFont()
                selected_font.setPointSize(Fonts.DESCRIPTION_SIZE)
                self.selected_label.setFont(selected_font)
                self.selected_label.setStyleSheet(f"color: {DarkTheme.SUCCESS_GREEN.name()};")
        else:
            self.select_button.setText("Select")
            self.select_button.setEnabled(True)
            if hasattr(self, 'selected_label'):
                self.selected_label.deleteLater()
                delattr(self, 'selected_label')


class AudioLevelMeter(QWidget):
    """Visual audio level meter widget."""
    
    def __init__(self, parent=None):
        super().__init__(parent)
        
        self.level = 0.0  # 0.0 to 1.0
        self.setFixedHeight(20)
        self.setMinimumWidth(200)
    
    def set_level(self, level: float):
        """Set the audio level (0.0 to 1.0)."""
        self.level = max(0.0, min(1.0, level))
        self.update()
    
    def paintEvent(self, event):
        """Custom paint event for audio level visualization."""
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        
        rect = self.rect()
        
        # Background
        painter.fillRect(rect, DarkTheme.CONTROL_BG)
        
        # Level bar
        if self.level > 0:
            level_width = int(rect.width() * self.level)
            level_rect = rect.adjusted(0, 0, level_width - rect.width(), 0)
            
            # Color based on level
            if self.level < 0.5:
                color = DarkTheme.SUCCESS_GREEN
            elif self.level < 0.8:
                color = DarkTheme.WARNING_YELLOW
            else:
                color = DarkTheme.ERROR_RED
            
            painter.fillRect(level_rect, color)


class MicrophonePage(BasePage):
    """Microphone and audio input settings."""
    
    def __init__(self, parent=None):
        super().__init__("Microphone", parent)
        self._create_content()
    
    def _create_content(self):
        """Create microphone settings content."""
        # Input Device section
        device_label = QLabel("Input Device")
        font = QFont()
        font.setPointSize(Fonts.SETTING_LABEL_SIZE)
        font.setWeight(Fonts.MEDIUM)
        device_label.setFont(font)
        device_label.setStyleSheet(f"color: {DarkTheme.PRIMARY_TEXT.name()}; margin-bottom: 8px;")
        self.add_content_widget(device_label)
        
        # Sample microphone devices
        devices_data = [
            ("mic_1", "Built-in Microphone", True, False),
            ("mic_2", "USB Audio Device", False, True),
            ("mic_3", "Bluetooth Headset", False, False),
        ]
        
        for device_id, name, is_default, is_selected in devices_data:
            mic_item = MicrophoneItem(device_id, name, is_default, is_selected)
            mic_item.selected.connect(self._on_microphone_selected)
            self.add_content_widget(mic_item)
        
        # Audio Level section
        level_label = QLabel("Audio Level Monitor")
        font = QFont()
        font.setPointSize(Fonts.SETTING_LABEL_SIZE)
        font.setWeight(Fonts.MEDIUM)
        level_label.setFont(font)
        level_label.setStyleSheet(f"color: {DarkTheme.PRIMARY_TEXT.name()}; margin-top: 24px; margin-bottom: 8px;")
        self.add_content_widget(level_label)
        
        # Audio level meter
        level_container = QWidget()
        level_layout = QHBoxLayout(level_container)
        level_layout.setContentsMargins(0, 8, 0, 8)
        
        level_layout.addWidget(QLabel("Level:"))
        self.audio_meter = AudioLevelMeter()
        self.audio_meter.set_level(0.3)  # Demo level
        level_layout.addWidget(self.audio_meter)
        level_layout.addStretch()
        
        self.add_content_widget(level_container)
        
        # Audio Settings section
        settings_label = QLabel("Audio Settings")
        font = QFont()
        font.setPointSize(Fonts.SETTING_LABEL_SIZE)
        font.setWeight(Fonts.MEDIUM)
        settings_label.setFont(font)
        settings_label.setStyleSheet(f"color: {DarkTheme.PRIMARY_TEXT.name()}; margin-top: 24px; margin-bottom: 8px;")
        self.add_content_widget(settings_label)
        
        # Sample Rate setting
        sample_rate_setting = SettingRow(
            title="Sample Rate",
            description="Audio recording sample rate (higher = better quality, larger files)",
            setting_name="sample_rate",
            control_type="dropdown",
            options=[
                {"label": "16000 Hz (Recommended)", "value": 16000},
                {"label": "22050 Hz", "value": 22050},
                {"label": "44100 Hz", "value": 44100},
                {"label": "48000 Hz", "value": 48000}
            ],
            current_value=16000
        )
        sample_rate_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(sample_rate_setting)
        
        # Noise Suppression setting
        noise_setting = SettingRow(
            title="Noise Suppression",
            description="Reduce background noise during recording",
            setting_name="noise_suppression",
            control_type="toggle",
            current_value=True
        )
        noise_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(noise_setting)
        
        # Automatic Gain Control setting
        agc_setting = SettingRow(
            title="Automatic Gain Control",
            description="Automatically adjust microphone sensitivity",
            setting_name="automatic_gain_control",
            control_type="toggle",
            current_value=True
        )
        agc_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(agc_setting)
        
        # Voice Activation setting
        vad_setting = SettingRow(
            title="Voice Activation Detection",
            description="Only record when speech is detected",
            setting_name="voice_activation",
            control_type="toggle",
            current_value=False
        )
        vad_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(vad_setting)
        
        # Sensitivity setting
        sensitivity_setting = SettingRow(
            title="Recording Sensitivity",
            description="How sensitive the microphone is to audio input",
            setting_name="recording_sensitivity",
            control_type="dropdown",
            options=[
                {"label": "Low", "value": "low"},
                {"label": "Medium", "value": "medium"},
                {"label": "High", "value": "high"}
            ],
            current_value="medium"
        )
        sensitivity_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(sensitivity_setting)
        
        # Buffer Size setting
        buffer_setting = SettingRow(
            title="Audio Buffer Size",
            description="Size of audio buffer (smaller = lower latency, higher CPU usage)",
            setting_name="audio_buffer_size",
            control_type="dropdown",
            options=[
                {"label": "256 samples", "value": 256},
                {"label": "512 samples (Recommended)", "value": 512},
                {"label": "1024 samples", "value": 1024},
                {"label": "2048 samples", "value": 2048}
            ],
            current_value=512
        )
        buffer_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(buffer_setting)
        
        # Add stretch to push content to top
        self.add_stretch()
    
    def _on_microphone_selected(self, device_id: str):
        """Handle microphone selection."""
        print(f"[MicrophonePage] Selected microphone: {device_id}")
        # Update UI to reflect selection
        # In real implementation, this would update the backend configuration