"""Sidebar styled like Spokenly app with proper sections and spacing."""

from typing import List, Dict, Any
from PySide6.QtCore import Qt, Signal
from PySide6.QtGui import QFont, QPainter, QPen, QIcon, QPixmap, QBrush, QColor
from PySide6.QtCore import QRect
from PySide6.QtWidgets import QWidget, QVBoxLayout, QHBoxLayout, QLabel, QPushButton

from asrpro.ui.utils.elements import subtle_separator

from .styles.dark_theme import DarkTheme, Dimensions, Fonts, Spacing
from .utils.icon_loader import IconLoader
from .traffic_lights import TrafficLights


class DragHeader(QWidget):
    """Header area that supports window dragging while hosting traffic lights."""

    window_action = Signal(str)

    def __init__(self, parent=None):
        super().__init__(parent)
        self._drag_origin = None
        self._window_origin = None
        self._setup_ui()

    def _setup_ui(self):
        layout = QHBoxLayout(self)
        layout.setContentsMargins(0, 4, 0, 0)
        layout.setSpacing(0)
        self.setFixedHeight(32)

        self.traffic_lights = TrafficLights(self)
        self.traffic_lights.close_clicked.connect(
            lambda: self.window_action.emit("close")
        )
        self.traffic_lights.minimize_clicked.connect(
            lambda: self.window_action.emit("minimize")
        )
        self.traffic_lights.hide_clicked.connect(
            lambda: self.window_action.emit("hide")
        )
        layout.addWidget(self.traffic_lights)

        # Expandable spacer to occupy the rest of the header; acts as drag area
        spacer = QWidget(self)
        spacer.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents, False)
        layout.addWidget(spacer, 1)

    def _over_controls(self, pos) -> bool:
        # Prevent drag when clicking on the traffic light buttons area
        return self.traffic_lights.geometry().contains(pos)

    def mousePressEvent(self, event):
        if event.button() == Qt.MouseButton.LeftButton and not self._over_controls(
            event.pos()
        ):
            wh = self.window().windowHandle() if self.window() else None
            if wh is not None:
                try:
                    wh.startSystemMove()
                    event.accept()
                    return
                except Exception:
                    pass
            # Fallback manual drag
            self._drag_origin = event.globalPosition().toPoint()
            self._window_origin = (
                self.window().frameGeometry().topLeft() if self.window() else None
            )
            event.accept()
            return
        super().mousePressEvent(event)

    def mouseMoveEvent(self, event):
        if event.buttons() & Qt.MouseButton.LeftButton:
            if self._drag_origin is not None and self._window_origin is not None:
                delta = event.globalPosition().toPoint() - self._drag_origin
                if self.window():
                    self.window().move(self._window_origin + delta)
                event.accept()
                return
        super().mouseMoveEvent(event)

    def mouseReleaseEvent(self, event):
        self._drag_origin = None
        self._window_origin = None
        super().mouseReleaseEvent(event)


class SectionHeader(QWidget):
    """Section header widget like in Spokenly app."""

    def __init__(self, title: str, parent=None):
        super().__init__(parent)
        self.title = title
        self._setup_ui()

    def _setup_ui(self):
        """Set up the section header layout."""
        layout = QHBoxLayout(self)
        layout.setContentsMargins(20, 16, 20, 8)
        layout.setSpacing(0)

        self.title_label = QLabel(self.title.upper())
        font = QFont()
        font.setPointSize(10)
        font.setWeight(Fonts.MEDIUM)
        font.setLetterSpacing(QFont.SpacingType.AbsoluteSpacing, 0.5)
        self.title_label.setFont(font)
        self.title_label.setStyleSheet(
            f"color: {DarkTheme.SECONDARY_TEXT.darker(150).name()};"
        )

        layout.addWidget(self.title_label)
        layout.addStretch()


