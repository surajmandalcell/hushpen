"""Sidebar widget with navigation and branding."""

from typing import List, Dict, Any
from PySide6.QtCore import Qt, Signal
from PySide6.QtGui import QFont, QPainter, QPen, QIcon
from PySide6.QtWidgets import QWidget, QVBoxLayout, QHBoxLayout, QLabel, QPushButton

from .styles.dark_theme import DarkTheme, Dimensions, Fonts, Spacing
from .utils.icon_loader import IconLoader
from .traffic_lights import TrafficLights


class NavigationItem(QWidget):
    """Individual navigation item with icon and text."""
    
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
        layout.setContentsMargins(Spacing.ITEM_PADDING_H, 
                                  Spacing.ITEM_PADDING_V,
                                  Spacing.ITEM_PADDING_H, 
                                  Spacing.ITEM_PADDING_V)
        layout.setSpacing(Spacing.ICON_TEXT_GAP)
        
        # Icon label
        self.icon_label = QLabel()
        self.icon_label.setFixedSize(Dimensions.NAV_ITEM_ICON_SIZE, Dimensions.NAV_ITEM_ICON_SIZE)
        self.icon_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        layout.addWidget(self.icon_label)
        
        # Text label
        self.text_label = QLabel(self.text)
        font = QFont()
        font.setPointSize(Fonts.BASE_SIZE)
        font.setWeight(Fonts.NORMAL)
        self.text_label.setFont(font)
        layout.addWidget(self.text_label)
        
        layout.addStretch()  # Push content to the left
        
        self._update_appearance()
    
    def _apply_styles(self):
        """Apply base styles."""
        self.setFixedHeight(Spacing.ITEM_PADDING_V * 2 + Dimensions.NAV_ITEM_ICON_SIZE + 4)
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
        icon = IconLoader.load_icon(self.icon_name, Dimensions.NAV_ITEM_ICON_SIZE, icon_color)
        self.icon_label.setPixmap(icon.pixmap(Dimensions.NAV_ITEM_ICON_SIZE, Dimensions.NAV_ITEM_ICON_SIZE))
        
        # Update background
        if bg_color:
            self.setStyleSheet(f"NavigationItem {{ background-color: rgba({bg_color.red()}, {bg_color.green()}, {bg_color.blue()}, {bg_color.alpha()}); }}")
        else:
            self.setStyleSheet("")
        
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


class LogoSection(QWidget):
    """App logo and branding section."""
    
    def __init__(self, parent=None):
        super().__init__(parent)
        self._setup_ui()
    
    def _setup_ui(self):
        """Set up the logo section."""
        layout = QHBoxLayout(self)
        layout.setContentsMargins(Dimensions.LOGO_PADDING, 
                                  Dimensions.LOGO_PADDING, 
                                  Dimensions.LOGO_PADDING, 
                                  Dimensions.LOGO_PADDING)
        layout.setSpacing(Dimensions.LOGO_GAP)
        
        # App icon (microphone SVG)
        self.icon_label = QLabel()
        self.icon_label.setFixedSize(Dimensions.LOGO_ICON_SIZE, Dimensions.LOGO_ICON_SIZE)
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
                    Dimensions.LOGO_ICON_SIZE, 
                    Dimensions.LOGO_ICON_SIZE,
                    Qt.AspectRatioMode.KeepAspectRatio,
                    Qt.TransformationMode.SmoothTransformation
                )
                self.icon_label.setPixmap(scaled_pixmap)
            else:
                # Fallback if pixmap couldn't be loaded
                icon = IconLoader.load_icon("mic", Dimensions.LOGO_ICON_SIZE, DarkTheme.SECONDARY_TEXT.name())
                self.icon_label.setPixmap(icon.pixmap(Dimensions.LOGO_ICON_SIZE, Dimensions.LOGO_ICON_SIZE))
        else:
            # Fallback to mic icon
            icon = IconLoader.load_icon("mic", Dimensions.LOGO_ICON_SIZE, DarkTheme.SECONDARY_TEXT.name())
            self.icon_label.setPixmap(icon.pixmap(Dimensions.LOGO_ICON_SIZE, Dimensions.LOGO_ICON_SIZE))
        
        layout.addWidget(self.icon_label)
        
        # App name
        self.name_label = QLabel("ASR Pro")
        font = QFont()
        font.setPointSize(Fonts.BASE_SIZE)
        font.setWeight(Fonts.NORMAL)
        self.name_label.setFont(font)
        self.name_label.setStyleSheet(f"color: {DarkTheme.SECONDARY_TEXT.name()};")
        
        layout.addWidget(self.name_label)
        layout.addStretch()


