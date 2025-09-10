"""Keyboard shortcuts and hotkeys settings page."""

from PySide6.QtCore import Qt, Signal
from PySide6.QtGui import QFont, QKeySequence, QKeyEvent
from PySide6.QtWidgets import QWidget, QVBoxLayout, QHBoxLayout, QLabel, QPushButton, QLineEdit

from ..components.setting_row import SettingRow
from ..styles.dark_theme import DarkTheme, Dimensions, Fonts, Spacing
from .base_page import BasePage


class HotkeyInput(QLineEdit):
    """Custom input widget for capturing hotkey combinations."""
    
    hotkey_changed = Signal(str)
    
    def __init__(self, parent=None):
        super().__init__(parent)
        
        self.setReadOnly(True)
        self.setPlaceholderText("Click and press keys...")
        self._current_hotkey = ""
        self._recording = False
        
        # Style the input
        self._apply_styles()
    
    def _apply_styles(self):
        """Apply hotkey input styling."""
        self.setStyleSheet(f"""
            HotkeyInput {{
                background-color: {DarkTheme.CONTROL_BG.name()};
                color: {DarkTheme.PRIMARY_TEXT.name()};
                border: 2px solid {DarkTheme.CARD_BORDER.name()};
                padding: 8px 12px;
                border-radius: 6px;
                font-size: {Fonts.BASE_SIZE}px;
                min-width: 200px;
            }}
            HotkeyInput:focus {{
                border: 2px solid {DarkTheme.ACCENT_BLUE.name()};
            }}
        """)
    
    def mousePressEvent(self, event):
        """Handle mouse press to start recording."""
        if event.button() == Qt.MouseButton.LeftButton:
            self._start_recording()
        super().mousePressEvent(event)
    
    def _start_recording(self):
        """Start recording hotkey combination."""
        self._recording = True
        self.setText("Recording... (Press Escape to cancel)")
        self.setFocus()
    
    def keyPressEvent(self, event: QKeyEvent):
        """Handle key press events for hotkey capture."""
        if not self._recording:
            super().keyPressEvent(event)
            return
        
        key = event.key()
        modifiers = event.modifiers()
        
        # Handle escape to cancel
        if key == Qt.Key.Key_Escape:
            self._cancel_recording()
            return
        
        # Ignore modifier-only presses
        if key in (Qt.Key.Key_Control, Qt.Key.Key_Shift, Qt.Key.Key_Alt, Qt.Key.Key_Meta):
            return
        
        # Build hotkey string
        hotkey_parts = []
        
        if modifiers & Qt.KeyboardModifier.ControlModifier:
            hotkey_parts.append("Ctrl")
        if modifiers & Qt.KeyboardModifier.AltModifier:
            hotkey_parts.append("Alt")
        if modifiers & Qt.KeyboardModifier.ShiftModifier:
            hotkey_parts.append("Shift")
        if modifiers & Qt.KeyboardModifier.MetaModifier:
            hotkey_parts.append("Meta")
        
        # Add the main key
        key_text = QKeySequence(key).toString()
        if key_text:
            hotkey_parts.append(key_text)
        
        if hotkey_parts:
            self._current_hotkey = "+".join(hotkey_parts)
            self._finish_recording()
    
    def _finish_recording(self):
        """Finish recording and save hotkey."""
        self._recording = False
        self.setText(self._current_hotkey)
        self.clearFocus()
        self.hotkey_changed.emit(self._current_hotkey)
    
    def _cancel_recording(self):
        """Cancel hotkey recording."""
        self._recording = False
        self.setText(self._current_hotkey)
        self.clearFocus()
    
    def set_hotkey(self, hotkey: str):
        """Set the current hotkey value."""
        self._current_hotkey = hotkey
        self.setText(hotkey)
    
    def get_hotkey(self) -> str:
        """Get the current hotkey value."""
        return self._current_hotkey


