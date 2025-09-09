"""Professional hotkey capture dialog with real-time key detection."""

from __future__ import annotations

from PySide6.QtCore import Qt, QTimer, Signal
from PySide6.QtWidgets import (
    QDialog,
    QVBoxLayout,
    QHBoxLayout,
    QLabel,
    QPushButton,
    QFrame,
)
from PySide6.QtGui import QKeyEvent, QFont


class HotkeyDialog(QDialog):
    """Professional hotkey capture dialog."""
    
    hotkey_captured = Signal(str)
    
    def __init__(self, parent=None, current_hotkey=""):
        super().__init__(parent)
        self.current_keys = set()
        self.captured_hotkey = ""
        self.current_hotkey = current_hotkey
        
        self.setFixedSize(400, 200)
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint | Qt.WindowType.Dialog)
        self.setAttribute(Qt.WidgetAttribute.WA_DeleteOnClose)
        
        # Make it translucent and modern
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        
        self.setup_ui()
        self.setFocusPolicy(Qt.FocusPolicy.StrongFocus)
        
    def setup_ui(self):
        """Setup the dialog UI."""
        layout = QVBoxLayout(self)
        layout.setSpacing(16)
        layout.setContentsMargins(24, 24, 24, 24)
        
        # Title
        title = QLabel("Press your desired hotkey combination")
        title.setAlignment(Qt.AlignmentFlag.AlignCenter)
        font = QFont()
        font.setPointSize(14)
        font.setBold(True)
        title.setFont(font)
        layout.addWidget(title)
        
        # Current hotkey display
        if self.current_hotkey:
            current_label = QLabel(f"Current: {self.current_hotkey}")
            current_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
            current_label.setStyleSheet("color: #666; font-size: 12px;")
            layout.addWidget(current_label)
        
        # Key display frame
        self.key_frame = QFrame()
        self.key_frame.setFrameStyle(QFrame.Shape.Box)
        self.key_frame.setStyleSheet("""
            QFrame {
                background-color: #2a2a2a;
                border: 2px solid #0a84ff;
                border-radius: 8px;
                padding: 16px;
                min-height: 40px;
            }
        """)
        
        key_layout = QVBoxLayout(self.key_frame)
        
        self.key_display = QLabel("Waiting for keys...")
        self.key_display.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.key_display.setStyleSheet("""
            QLabel {
                color: #ffffff;
                font-size: 16px;
                font-weight: bold;
                background: transparent;
                border: none;
            }
        """)
        key_layout.addWidget(self.key_display)
        
        layout.addWidget(self.key_frame)
        
        # Instructions
        instructions = QLabel("Press ESC to cancel • Press ENTER to accept")
        instructions.setAlignment(Qt.AlignmentFlag.AlignCenter)
        instructions.setStyleSheet("color: #999; font-size: 11px;")
        layout.addWidget(instructions)
        
        # Buttons
        button_layout = QHBoxLayout()
        button_layout.addStretch()
        
        self.cancel_btn = QPushButton("Cancel")
        self.cancel_btn.clicked.connect(self.reject)
        self.cancel_btn.setStyleSheet("""
            QPushButton {
                background-color: #2a2a2a;
                color: #ffffff;
                border: 1px solid #444;
                border-radius: 6px;
                padding: 8px 16px;
                font-size: 12px;
            }
            QPushButton:hover {
                background-color: #333;
            }
        """)
        
        self.accept_btn = QPushButton("Accept")
        self.accept_btn.clicked.connect(self.accept_hotkey)
        self.accept_btn.setEnabled(False)
        self.accept_btn.setStyleSheet("""
            QPushButton {
                background-color: #0a84ff;
                color: #ffffff;
                border: none;
                border-radius: 6px;
                padding: 8px 16px;
                font-size: 12px;
                font-weight: bold;
            }
            QPushButton:hover {
                background-color: #0968c4;
            }
            QPushButton:disabled {
                background-color: #333;
                color: #666;
            }
        """)
        
        button_layout.addWidget(self.cancel_btn)
        button_layout.addWidget(self.accept_btn)
        layout.addLayout(button_layout)  # Use addLayout for layouts, not addWidget
        
        # Style the dialog with Mac-like appearance
        self.setStyleSheet("""
            QDialog {
                background-color: rgba(30, 30, 30, 0.95);
                color: #ffffff;
                border-radius: 12px;
                border: 1px solid rgba(255, 255, 255, 0.1);
            }
        """)
        
        # Timer to clear old keys
        self.clear_timer = QTimer()
        self.clear_timer.setSingleShot(True)
        self.clear_timer.timeout.connect(self.clear_keys)
        
    def keyPressEvent(self, event: QKeyEvent):
        """Handle key press events."""
        key = event.key()
        
        # Handle special keys
        if key == Qt.Key.Key_Escape:
            self.reject()
            return
        elif key == Qt.Key.Key_Return or key == Qt.Key.Key_Enter:
            if self.captured_hotkey:
                self.accept_hotkey()
            return
        
        # Add the key to current keys
        modifiers = event.modifiers()
        self.current_keys.clear()
        
        # Add modifiers
        if modifiers & Qt.KeyboardModifier.ControlModifier:
            self.current_keys.add("Ctrl")
        if modifiers & Qt.KeyboardModifier.AltModifier:
            self.current_keys.add("Alt")
        if modifiers & Qt.KeyboardModifier.ShiftModifier:
            self.current_keys.add("Shift")
        if modifiers & Qt.KeyboardModifier.MetaModifier:
            self.current_keys.add("Cmd")
        
        # Add the actual key (if it's not a modifier)
        if key not in (Qt.Key.Key_Control, Qt.Key.Key_Alt, Qt.Key.Key_Shift, Qt.Key.Key_Meta):
            key_name = self.get_key_name(key)
            if key_name:
                self.current_keys.add(key_name)
        
        self.update_display()
        
        # Reset timer
        self.clear_timer.stop()
        self.clear_timer.start(2000)  # Clear after 2 seconds of inactivity
        
    def keyReleaseEvent(self, event: QKeyEvent):
        """Handle key release events."""
        # Don't clear on modifier release, wait for timer
        pass
        
    def get_key_name(self, key: int) -> str:
        """Convert Qt key code to readable name."""
        key_map = {
            Qt.Key.Key_Space: "Space",
            Qt.Key.Key_Tab: "Tab",
            Qt.Key.Key_Backspace: "Backspace",
            Qt.Key.Key_Delete: "Delete",
            Qt.Key.Key_Insert: "Insert",
            Qt.Key.Key_Home: "Home",
            Qt.Key.Key_End: "End",
            Qt.Key.Key_PageUp: "PageUp",
            Qt.Key.Key_PageDown: "PageDown",
            Qt.Key.Key_Up: "Up",
            Qt.Key.Key_Down: "Down",
            Qt.Key.Key_Left: "Left",
            Qt.Key.Key_Right: "Right",
            Qt.Key.Key_F1: "F1", Qt.Key.Key_F2: "F2", Qt.Key.Key_F3: "F3", Qt.Key.Key_F4: "F4",
            Qt.Key.Key_F5: "F5", Qt.Key.Key_F6: "F6", Qt.Key.Key_F7: "F7", Qt.Key.Key_F8: "F8",
            Qt.Key.Key_F9: "F9", Qt.Key.Key_F10: "F10", Qt.Key.Key_F11: "F11", Qt.Key.Key_F12: "F12",
            Qt.Key.Key_Comma: ",", Qt.Key.Key_Period: ".", Qt.Key.Key_Semicolon: ";",
            Qt.Key.Key_Apostrophe: "'", Qt.Key.Key_BracketLeft: "[", Qt.Key.Key_BracketRight: "]",
            Qt.Key.Key_Backslash: "\\", Qt.Key.Key_Slash: "/", Qt.Key.Key_Minus: "-", Qt.Key.Key_Equal: "=",
            Qt.Key.Key_QuoteLeft: "`",
        }
        
        if key in key_map:
            return key_map[key]
        elif Qt.Key.Key_A <= key <= Qt.Key.Key_Z:
            return chr(key).upper()
        elif Qt.Key.Key_0 <= key <= Qt.Key.Key_9:
            return chr(key)
        else:
            return ""
    
    def update_display(self):
        """Update the key display."""
        if self.current_keys:
            # Sort modifiers first, then other keys
            modifiers = ["Ctrl", "Alt", "Shift", "Cmd"]
            sorted_keys = []
            
            for mod in modifiers:
                if mod in self.current_keys:
                    sorted_keys.append(mod)
            
            for key in sorted(self.current_keys):
                if key not in modifiers:
                    sorted_keys.append(key)
            
            display_text = " + ".join(sorted_keys)
            self.key_display.setText(display_text)
            
            # Convert to format expected by the app
            hotkey_parts = []
            if "Ctrl" in self.current_keys:
                hotkey_parts.append("<ctrl>")
            if "Alt" in self.current_keys:
                hotkey_parts.append("<alt>")
            if "Shift" in self.current_keys:
                hotkey_parts.append("<shift>")
            if "Cmd" in self.current_keys:
                hotkey_parts.append("<cmd>")
            
            # Add non-modifier keys
            for key in self.current_keys:
                if key not in ["Ctrl", "Alt", "Shift", "Cmd"]:
                    hotkey_parts.append(key.lower())
            
            self.captured_hotkey = "+".join(hotkey_parts)
            self.accept_btn.setEnabled(bool(self.captured_hotkey))
            
            # Update frame style for active state
            self.key_frame.setStyleSheet("""
                QFrame {
                    background-color: #2a2a2a;
                    border: 2px solid #28ca42;
                    border-radius: 8px;
                    padding: 16px;
                    min-height: 40px;
                }
            """)
        else:
            self.key_display.setText("Waiting for keys...")
            self.captured_hotkey = ""
            self.accept_btn.setEnabled(False)
            
            # Reset frame style
            self.key_frame.setStyleSheet("""
                QFrame {
                    background-color: #2a2a2a;
                    border: 2px solid #0a84ff;
                    border-radius: 8px;
                    padding: 16px;
                    min-height: 40px;
                }
            """)
    
    def clear_keys(self):
        """Clear current keys after timeout."""
        self.current_keys.clear()
        self.update_display()
    
    def accept_hotkey(self):
        """Accept the captured hotkey."""
        if self.captured_hotkey:
            self.hotkey_captured.emit(self.captured_hotkey)
            self.accept()


__all__ = ["HotkeyDialog"]