class Sidebar(QWidget):
    """Main sidebar widget with navigation."""
    
    page_requested = Signal(str)  # section_id
    window_action = Signal(str)   # "close", "minimize", "hide"
    
    def __init__(self, parent=None):
        super().__init__(parent)
        
        self.navigation_items: List[NavigationItem] = []
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
        header_widget.setFixedHeight(Dimensions.SIDEBAR_HEADER_HEIGHT)
        header_layout = QHBoxLayout(header_widget)
        header_layout.setContentsMargins(0, 0, 0, 0)
        
        self.traffic_lights = TrafficLights()
        self.traffic_lights.close_clicked.connect(lambda: self.window_action.emit("close"))
        self.traffic_lights.minimize_clicked.connect(lambda: self.window_action.emit("minimize"))
        self.traffic_lights.hide_clicked.connect(lambda: self.window_action.emit("hide"))
        
        header_layout.addWidget(self.traffic_lights)
        layout.addWidget(header_widget)
        
        # Logo section
        self.logo_section = LogoSection()
        layout.addWidget(self.logo_section)
        
        # Navigation list
        nav_container = QWidget()
        nav_layout = QVBoxLayout(nav_container)
        nav_layout.setContentsMargins(0, Dimensions.SIDEBAR_PADDING_V, 0, 0)
        nav_layout.setSpacing(2)  # Small gap between items
        
        # Create navigation items
        nav_items_data = [
            ("general", "settings", "General"),
            ("models", "cpu", "Dictation Engine"),
            ("keyboard", "keyboard", "Keyboard"),
            ("microphone", "mic", "Microphone"),
            ("about", "info", "About"),
        ]
        
        for section_id, icon_name, text in nav_items_data:
            item = NavigationItem(section_id, icon_name, text)
            item.clicked.connect(self._on_navigation_clicked)
            self.navigation_items.append(item)
            nav_layout.addWidget(item)
        
        layout.addWidget(nav_container, 1)  # Take remaining space
        
        # Footer section
        footer_container = QWidget()
        footer_layout = QVBoxLayout(footer_container)
        footer_layout.setContentsMargins(0, Dimensions.SIDEBAR_PADDING_V, 0, Dimensions.SIDEBAR_PADDING_V)
        footer_layout.setSpacing(2)  # Small gap between footer items
        
        # About item (duplicate in footer)
        self.about_footer = NavigationItem("about", "info", "About ASR Pro")
        self.about_footer.clicked.connect(self._on_navigation_clicked)
        footer_layout.addWidget(self.about_footer)
        
        # Exit item
        self.exit_item = NavigationItem("exit", "power", "Exit")
        self.exit_item.clicked.connect(lambda: self.window_action.emit("close"))
        footer_layout.addWidget(self.exit_item)
        
        layout.addWidget(footer_container)
        
        # Set initial active item
        self._update_active_item("general")
    
    def _apply_styles(self):
        """Apply sidebar styling."""
        self.setFixedWidth(Dimensions.SIDEBAR_WIDTH)
        # Use transparent background to allow custom paint event
        self.setStyleSheet(f"""
            Sidebar {{
                background-color: transparent;
            }}
        """)
    
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
        
        # Update footer about item
        self.about_footer.set_active(section_id == "about")
    
    def paintEvent(self, event):
        """Custom paint event for translucent Mac-like background."""
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        
        # Fill with translucent background
        painter.fillRect(self.rect(), DarkTheme.SIDEBAR_BG)