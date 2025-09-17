"""Models/Dictation Engine settings page."""

from PySide6.QtCore import Qt
from PySide6.QtGui import QFont, QPainter, QBrush
from PySide6.QtWidgets import QWidget, QVBoxLayout, QHBoxLayout, QLabel, QPushButton

from ..layouts.setting_row import SettingRow
from ..styles.dark_theme import DarkTheme, Dimensions, Fonts, Spacing
from .base_page import BasePage


class ModelCard(QWidget):
    """Card widget for displaying model information."""
    
    def __init__(self, name: str, description: str, size: str, status: str = "Available", parent=None):
        super().__init__(parent)
        
        self.name = name
        self.description = description
        self.model_size = size  # Renamed to avoid conflict with QWidget.size()
        self.status = status
        
        self._setup_ui()
        self._apply_styles()
    
    def _setup_ui(self):
        """Set up the model card layout."""
        layout = QVBoxLayout(self)
        layout.setContentsMargins(Spacing.CARD_PADDING, Spacing.CARD_PADDING,
                                 Spacing.CARD_PADDING, Spacing.CARD_PADDING)
        layout.setSpacing(8)
        
        # Header row with name and status
        header_layout = QHBoxLayout()
        header_layout.setSpacing(0)
        
        # Model name
        self.name_label = QLabel(self.name)
        font = QFont()
        font.setPointSize(Fonts.scaled(Fonts.SETTING_LABEL_SIZE))
        font.setWeight(Fonts.adjust_weight(Fonts.MEDIUM))
        self.name_label.setFont(font)
        header_layout.addWidget(self.name_label)
        
        header_layout.addStretch()
        
        # Status badge
        self.status_label = QLabel(self.status)
        status_font = QFont()
        status_font.setPointSize(Fonts.scaled(Fonts.DESCRIPTION_SIZE))
        status_font.setWeight(Fonts.adjust_weight(Fonts.NORMAL))
        self.status_label.setFont(status_font)
        header_layout.addWidget(self.status_label)
        
        layout.addLayout(header_layout)
        
        # Description
        self.desc_label = QLabel(self.description)
        desc_font = QFont()
        desc_font.setPointSize(Fonts.scaled(Fonts.DESCRIPTION_SIZE))
        desc_font.setWeight(Fonts.adjust_weight(Fonts.NORMAL))
        self.desc_label.setFont(desc_font)
        self.desc_label.setWordWrap(True)
        layout.addWidget(self.desc_label)
        
        # Size info
        self.size_label = QLabel(f"Size: {self.model_size}")
        size_font = QFont()
        size_font.setPointSize(Fonts.scaled(Fonts.DESCRIPTION_SIZE))
        size_font.setWeight(Fonts.adjust_weight(Fonts.NORMAL))
        self.size_label.setFont(size_font)
        layout.addWidget(self.size_label)
        
        # Action button
        self.action_button = QPushButton("Download" if self.status == "Available" else "Select")
        layout.addWidget(self.action_button)
    
    def _apply_styles(self):
        """Apply model card styling."""
        # Main card background
        self.setStyleSheet(f"""
            ModelCard {{
                background-color: {DarkTheme.CARD_BG.name()};
                border: 1px solid {DarkTheme.CARD_BORDER.name()};
                border-radius: {Dimensions.CARD_RADIUS}px;
            }}
        """)
        
        # Text colors
        self.name_label.setStyleSheet(f"color: {DarkTheme.PRIMARY_TEXT.name()};")
        self.desc_label.setStyleSheet(f"color: {DarkTheme.SECONDARY_TEXT.name()};")
        self.size_label.setStyleSheet(f"color: {DarkTheme.SECONDARY_TEXT.name()};")
        
        # Status color based on status
        if self.status == "Downloaded":
            status_color = DarkTheme.SUCCESS_GREEN.name()
        elif self.status == "Downloading":
            status_color = DarkTheme.WARNING_YELLOW.name()
        else:
            status_color = DarkTheme.SECONDARY_TEXT.name()
        
        self.status_label.setStyleSheet(f"color: {status_color};")
        
        # Action button styling
        self.action_button.setStyleSheet(f"""
            QPushButton {{
                background-color: {DarkTheme.ACCENT_BLUE.name()};
                color: {DarkTheme.PRIMARY_TEXT.name()};
                border: none;
                padding: 8px 16px;
                border-radius: 6px;
                font-size: {Fonts.CONTROL_SIZE}px;
                font-weight: {Fonts.MEDIUM};
            }}
            QPushButton:hover {{
                background-color: {DarkTheme.ACCENT_BLUE.lighter(110).name()};
            }}
            QPushButton:pressed {{
                background-color: {DarkTheme.ACCENT_BLUE.darker(110).name()};
            }}
        """)


