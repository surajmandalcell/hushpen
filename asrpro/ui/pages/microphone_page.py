"""Microphone and audio settings page."""

from PySide6.QtCore import Qt, Signal, QTimer
from PySide6.QtGui import QFont, QPainter
from PySide6.QtWidgets import QWidget, QVBoxLayout, QHBoxLayout, QLabel, QPushButton, QProgressBar, QMessageBox

from ..layouts.setting_row import SettingRow
from ..styles.dark_theme import DarkTheme, Dimensions, Fonts, Spacing
from .base_page import BasePage
from ...audio_recorder import AudioRecorder
from ...config import config


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
        font.setPointSize(Fonts.scaled(Fonts.SETTING_LABEL_SIZE))
        font.setWeight(Fonts.adjust_weight(Fonts.MEDIUM))
        self.name_label.setFont(font)
        info_layout.addWidget(self.name_label)
        
        # Status labels
        status_layout = QHBoxLayout()
        status_layout.setSpacing(12)
        
        if self.is_default:
            self.default_label = QLabel("System Default")
            default_font = QFont()
            default_font.setPointSize(Fonts.scaled(Fonts.DESCRIPTION_SIZE))
            self.default_label.setFont(default_font)
            status_layout.addWidget(self.default_label)
        
        if self.is_selected:
            self.selected_label = QLabel("Currently Selected")
            selected_font = QFont()
            selected_font.setPointSize(Fonts.scaled(Fonts.DESCRIPTION_SIZE))
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
                selected_font.setPointSize(Fonts.scaled(Fonts.DESCRIPTION_SIZE))
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
        self.mic_items = {}  # Track microphone item widgets
        self.selected_device_index = None
        self.test_recorder = None
        self.level_timer = None
        self._create_content()
        self._refresh_devices()
    
    def _create_content(self):
        """Create microphone settings content."""
        # Input Device section
        device_section = QWidget()
        device_layout = QHBoxLayout(device_section)
        device_layout.setContentsMargins(0, 0, 0, 0)
        
        device_label = QLabel("Input Device")
        font = QFont()
        font.setPointSize(Fonts.scaled(Fonts.SETTING_LABEL_SIZE))
        font.setWeight(Fonts.adjust_weight(Fonts.MEDIUM))
        device_label.setFont(font)
        device_label.setStyleSheet(f"color: {DarkTheme.PRIMARY_TEXT.name()};")
        device_layout.addWidget(device_label)
        
        device_layout.addStretch()
        
        # Refresh button
        self.refresh_btn = QPushButton("Refresh Devices")
        self.refresh_btn.clicked.connect(self._refresh_devices)
        self.refresh_btn.setStyleSheet(f"""
            QPushButton {{
                background-color: {DarkTheme.BUTTON_BG.name()};
                color: {DarkTheme.PRIMARY_TEXT.name()};
                border: none;
                padding: 6px 12px;
                border-radius: 4px;
                font-size: {Fonts.CONTROL_SIZE}px;
            }}
            QPushButton:hover {{
                background-color: {DarkTheme.BUTTON_HOVER_BG.name()};
            }}
        """)
        device_layout.addWidget(self.refresh_btn)
        
        self.add_content_widget(device_section)
        
        # Container for device items
        self.devices_container = QWidget()
        self.devices_layout = QVBoxLayout(self.devices_container)
        self.devices_layout.setContentsMargins(0, 0, 0, 0)
        self.devices_layout.setSpacing(8)
        self.add_content_widget(self.devices_container)
        
        # Audio Level section
        level_label = QLabel("Audio Level Monitor")
        font = QFont()
        font.setPointSize(Fonts.scaled(Fonts.SETTING_LABEL_SIZE))
        font.setWeight(Fonts.adjust_weight(Fonts.MEDIUM))
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
        font.setPointSize(Fonts.scaled(Fonts.SETTING_LABEL_SIZE))
        font.setWeight(Fonts.adjust_weight(Fonts.MEDIUM))
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
        try:
            device_index = int(device_id)
            print(f"[MicrophonePage] Selected microphone index: {device_index}")
            
            # Update selected state in UI
            for dev_id, item in self.mic_items.items():
                item.set_selected(dev_id == device_id)
            
            # Save to config
            self.selected_device_index = device_index
            config.set('audio.input_device', device_index)
            
        except (ValueError, TypeError) as e:
            print(f"[MicrophonePage] Invalid device ID: {device_id}, error: {e}")
    
    def _refresh_devices(self):
        """Refresh the list of audio devices."""
        print("[MicrophonePage] Refreshing audio devices...")
        
        # Clear existing items
        for item in self.mic_items.values():
            item.deleteLater()
        self.mic_items.clear()
        
        # Clear layout
        while self.devices_layout.count():
            child = self.devices_layout.takeAt(0)
            if child.widget():
                child.widget().deleteLater()
        
        # Get actual audio devices
        devices = AudioRecorder.list_devices()
        
        if not devices:
            no_devices_label = QLabel("No audio input devices found.\nPlease check microphone permissions.")
            no_devices_label.setStyleSheet(f"color: {DarkTheme.WARNING_YELLOW.name()}; padding: 20px;")
            self.devices_layout.addWidget(no_devices_label)
            return
        
        # Get saved device from config
        saved_device = config.get('audio.input_device', None)
        
        # Add device items
        for device in devices:
            device_id = str(device['index'])
            is_selected = (saved_device == device['index']) if saved_device is not None else device['is_default']
            
            mic_item = MicrophoneItem(
                device_id=device_id,
                name=device['name'],
                is_default=device['is_default'],
                is_selected=is_selected
            )
            
            mic_item.selected.connect(self._on_microphone_selected)
            mic_item.test_button.clicked.connect(lambda checked, idx=device['index']: self._test_device(idx))
            
            self.devices_layout.addWidget(mic_item)
            self.mic_items[device_id] = mic_item
            
            if is_selected:
                self.selected_device_index = device['index']
        
        print(f"[MicrophonePage] Found {len(devices)} audio input devices")
    
    def _test_device(self, device_index: int):
        """Test an audio device."""
        print(f"[MicrophonePage] Testing device {device_index}")
        
        try:
            # Simple test: try to create a recorder with the device
            test_recorder = AudioRecorder(device=device_index)
            
            # Show success message
            QMessageBox.information(
                self,
                "Device Test",
                "Audio device is working!\n\nIf you didn't hear anything, make sure:\n- Your microphone is not muted\n- Volume is turned up\n- Correct device is selected in System Settings"
            )
            
        except Exception as e:
            QMessageBox.warning(
                self,
                "Device Test Failed",
                f"Failed to test audio device:\n{str(e)}\n\nPlease check microphone permissions."
            )
