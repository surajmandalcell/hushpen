"""Base page class for consistent page structure."""

from PySide6.QtCore import Qt, Signal
from PySide6.QtGui import QFont
from PySide6.QtWidgets import QWidget, QVBoxLayout, QScrollArea, QLabel

from ..styles.dark_theme import DarkTheme, Fonts, Spacing


class BasePage(QWidget):
    """Base class for all settings pages."""
    
    setting_changed = Signal(str, object)  # setting_name, value
    
    def __init__(self, title: str, parent=None):
        super().__init__(parent)
        
        self.title = title
        self._setup_ui()
        self._apply_styles()
    
    def _setup_ui(self):
        """Set up the base page structure."""
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)
        
        # Create scroll area for content
        self.scroll_area = QScrollArea()
        self.scroll_area.setWidgetResizable(True)
        self.scroll_area.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self.scroll_area.setVerticalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAsNeeded)
        
        # Content widget inside scroll area
        self.content_widget = QWidget()
        self.content_layout = QVBoxLayout(self.content_widget)
        self.content_layout.setContentsMargins(Spacing.PAGE_PADDING, Spacing.PAGE_PADDING, 
                                               Spacing.PAGE_PADDING, Spacing.PAGE_PADDING)
        self.content_layout.setSpacing(Spacing.SECTION_GAP)
        
        # Page title
        self._create_title()
        
        # Add content widget to scroll area
        self.scroll_area.setWidget(self.content_widget)
        layout.addWidget(self.scroll_area)
    
    def _create_title(self):
        """Create the page title label."""
        self.title_label = QLabel(self.title)
        font = QFont()
        font.setPointSize(Fonts.PAGE_TITLE_SIZE)
        font.setWeight(Fonts.BOLD)
        self.title_label.setFont(font)
        self.title_label.setStyleSheet(f"color: {DarkTheme.PRIMARY_TEXT.name()};")
        self.content_layout.addWidget(self.title_label)
    
    def _apply_styles(self):
        """Apply base page styling."""
        self.setStyleSheet(f"""
            BasePage {{
                background-color: {DarkTheme.MAIN_BG.name()};
            }}
            QScrollArea {{
                border: none;
                background-color: {DarkTheme.MAIN_BG.name()};
            }}
            QScrollBar:vertical {{
                background-color: {DarkTheme.SIDEBAR_BG.name()};
                width: 12px;
                border-radius: 6px;
                margin: 0;
            }}
            QScrollBar::handle:vertical {{
                background-color: {DarkTheme.SECONDARY_TEXT.name()};
                border-radius: 6px;
                min-height: 20px;
                margin: 2px;
            }}
            QScrollBar::handle:vertical:hover {{
                background-color: {DarkTheme.PRIMARY_TEXT.name()};
            }}
            QScrollBar::add-line:vertical,
            QScrollBar::sub-line:vertical {{
                height: 0px;
            }}
        """)
    
    def add_content_widget(self, widget: QWidget):
        """Add a widget to the content layout."""
        self.content_layout.addWidget(widget)
    
    def add_stretch(self):
        """Add stretch to push content to top."""
        self.content_layout.addStretch()
    
    def _on_setting_changed(self, setting_name: str, value):
        """Handle setting changes and emit signal."""
        self.setting_changed.emit(setting_name, value)
        print(f"[{self.__class__.__name__}] Setting changed: {setting_name} = {value}")