class ModelsPage(BasePage):
    """Models and dictation engine settings."""
    
    def __init__(self, parent=None):
        super().__init__("Dictation Engine", parent)
        self._create_content()
    
    def _create_content(self):
        """Create models settings content."""
        # Current Model setting
        current_model_setting = SettingRow(
            title="Current Model",
            description="The currently selected dictation model",
            setting_name="current_model",
            control_type="dropdown",
            options=[
                {"label": "Whisper Tiny (39 MB)", "value": "whisper-tiny"},
                {"label": "Whisper Base (74 MB)", "value": "whisper-base"},
                {"label": "Whisper Small (244 MB)", "value": "whisper-small"},
                {"label": "Whisper Medium (769 MB)", "value": "whisper-medium"},
                {"label": "Whisper Large (1550 MB)", "value": "whisper-large"}
            ],
            current_value="whisper-base"
        )
        current_model_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(current_model_setting)
        
        # Language setting
        language_setting = SettingRow(
            title="Default Language",
            description="Set the primary language for dictation recognition",
            setting_name="dictation_language",
            control_type="dropdown",
            options=[
                {"label": "Auto-detect", "value": "auto"},
                {"label": "English", "value": "en"},
                {"label": "Spanish", "value": "es"},
                {"label": "French", "value": "fr"},
                {"label": "German", "value": "de"},
                {"label": "Italian", "value": "it"},
                {"label": "Portuguese", "value": "pt"},
                {"label": "Russian", "value": "ru"},
                {"label": "Japanese", "value": "ja"},
                {"label": "Chinese", "value": "zh"}
            ],
            current_value="auto"
        )
        language_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(language_setting)
        
        # Processing Device setting
        device_setting = SettingRow(
            title="Processing Device",
            description="Choose the hardware device for model processing",
            setting_name="processing_device",
            control_type="dropdown",
            options=[
                {"label": "CPU (Default)", "value": "cpu"},
                {"label": "GPU (CUDA)", "value": "cuda"},
                {"label": "GPU (Metal)", "value": "metal"}
            ],
            current_value="cpu"
        )
        device_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(device_setting)
        
        # Real-time Processing setting
        realtime_setting = SettingRow(
            title="Real-time Processing",
            description="Enable real-time transcription during recording",
            setting_name="realtime_processing",
            control_type="toggle",
            current_value=False
        )
        realtime_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(realtime_setting)
        
        # Model Storage Location setting
        storage_setting = SettingRow(
            title="Model Storage Location",
            description="Directory where downloaded models are stored",
            setting_name="model_storage_location",
            control_type="folder",
            current_value=""
        )
        storage_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(storage_setting)
        
        # Available Models section
        models_label = QLabel("Available Models")
        font = QFont()
        font.setPointSize(Fonts.scaled(Fonts.SETTING_LABEL_SIZE))
        font.setWeight(Fonts.adjust_weight(Fonts.MEDIUM))
        models_label.setFont(font)
        models_label.setStyleSheet(f"color: {DarkTheme.PRIMARY_TEXT.name()}; margin-top: 16px;")
        self.add_content_widget(models_label)
        
        # Create model cards
        models_data = [
            ("Whisper Tiny", "Fastest processing, lower accuracy. Good for real-time use.", "39 MB", "Downloaded"),
            ("Whisper Base", "Balanced speed and accuracy. Recommended for most users.", "74 MB", "Downloaded"),
            ("Whisper Small", "Better accuracy, slower processing. Good for quality transcription.", "244 MB", "Available"),
            ("Whisper Medium", "High accuracy, requires more processing power.", "769 MB", "Available"),
            ("Whisper Large", "Best accuracy, slowest processing. For maximum quality.", "1550 MB", "Available")
        ]
        
        for name, description, size, status in models_data:
            card = ModelCard(name, description, size, status)
            self.add_content_widget(card)
        
        # Add stretch to push content to top
        self.add_stretch()