class SpokenlyNavigationItem(QWidget):
    """Navigation item styled like Spokenly app."""

    clicked = Signal(str)

    def __init__(self, section_id: str, icon_name: str, text: str, parent=None):
        super().__init__(parent)

        self.section_id = section_id
        self.icon_name = icon_name
        self.text = text
        self.is_active = False
        self.is_hovered = False

        self._setup_ui()
        self._apply_styles()

        self.setMouseTracking(True)

    def _setup_ui(self):
        """Set up the navigation item layout."""
        layout = QHBoxLayout(self)
        layout.setContentsMargins(20, 0, 20, 0)
        layout.setSpacing(12)

        self.icon_label = QLabel()
        self.icon_label.setFixedSize(14, 14)
        self.icon_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.icon_label.setContentsMargins(0, 2, 0, 0)
        layout.addWidget(self.icon_label)

        self.text_label = QLabel(self.text)
        font = QFont()
        font.setPointSize(11)
        font.setWeight(Fonts.NORMAL)
        self.text_label.setFont(font)
        layout.addWidget(self.text_label)

        layout.addStretch()

        self._update_appearance()

    def _apply_styles(self):
        """Apply base styles."""
        self.setFixedHeight(32)
        self.setCursor(Qt.CursorShape.PointingHandCursor)

    def set_active(self, active: bool):
        """Set the active state of this navigation item."""
        if self.is_active != active:
            self.is_active = active
            if active:
                # When active, ensure hover visuals don't conflict
                self.is_hovered = False
            self._update_appearance()

    def set_hover(self, hovered: bool):
        """Explicitly set hover state and refresh appearance."""
        if self.is_hovered != hovered:
            self.is_hovered = hovered
            self._update_appearance()

    def _update_appearance(self):
        """Update colors and icon based on current state."""
        if self.is_active:
            text_color = DarkTheme.ACCENT_BLUE.name()
            icon_color = DarkTheme.ACCENT_BLUE.name()
            has_left_border = True
        elif self.is_hovered:
            text_color = DarkTheme.SECONDARY_TEXT.name()
            icon_color = DarkTheme.SECONDARY_TEXT.name()
            bg_color = None
            has_left_border = False
        else:
            text_color = DarkTheme.SECONDARY_TEXT.darker(160).name()
            icon_color = DarkTheme.SECONDARY_TEXT.darker(160).name()
            bg_color = None
            has_left_border = False

        self.text_label.setStyleSheet(f"color: {text_color};")

        icon = IconLoader.load_icon(self.icon_name, 14, icon_color)
        self.icon_label.setPixmap(icon.pixmap(14, 14))

        if self.is_active:
            r, g, b = (
                DarkTheme.ACCENT_BLUE.red(),
                DarkTheme.ACCENT_BLUE.green(),
                DarkTheme.ACCENT_BLUE.blue(),
            )
            self.setStyleSheet(
                """
                SpokenlyNavigationItem {
                    background-color: rgba(255,0,0, 60);
                    border-radius: 6px;
                    margin: 2px 8px;
                    border-left: 20px solid rgba(%d, %d, %d, 153);
                    padding-left: 16px;
                }
                """
                % (r, g, b)
            )
        else:
            self.setStyleSheet(
                """
                SpokenlyNavigationItem { 
                    background-color: transparent; 
                    margin: 2px 8px;
                }
            """
            )

        self.update()

    def mousePressEvent(self, event):
        """Handle mouse press events."""
        if event.button() == Qt.MouseButton.LeftButton:
            self.clicked.emit(self.section_id)
        super().mousePressEvent(event)

    def enterEvent(self, event):
        """Handle mouse enter events."""
        self.set_hover(True)
        super().enterEvent(event)

    def leaveEvent(self, event):
        """Handle mouse leave events."""
        self.set_hover(False)
        super().leaveEvent(event)