class HotkeyRow(QWidget):
    """Custom row widget for hotkey settings."""
    
    hotkey_changed = Signal(str, str)  # setting_name, hotkey
    
    def __init__(self, title: str, description: str, setting_name: str, current_hotkey: str = "", parent=None):
        super().__init__(parent)
        
        self.setting_name = setting_name
        self._setup_ui(title, description, current_hotkey)
        self._apply_styles()
    
    def _setup_ui(self, title: str, description: str, current_hotkey: str):
        """Set up the hotkey row layout."""
        layout = QHBoxLayout(self)
        layout.setContentsMargins(0, Spacing.ITEM_PADDING_V, 0, Spacing.ITEM_PADDING_V)
        layout.setSpacing(0)
        
        # Left side: Info section
        info_layout = QVBoxLayout()
        info_layout.setContentsMargins(0, 0, 0, 0)
        info_layout.setSpacing(4)
        
        # Title label
        self.title_label = QLabel(title)
        font = QFont()
        font.setPointSize(Fonts.SETTING_LABEL_SIZE)
        font.setWeight(Fonts.MEDIUM)
        self.title_label.setFont(font)
        info_layout.addWidget(self.title_label)
        
        # Description label
        self.desc_label = QLabel(description)
        font = QFont()
        font.setPointSize(Fonts.DESCRIPTION_SIZE)
        font.setWeight(Fonts.NORMAL)
        self.desc_label.setFont(font)
        self.desc_label.setWordWrap(True)
        info_layout.addWidget(self.desc_label)
        
        info_widget = QWidget()
        info_widget.setLayout(info_layout)
        layout.addWidget(info_widget, 1)
        
        # Right side: Hotkey input and buttons
        controls_layout = QHBoxLayout()
        controls_layout.setSpacing(8)
        
        # Hotkey input
        self.hotkey_input = HotkeyInput()
        self.hotkey_input.set_hotkey(current_hotkey)
        self.hotkey_input.hotkey_changed.connect(self._on_hotkey_changed)
        controls_layout.addWidget(self.hotkey_input)
        
        # Clear button
        self.clear_button = QPushButton("Clear")
        self.clear_button.clicked.connect(self._clear_hotkey)
        controls_layout.addWidget(self.clear_button)
        
        layout.addSpacing(Spacing.CONTROL_MARGIN)
        layout.addLayout(controls_layout)
    
    def _apply_styles(self):
        """Apply hotkey row styling."""
        self.title_label.setStyleSheet(f"color: {DarkTheme.PRIMARY_TEXT.name()};")
        self.desc_label.setStyleSheet(f"color: {DarkTheme.SECONDARY_TEXT.name()};")
        
        # Clear button styling
        self.clear_button.setStyleSheet(f"""
            QPushButton {{
                background-color: {DarkTheme.BUTTON_BG.name()};
                color: {DarkTheme.PRIMARY_TEXT.name()};
                border: none;
                padding: 8px 12px;
                border-radius: 6px;
                font-size: {Fonts.BASE_SIZE}px;
                min-width: 60px;
            }}
            QPushButton:hover {{
                background-color: {DarkTheme.BUTTON_HOVER_BG.name()};
            }}
        """)
    
    def _on_hotkey_changed(self, hotkey: str):
        """Handle hotkey changes."""
        self.hotkey_changed.emit(self.setting_name, hotkey)
    
    def _clear_hotkey(self):
        """Clear the current hotkey."""
        self.hotkey_input.set_hotkey("")
        self._on_hotkey_changed("")
    
    def set_hotkey(self, hotkey: str):
        """Set the hotkey value."""
        self.hotkey_input.set_hotkey(hotkey)


class KeyboardPage(BasePage):
    """Keyboard shortcuts and hotkeys settings."""
    
    def __init__(self, parent=None):
        super().__init__("Keyboard", parent)
        self._create_content()
    
    def _create_content(self):
        """Create keyboard settings content."""
        # Global Hotkey section
        global_label = QLabel("Global Hotkeys")
        font = QFont()
        font.setPointSize(Fonts.SETTING_LABEL_SIZE)
        font.setWeight(Fonts.MEDIUM)
        global_label.setFont(font)
        global_label.setStyleSheet(f"color: {DarkTheme.PRIMARY_TEXT.name()}; margin-bottom: 8px;")
        self.add_content_widget(global_label)
        
        # Start/Stop Recording hotkey
        recording_hotkey = HotkeyRow(
            title="Start/Stop Recording",
            description="Global hotkey to toggle recording on and off",
            setting_name="toggle_recording_hotkey",
            current_hotkey="Ctrl+Shift+R"
        )
        recording_hotkey.hotkey_changed.connect(self._on_setting_changed)
        self.add_content_widget(recording_hotkey)
        
        # Show/Hide Window hotkey
        window_hotkey = HotkeyRow(
            title="Show/Hide Window",
            description="Global hotkey to show or hide the main application window",
            setting_name="toggle_window_hotkey",
            current_hotkey="Ctrl+Shift+A"
        )
        window_hotkey.hotkey_changed.connect(self._on_setting_changed)
        self.add_content_widget(window_hotkey)
        
        # Quick Paste hotkey
        paste_hotkey = HotkeyRow(
            title="Quick Paste",
            description="Instantly paste the last transcribed text",
            setting_name="quick_paste_hotkey",
            current_hotkey="Ctrl+Shift+V"
        )
        paste_hotkey.hotkey_changed.connect(self._on_setting_changed)
        self.add_content_widget(paste_hotkey)
        
        # Text Processing section
        processing_label = QLabel("Text Processing")
        font = QFont()
        font.setPointSize(Fonts.SETTING_LABEL_SIZE)
        font.setWeight(Fonts.MEDIUM)
        processing_label.setFont(font)
        processing_label.setStyleSheet(f"color: {DarkTheme.PRIMARY_TEXT.name()}; margin-top: 24px; margin-bottom: 8px;")
        self.add_content_widget(processing_label)
        
        # Auto-capitalize setting
        capitalize_setting = SettingRow(
            title="Auto-capitalize",
            description="Automatically capitalize the first letter of sentences",
            setting_name="auto_capitalize",
            control_type="toggle",
            current_value=True
        )
        capitalize_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(capitalize_setting)
        
        # Auto-punctuation setting
        punctuation_setting = SettingRow(
            title="Smart Punctuation",
            description="Automatically add punctuation based on speech patterns",
            setting_name="smart_punctuation",
            control_type="toggle",
            current_value=True
        )
        punctuation_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(punctuation_setting)
        
        # Text Replacement setting
        replacement_setting = SettingRow(
            title="Text Replacement",
            description="Enable custom text replacements (e.g., 'u' → 'you')",
            setting_name="text_replacement",
            control_type="toggle",
            current_value=False
        )
        replacement_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(replacement_setting)
        
        # Word Spacing setting
        spacing_setting = SettingRow(
            title="Word Spacing",
            description="Add appropriate spacing between words",
            setting_name="word_spacing",
            control_type="dropdown",
            options=[
                {"label": "Single Space", "value": "single"},
                {"label": "Smart Spacing", "value": "smart"},
                {"label": "No Extra Spacing", "value": "none"}
            ],
            current_value="smart"
        )
        spacing_setting.value_changed.connect(self._on_setting_changed)
        self.add_content_widget(spacing_setting)
        
        # Add stretch to push content to top
        self.add_stretch()