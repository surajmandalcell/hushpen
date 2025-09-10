"""Setting row widget for consistent setting layout."""

from typing import Optional, Any, Union
from PySide6.QtCore import Qt, Signal
from PySide6.QtGui import QFont
from PySide6.QtWidgets import (
    QWidget, QHBoxLayout, QVBoxLayout, QLabel, 
    QComboBox, QLineEdit, QPushButton, QFileDialog
)

from ..styles.dark_theme import DarkTheme, Dimensions, Fonts, Spacing
from .toggle_switch import ToggleSwitch


class SettingRow(QWidget):
    """A standardized setting row with label, description, and control."""
    
    value_changed = Signal(str, object)  # setting_name, new_value
    
    def __init__(self, 
                 title: str,
                 description: str,
                 setting_name: str,
                 control_type: str = "toggle",
                 options: Optional[list] = None,
                 current_value: Any = None,
                 parent=None):
        super().__init__(parent)
        
        self.setting_name = setting_name
        self.control_type = control_type
        self.current_value = current_value
        
        self._setup_ui(title, description, options)
        self._apply_styles()
        
        # Set initial value
        if current_value is not None:
            self.set_value(current_value)
    
    def _setup_ui(self, title: str, description: str, options: Optional[list]):
        """Set up the setting row layout."""
        layout = QHBoxLayout(self)
        layout.setContentsMargins(0, Spacing.ITEM_PADDING_V, 0, Spacing.ITEM_PADDING_V)
        layout.setSpacing(0)
        
        # Left side: Info section
        info_layout = QVBoxLayout()
        info_layout.setContentsMargins(0, 0, 0, 0)
        info_layout.setSpacing(4)  # 4px margin between title and description
        
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
        layout.addWidget(info_widget, 1)  # Take remaining space
        
        # Right side: Control section
        self.control_widget = self._create_control(options)
        if self.control_widget:
            layout.addSpacing(Spacing.CONTROL_MARGIN)
            layout.addWidget(self.control_widget)
    
    def _create_control(self, options: Optional[list]) -> Optional[QWidget]:
        """Create the appropriate control widget."""
        if self.control_type == "toggle":
            control = ToggleSwitch()
            control.toggled.connect(lambda checked: self._emit_value_changed(checked))
            return control
        
        elif self.control_type == "dropdown":
            control = QComboBox()
            if options:
                for option in options:
                    if isinstance(option, dict):
                        control.addItem(option.get("label", ""), option.get("value", ""))
                    else:
                        control.addItem(str(option), option)
            control.currentTextChanged.connect(lambda text: self._emit_value_changed(control.currentData()))
            return control
        
        elif self.control_type == "folder":
            return self._create_folder_input()
        
        elif self.control_type == "text":
            control = QLineEdit()
            control.textChanged.connect(lambda text: self._emit_value_changed(text))
            return control
        
        elif self.control_type == "readonly":
            control = QLabel()
            return control
        
        return None
    
    def _create_folder_input(self) -> QWidget:
        """Create a folder input with text field and browse button."""
        container = QWidget()
        layout = QHBoxLayout(container)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(8)  # 8px gap as per original design
        
        # Text input
        self.folder_input = QLineEdit()
        self.folder_input.setReadOnly(True)
        layout.addWidget(self.folder_input)
        
        # Browse button
        browse_btn = QPushButton("Browse")
        browse_btn.clicked.connect(self._browse_folder)
        layout.addWidget(browse_btn)
        
        return container
    
    def _browse_folder(self):
        """Open folder browser dialog."""
        folder = QFileDialog.getExistingDirectory(
            self,
            "Select Folder",
            self.folder_input.text() if hasattr(self, 'folder_input') else ""
        )
        if folder and hasattr(self, 'folder_input'):
            self.folder_input.setText(folder)
            self._emit_value_changed(folder)
    
    def _apply_styles(self):
        """Apply dark theme styles to the components."""
        # Title color
        self.title_label.setStyleSheet(f"color: {DarkTheme.PRIMARY_TEXT.name()};")
        
        # Description color  
        self.desc_label.setStyleSheet(f"color: {DarkTheme.SECONDARY_TEXT.name()};")
        
        # Style control widget based on type
        if self.control_widget is not None:
            self._style_control_widget()
    
    def _style_control_widget(self):
        """Apply styles to the control widget."""
        if self.control_type == "dropdown" and self.control_widget is not None:
            self.control_widget.setStyleSheet(f"""
                QComboBox {{
                    background-color: {DarkTheme.CONTROL_BG.name()};
                    color: {DarkTheme.PRIMARY_TEXT.name()};
                    border: none;
                    padding: 8px 32px 8px 12px;
                    border-radius: 6px;
                    font-size: {Fonts.BASE_SIZE}px;
                    min-width: 120px;
                }}
                QComboBox:focus {{
                    border: 2px solid {DarkTheme.FOCUS_BORDER.name()};
                }}
                QComboBox::drop-down {{
                    subcontrol-origin: padding;
                    subcontrol-position: top right;
                    width: 20px;
                    border: none;
                }}
                QComboBox::down-arrow {{
                    image: none;
                    border-left: 4px solid transparent;
                    border-right: 4px solid transparent;
                    border-top: 4px solid {DarkTheme.SECONDARY_TEXT.name()};
                    width: 0px;
                    height: 0px;
                }}
                QComboBox QAbstractItemView {{
                    background-color: {DarkTheme.CONTROL_BG.name()};
                    color: {DarkTheme.PRIMARY_TEXT.name()};
                    selection-background-color: {DarkTheme.ACCENT_BLUE.name()};
                    border: 1px solid {DarkTheme.CARD_BORDER.name()};
                    border-radius: 6px;
                }}
            """)
        
        elif self.control_type == "folder" and self.control_widget is not None:
            folder_style = f"""
                QLineEdit {{
                    background-color: {DarkTheme.CONTROL_BG.name()};
                    color: {DarkTheme.PRIMARY_TEXT.name()};
                    border: none;
                    padding: 8px 12px;
                    border-radius: 6px;
                    font-size: {Fonts.BASE_SIZE}px;
                    min-width: 200px;
                }}
                QLineEdit:focus {{
                    border: 2px solid {DarkTheme.FOCUS_BORDER.name()};
                }}
                QPushButton {{
                    background-color: {DarkTheme.BUTTON_BG.name()};
                    color: {DarkTheme.PRIMARY_TEXT.name()};
                    border: none;
                    padding: 8px 12px;
                    border-radius: 6px;
                    font-size: {Fonts.BASE_SIZE}px;
                }}
                QPushButton:hover {{
                    background-color: {DarkTheme.BUTTON_HOVER_BG.name()};
                }}
            """
            self.control_widget.setStyleSheet(folder_style)
        
        elif self.control_type == "text" and self.control_widget is not None:
            self.control_widget.setStyleSheet(f"""
                QLineEdit {{
                    background-color: {DarkTheme.CONTROL_BG.name()};
                    color: {DarkTheme.PRIMARY_TEXT.name()};
                    border: none;
                    padding: 8px 12px;
                    border-radius: 6px;
                    font-size: {Fonts.BASE_SIZE}px;
                    min-width: 200px;
                }}
                QLineEdit:focus {{
                    border: 2px solid {DarkTheme.FOCUS_BORDER.name()};
                }}
            """)
        
        elif self.control_type == "readonly" and self.control_widget is not None:
            self.control_widget.setStyleSheet(f"color: {DarkTheme.SECONDARY_TEXT.name()};")
    
    def set_value(self, value: Any):
        """Set the control value."""
        self.current_value = value
        
        if self.control_type == "toggle" and isinstance(self.control_widget, ToggleSwitch):
            self.control_widget.setChecked(bool(value), animate=False)
        
        elif self.control_type == "dropdown" and isinstance(self.control_widget, QComboBox):
            # Find and set the item with matching data
            for i in range(self.control_widget.count()):
                if self.control_widget.itemData(i) == value:
                    self.control_widget.setCurrentIndex(i)
                    break
        
        elif self.control_type == "folder" and hasattr(self, 'folder_input'):
            self.folder_input.setText(str(value) if value else "")
        
        elif self.control_type == "text" and isinstance(self.control_widget, QLineEdit):
            self.control_widget.setText(str(value) if value else "")
        
        elif self.control_type == "readonly" and isinstance(self.control_widget, QLabel):
            self.control_widget.setText(str(value) if value else "")
    
    def get_value(self) -> Any:
        """Get the current control value."""
        return self.current_value
    
    def _emit_value_changed(self, value: Any):
        """Emit value changed signal."""
        self.current_value = value
        self.value_changed.emit(self.setting_name, value)
    
    def paintEvent(self, event):
        """Custom paint event to draw bottom border."""
        super().paintEvent(event)
        
        # Draw subtle bottom border (except for last item)
        from PySide6.QtGui import QPainter, QPen
        painter = QPainter(self)
        painter.setPen(QPen(DarkTheme.SUBTLE_BORDER, 1))
        
        rect = self.rect()
        painter.drawLine(0, rect.height() - 1, rect.width(), rect.height() - 1)