class SpokenlyLogoSection(QWidget):
    """App logo section styled like Spokenly."""

    def __init__(self, parent=None):
        super().__init__(parent)
        self._setup_ui()

    def _setup_ui(self):
        """Set up the logo section."""
        layout = QHBoxLayout(self)
        layout.setContentsMargins(20, 16, 20, 16)
        layout.setSpacing(6)

        self.name_label = QLabel("ASR Pro")
        font = QFont()
        font.setPointSize(12)
        font.setWeight(Fonts.SEMIBOLD)
        self.name_label.setFont(font)
        r, g, b = (
            DarkTheme.PRIMARY_TEXT.red(),
            DarkTheme.PRIMARY_TEXT.green(),
            DarkTheme.PRIMARY_TEXT.blue(),
        )
        self.name_label.setStyleSheet(f"color: rgba({r}, {g}, {b}, 204);")

        layout.addWidget(self.name_label)
        layout.addStretch()


class SpokemlySidebar(QWidget):
    """Sidebar styled exactly like Spokenly app."""

    page_requested = Signal(str)
    window_action = Signal(str)

    def __init__(self, parent=None):
        super().__init__(parent)

        self.navigation_items: List[SpokenlyNavigationItem] = []
        self.current_section = "general"

        self._setup_ui()
        self._apply_styles()

    def _setup_ui(self):
        """Set up the sidebar layout."""
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)

        header_widget = DragHeader()
        header_widget.window_action.connect(self.window_action.emit)
        layout.addWidget(header_widget)

        self.logo_section = SpokenlyLogoSection()
        layout.addWidget(self.logo_section)

        nav_container = QWidget()
        nav_layout = QVBoxLayout(nav_container)
        nav_layout.setContentsMargins(0, 0, 0, 0)
        nav_layout.setSpacing(2)

        nav_items_data = [
            ("general", "settings", "General Settings"),
            ("transcribe", "folder-open", "Transcribe File"),
            ("models", "cpu", "Dictation Models"),
            ("microphone", "mic", "Microphone"),
            ("keyboard", "keyboard", "Keyboard Controls"),
        ]

        for section_id, icon_name, text in nav_items_data:
            item = SpokenlyNavigationItem(section_id, icon_name, text)
            item.clicked.connect(self._on_navigation_clicked)
            self.navigation_items.append(item)
            nav_layout.addWidget(item)

        layout.addWidget(nav_container)

        layout.addStretch()

        subtle_separator(layout=layout)

        footer_container = QWidget()
        footer_layout = QVBoxLayout(footer_container)
        footer_layout.setContentsMargins(0, 8, 0, 16)
        footer_layout.setSpacing(2)

        self.about_item = SpokenlyNavigationItem("about", "info", "About")
        self.about_item.clicked.connect(self._on_navigation_clicked)
        footer_layout.addWidget(self.about_item)

        self.exit_item = SpokenlyNavigationItem("exit", "power", "Exit")
        self.exit_item.clicked.connect(lambda: self.window_action.emit("close"))
        footer_layout.addWidget(self.exit_item)

        layout.addWidget(footer_container)

        self._update_active_item("general")

    def _apply_styles(self):
        """Apply sidebar styling."""
        self.setFixedWidth(240)
        self.setStyleSheet("SpokemlySidebar { background-color: transparent; }")

    def _on_navigation_clicked(self, section_id: str):
        """Handle navigation item clicks."""
        if section_id != "exit":
            # Reset all hover states to prevent sticky hover after click
            for item in self.navigation_items:
                item.set_hover(False)
            self.about_item.set_hover(False)
            self.set_active_section(section_id)
            self.page_requested.emit(section_id)

    def set_active_section(self, section_id: str):
        """Set the active navigation section."""
        if self.current_section != section_id:
            self.current_section = section_id
            self._update_active_item(section_id)

    def _update_active_item(self, section_id: str):
        """Update visual active state of navigation items."""
        # Clear all hovers to avoid stuck hover state after clicks
        for item in self.navigation_items:
            item.set_hover(False)
            item.set_active(item.section_id == section_id)

        # Footer items
        self.about_item.set_hover(False)
        self.about_item.set_active(section_id == "about")

    def paintEvent(self, event):
        """Custom paint event for sidebar background (opaque)."""
        painter = QPainter(self)
        try:
            painter.setRenderHint(QPainter.RenderHint.Antialiasing, True)
            painter.fillRect(self.rect(), DarkTheme.SIDEBAR_BG)
        finally:
            painter.end()
