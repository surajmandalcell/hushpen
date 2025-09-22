"""Keyboard shortcuts and hotkeys settings page."""

import platform
import subprocess
from PySide6.QtCore import Qt, Signal
from PySide6.QtGui import QFont, QKeySequence, QKeyEvent
from PySide6.QtWidgets import QWidget, QVBoxLayout, QHBoxLayout, QLabel, QPushButton, QLineEdit, QMessageBox

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
                font-size: {Fonts.CONTROL_SIZE}px;
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
        
        # Build hotkey string (use Command instead of Ctrl on macOS)
        hotkey_parts = []
        is_macos = platform.system() == 'Darwin'
        
        if modifiers & Qt.KeyboardModifier.ControlModifier:
            hotkey_parts.append("Ctrl")
        if modifiers & Qt.KeyboardModifier.MetaModifier:
            # On macOS, Meta is the Command key
            hotkey_parts.append("Cmd" if is_macos else "Meta")
        if modifiers & Qt.KeyboardModifier.AltModifier:
            hotkey_parts.append("Alt" if not is_macos else "Option")
        if modifiers & Qt.KeyboardModifier.ShiftModifier:
            hotkey_parts.append("Shift")
        
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
        font.setPointSize(Fonts.scaled(Fonts.SETTING_LABEL_SIZE))
        font.setWeight(Fonts.adjust_weight(Fonts.MEDIUM))
        self.title_label.setFont(font)
        info_layout.addWidget(self.title_label)
        
        # Description label
        self.desc_label = QLabel(description)
        font = QFont()
        font.setPointSize(Fonts.scaled(Fonts.DESCRIPTION_SIZE))
        font.setWeight(Fonts.adjust_weight(Fonts.NORMAL))
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
                font-size: {Fonts.CONTROL_SIZE}px;
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
    
    def __init__(self, hotkey=None, parent=None):
        super().__init__("Keyboard", parent)
        self.hotkey = hotkey
        self._create_content()
    
    def _create_content(self):
        """Create simplified keyboard settings content."""
        # Main recording hotkey
        self.recording_hotkey = HotkeyRow(
            title="Show/Hide Overlay",
            description="Press once to start recording, press again to transcribe",
            setting_name="toggle_recording_hotkey",
            current_hotkey="Cmd+Shift+Space" if platform.system() == 'Darwin' else "Ctrl+Alt+T"
        )
        self.recording_hotkey.hotkey_changed.connect(self._on_hotkey_changed)
        self.add_content_widget(self.recording_hotkey)
        
        # Accessibility permissions section for macOS
        if platform.system() == 'Darwin':
            # Accessibility section
            access_label = QLabel("Accessibility Permissions")
            font = QFont()
            font.setPointSize(Fonts.scaled(Fonts.SETTING_LABEL_SIZE))
            font.setWeight(Fonts.adjust_weight(Fonts.MEDIUM))
            access_label.setFont(font)
            access_label.setStyleSheet(f"color: {DarkTheme.PRIMARY_TEXT.name()}; margin-top: 24px; margin-bottom: 8px;")
            self.add_content_widget(access_label)
            
            # Permission info widget
            permission_widget = QWidget()
            permission_layout = QVBoxLayout(permission_widget)
            permission_layout.setContentsMargins(0, 0, 0, 0)
            permission_layout.setSpacing(8)
            
            # Description
            desc_label = QLabel("ASR Pro requires accessibility permissions to use global hotkeys.")
            desc_font = QFont()
            desc_font.setPointSize(Fonts.scaled(Fonts.DESCRIPTION_SIZE))
            desc_font.setWeight(Fonts.adjust_weight(Fonts.NORMAL))
            desc_label.setFont(desc_font)
            desc_label.setStyleSheet(f"color: {DarkTheme.SECONDARY_TEXT.name()};")
            desc_label.setWordWrap(True)
            permission_layout.addWidget(desc_label)
            
            # Button to open settings
            self.permission_button = QPushButton("Open Accessibility Settings")
            self.permission_button.clicked.connect(self._open_accessibility_settings)
            self.permission_button.setStyleSheet(f"""
                QPushButton {{
                    background-color: {DarkTheme.ACCENT_BLUE.name()};
                    color: white;
                    border: none;
                    padding: 10px 20px;
                    border-radius: 6px;
                    font-size: {Fonts.CONTROL_SIZE}px;
                    font-weight: 500;
                }}
                QPushButton:hover {{
                    background-color: {DarkTheme.ACCENT_BLUE.darker(110).name()};
                }}
            """)
            self.permission_button.setMaximumWidth(250)
            permission_layout.addWidget(self.permission_button)
            
            self.add_content_widget(permission_widget)
        
        # Add stretch to push content to top
        self.add_stretch()
    
    def _open_accessibility_settings(self):
        """Open macOS accessibility settings."""
        try:
            # Open System Settings to Accessibility pane
            subprocess.run([
                'osascript', '-e',
                'tell application "System Settings" to reveal anchor "Privacy_Accessibility" of pane id "com.apple.preference.security"'
            ])
            subprocess.run(['osascript', '-e', 'tell application "System Settings" to activate'])
            
            # Show info message
            QMessageBox.information(
                self,
                "Accessibility Settings",
                "Please add Terminal (or Python) to the allowed apps in Accessibility.\n\n"
                "After granting permission, restart ASR Pro for the changes to take effect."
            )
        except Exception as e:
            QMessageBox.warning(
                self,
                "Error",
                f"Failed to open System Settings: {str(e)}"
            )
    
    def _on_hotkey_changed(self, setting_name: str, hotkey: str):
        """Handle hotkey changes."""
        print(f"[KeyboardPage] Hotkey changed: {setting_name} = {hotkey}")
        
        # Convert display format to pynput format
        pynput_hotkey = self._convert_to_pynput_format(hotkey)
        
        # Update the hotkey in the system
        if self.hotkey:
            self.hotkey.set_hotkey(pynput_hotkey)
    
    def _convert_to_pynput_format(self, hotkey: str) -> str:
        """Convert displayed hotkey format to pynput format."""
        # Replace display names with pynput names
        conversions = {
            'Cmd': 'cmd',
            'Command': 'cmd',
            'Option': 'alt',
            'Ctrl': 'ctrl',
            'Shift': 'shift',
            'Space': 'space',
        }
        
        parts = hotkey.split('+')
        pynput_parts = []
        
        for part in parts:
            part = part.strip()
            # Convert to lowercase and check conversions
            lower_part = conversions.get(part, part.lower())
            pynput_parts.append(lower_part)
        
        # pynput format uses angle brackets for modifiers
        result = '+'.join(f'<{p}>' if p in ['cmd', 'ctrl', 'alt', 'shift'] else p for p in pynput_parts)
        return result
