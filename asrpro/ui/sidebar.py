"""Completely remade sidebar widget with proper compact spacing."""

from typing import List, Dict, Any
from PySide6.QtCore import Qt, Signal
from PySide6.QtGui import QFont, QPainter, QPen, QIcon, QPixmap, QBrush
from PySide6.QtWidgets import QWidget, QVBoxLayout, QHBoxLayout, QLabel, QPushButton

from .styles.dark_theme import DarkTheme, Dimensions, Fonts, Spacing
from .utils.icon_loader import IconLoader
from .traffic_lights import TrafficLights


class CompactNavigationItem(QWidget):
    """Compact navigation item with proper spacing."""
    
    clicked = Signal(str)  # section_id
    
    def __init__(self, section_id: str, icon_name: str, text: str, parent=None):
        super().__init__(parent)
        
        self.section_id = section_id
        self.icon_name = icon_name
        self.text = text
        self.is_active = False
        self.is_hovered = False
        
        self._setup_ui()
        self._apply_styles()
        
        # Enable mouse tracking for hover effects
        self.setMouseTracking(True)
    
    def _setup_ui(self):
        """Set up the navigation item layout."""
        layout = QHBoxLayout(self)
        layout.setContentsMargins(12, 6, 12, 6)  # Compact padding
        layout.setSpacing(8)  # Small gap between icon and text
        
        # Icon label
        self.icon_label = QLabel()
        self.icon_label.setFixedSize(16, 16)
        self.icon_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        layout.addWidget(self.icon_label)
        
        # Text label
        self.text_label = QLabel(self.text)
        font = QFont()
        font.setPointSize(13)
        font.setWeight(Fonts.NORMAL)
        self.text_label.setFont(font)
        layout.addWidget(self.text_label)
        
        layout.addStretch()  # Push content to the left
        
        self._update_appearance()
    
    def _apply_styles(self):
        """Apply base styles."""
        self.setFixedHeight(28)  # Compact height
        self.setCursor(Qt.CursorShape.PointingHandCursor)
    
    def set_active(self, active: bool):
        """Set the active state of this navigation item."""
        if self.is_active != active:
            self.is_active = active
            self._update_appearance()
    
    def _update_appearance(self):
        """Update colors and icon based on current state."""
        if self.is_active:
            text_color = DarkTheme.ACCENT_BLUE.name()
            icon_color = DarkTheme.ACCENT_BLUE.name()
            bg_color = DarkTheme.ACCENT_BLUE_BG
        elif self.is_hovered:
            text_color = DarkTheme.PRIMARY_TEXT.name()
            icon_color = DarkTheme.PRIMARY_TEXT.name()
            bg_color = DarkTheme.HOVER_WHITE_BG
        else:
            text_color = DarkTheme.SECONDARY_TEXT.name()
            icon_color = DarkTheme.SECONDARY_TEXT.name()
            bg_color = None
        
        # Update text color
        self.text_label.setStyleSheet(f"color: {text_color};")
        
        # Update icon with color tinting
        icon = IconLoader.load_icon(self.icon_name, 16, icon_color)
        self.icon_label.setPixmap(icon.pixmap(16, 16))
        
        # Update background
        if bg_color:
            self.setStyleSheet(f"CompactNavigationItem {{ background-color: rgba({bg_color.red()}, {bg_color.green()}, {bg_color.blue()}, {bg_color.alpha()}); border-radius: 4px; }}")
        else:
            self.setStyleSheet("CompactNavigationItem { background-color: transparent; }")
        
        self.update()
    
    def mousePressEvent(self, event):
        """Handle mouse press events."""
        if event.button() == Qt.MouseButton.LeftButton:
            self.clicked.emit(self.section_id)
        super().mousePressEvent(event)
    
    def enterEvent(self, event):
        """Handle mouse enter events."""
        self.is_hovered = True
        self._update_appearance()
        super().enterEvent(event)
    
    def leaveEvent(self, event):
        """Handle mouse leave events."""
        self.is_hovered = False
        self._update_appearance()
        super().leaveEvent(event)


class CompactLogoSection(QWidget):
    """Compact app logo and branding section."""
    
    def __init__(self, parent=None):
        super().__init__(parent)
        self._setup_ui()
    
    def _setup_ui(self):
        """Set up the logo section."""
        layout = QHBoxLayout(self)
        layout.setContentsMargins(12, 12, 12, 8)  # Compact padding
        layout.setSpacing(6)  # Small gap
        
        # App icon (PNG with white filter)
        self.icon_label = QLabel()
        self.icon_label.setFixedSize(18, 18)  # Slightly larger for visibility
        self.icon_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        
        # Load app icon (PNG) with white color filter
        from pathlib import Path
        from PySide6.QtGui import QPixmap, QPainter, QBrush, QColor
        
        icon_path = Path(__file__).parent.parent.parent / "assets" / "icon.png"
        if icon_path.exists():
            # Load original pixmap
            original_pixmap = QPixmap(str(icon_path))
            if not original_pixmap.isNull():
                # Create white-tinted version
                white_pixmap = QPixmap(original_pixmap.size())
                white_pixmap.fill(Qt.GlobalColor.transparent)
                
                painter = QPainter(white_pixmap)
                painter.setRenderHint(QPainter.RenderHint.Antialiasing)
                
                # Draw original pixmap
                painter.drawPixmap(0, 0, original_pixmap)
                
                # Apply white overlay with SourceIn composition mode
                painter.setCompositionMode(QPainter.CompositionMode.CompositionMode_SourceIn)
                painter.fillRect(white_pixmap.rect(), QBrush(DarkTheme.SECONDARY_TEXT))
                
                painter.end()
                
                # Scale to desired size
                scaled_pixmap = white_pixmap.scaled(
                    18, 18,
                    Qt.AspectRatioMode.KeepAspectRatio,
                    Qt.TransformationMode.SmoothTransformation
                )
                self.icon_label.setPixmap(scaled_pixmap)
            else:
                # Fallback if pixmap couldn't be loaded
                icon = IconLoader.load_icon("mic", 18, DarkTheme.SECONDARY_TEXT.name())
                self.icon_label.setPixmap(icon.pixmap(18, 18))
        else:
            # Fallback to mic icon
            icon = IconLoader.load_icon("mic", 18, DarkTheme.SECONDARY_TEXT.name())
            self.icon_label.setPixmap(icon.pixmap(18, 18))
        
        layout.addWidget(self.icon_label)
        
        # App name
        self.name_label = QLabel("ASR Pro")
        font = QFont()
        font.setPointSize(13)
        font.setWeight(Fonts.MEDIUM)
        self.name_label.setFont(font)
        self.name_label.setStyleSheet(f"color: {DarkTheme.SECONDARY_TEXT.name()};")
        
        layout.addWidget(self.name_label)
        layout.addStretch()


class CompactSidebar(QWidget):
    """Completely remade compact sidebar widget."""
    
    page_requested = Signal(str)  # section_id
    window_action = Signal(str)   # "close", "minimize", "hide"
    
    def __init__(self, parent=None):
        super().__init__(parent)
        
        self.navigation_items: List[CompactNavigationItem] = []
        self.current_section = "general"  # Default active section
        
        self._setup_ui()
        self._apply_styles()
    
    def _setup_ui(self):
        """Set up the sidebar layout."""
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)
        
        # Header with traffic lights
        header_widget = QWidget()
        header_widget.setFixedHeight(28)  # Compact header
        header_layout = QHBoxLayout(header_widget)
        header_layout.setContentsMargins(0, 0, 0, 0)
        
        self.traffic_lights = TrafficLights()
        self.traffic_lights.close_clicked.connect(lambda: self.window_action.emit("close"))
        self.traffic_lights.minimize_clicked.connect(lambda: self.window_action.emit("minimize"))
        self.traffic_lights.hide_clicked.connect(lambda: self.window_action.emit("hide"))
        
        header_layout.addWidget(self.traffic_lights)
        layout.addWidget(header_widget)
        
        # Logo section
        self.logo_section = CompactLogoSection()
        layout.addWidget(self.logo_section)
        
        # Main navigation (compact)
        nav_container = QWidget()
        nav_layout = QVBoxLayout(nav_container)
        nav_layout.setContentsMargins(0, 4, 0, 4)  # Minimal padding
        nav_layout.setSpacing(1)  # Very small gap between items
        
        # Create navigation items
        nav_items_data = [
            ("general", "settings", "General"),
            ("models", "cpu", "Dictation Engine"),
            ("keyboard", "keyboard", "Keyboard"),
            ("microphone", "mic", "Microphone"),
            ("about", "info", "About"),
        ]
        
        for section_id, icon_name, text in nav_items_data:
            item = CompactNavigationItem(section_id, icon_name, text)
            item.clicked.connect(self._on_navigation_clicked)
            self.navigation_items.append(item)
            nav_layout.addWidget(item)
        
        layout.addWidget(nav_container)
        
        # Add stretch to push footer to bottom
        layout.addStretch()
        
        # Footer section (compact)
        footer_container = QWidget()
        footer_layout = QVBoxLayout(footer_container)
        footer_layout.setContentsMargins(0, 4, 0, 8)  # Small padding
        footer_layout.setSpacing(1)  # Small gap
        
        # Exit item
        self.exit_item = CompactNavigationItem("exit", "power", "Exit")
        self.exit_item.clicked.connect(lambda: self.window_action.emit("close"))
        footer_layout.addWidget(self.exit_item)
        
        layout.addWidget(footer_container)
        
        # Set initial active item
        self._update_active_item("general")
    
    def _apply_styles(self):
        """Apply sidebar styling."""
        self.setFixedWidth(240)
        # Use transparent background to allow custom paint event
        self.setStyleSheet("CompactSidebar { background-color: transparent; }")
    
    def _on_navigation_clicked(self, section_id: str):
        """Handle navigation item clicks."""
        if section_id != "exit":
            self.set_active_section(section_id)
            self.page_requested.emit(section_id)
    
    def set_active_section(self, section_id: str):
        """Set the active navigation section."""
        if self.current_section != section_id:
            self.current_section = section_id
            self._update_active_item(section_id)
    
    def _update_active_item(self, section_id: str):
        """Update visual active state of navigation items."""
        # Update main navigation items
        for item in self.navigation_items:
            item.set_active(item.section_id == section_id)
    
    def paintEvent(self, event):
        """Custom paint event for translucent Mac-like background."""
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        
        # Fill with translucent background
        painter.fillRect(self.rect(), DarkTheme.SIDEBAR_BG)