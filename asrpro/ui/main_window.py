"""Main native PyQt window implementation."""

from PySide6.QtCore import Qt, QTimer
from PySide6.QtGui import QPainter, QBrush, QPainterPath, QPixmap
from PySide6.QtWidgets import QWidget, QHBoxLayout, QApplication, QGraphicsDropShadowEffect

from .styles.dark_theme import DarkTheme, Dimensions
from .sidebar import SpokemlySidebar as Sidebar
from .content_area import ContentArea
from .utils.icon_loader import IconLoader

from .overlay import Overlay
from ..model_manager import ModelManager
from ..hotkey import ToggleHotkey


class NativeMainWindow(QWidget):
    """Pixel-perfect native PyQt recreation of the original HTML/CSS interface."""
    
    def __init__(self):
        super().__init__()
        
        # Core application components
        self.model_manager = ModelManager()
        self.hotkey = ToggleHotkey(self._on_hotkey_toggle)
        self.overlay = Overlay()
        
        self._setup_window()
        self._setup_ui()
        self._apply_theme()
        
        # Preload common icons for better performance
        IconLoader.preload_common_icons()
        
        # Start hotkey monitoring
        self.hotkey.start()
        
        print("[Native] Native PyQt main window initialized successfully")
    
    def _setup_window(self):
        """Configure the main window properties."""
        # Set window properties
        self.setWindowTitle("ASR Pro")
        self.setFixedSize(Dimensions.WINDOW_WIDTH, Dimensions.WINDOW_HEIGHT)
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint | Qt.WindowType.Window)
        
        # Enable translucent background for rounded corners
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        
        # Add drop shadow effect
        self._add_drop_shadow()
    
    def _setup_ui(self):
        """Set up the main UI layout."""
        # Main horizontal layout
        main_layout = QHBoxLayout(self)
        main_layout.setContentsMargins(0, 0, 0, 0)
        main_layout.setSpacing(0)
        
        # Sidebar
        self.sidebar = Sidebar(self)
        self.sidebar.page_requested.connect(self._on_page_requested)
        self.sidebar.window_action.connect(self._on_window_action)
        main_layout.addWidget(self.sidebar)
        
        # Content area
        self.content_area = ContentArea(self)
        main_layout.addWidget(self.content_area, 1)  # Take remaining space
    
    def _apply_theme(self):
        """Apply the dark theme palette and styling."""
        # Set application palette
        palette = DarkTheme.get_palette()
        self.setPalette(palette)
        
        # Set window-specific styling
        self.setStyleSheet(f"""
            NativeMainWindow {{
                background-color: transparent;
            }}
        """)
    
    def _add_drop_shadow(self):
        """Add macOS-style drop shadow effect."""
        shadow = QGraphicsDropShadowEffect()
        shadow.setBlurRadius(30)
        shadow.setOffset(0, 8)
        shadow.setColor(DarkTheme.MAIN_BG.darker(300))
        self.setGraphicsEffect(shadow)
    
    def _on_page_requested(self, section_id: str):
        """Handle page navigation requests from sidebar."""
        self.content_area.show_page(section_id)
        print(f"[Native] Navigated to section: {section_id}")
    
    def _on_window_action(self, action: str):
        """Handle window control actions."""
        if action == "close":
            self.close_app()
        elif action == "minimize":
            self.showMinimized()
        elif action == "hide":
            self.hide()
    
    def _on_hotkey_toggle(self, recording: bool):
        """Handle hotkey toggle events."""
        print(f"[Native] Hotkey toggle: recording={recording}")
        try:
            if recording:
                self.overlay.show_smooth()
            else:
                self.overlay.close_smooth()
        except Exception as e:
            print(f"[Native] Error toggling overlay: {e}")
    
    def paintEvent(self, event):
        """Custom paint event for rounded corners and background."""
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        
        # Create rounded rectangle path
        path = QPainterPath()
        path.addRoundedRect(self.rect(), Dimensions.WINDOW_RADIUS, Dimensions.WINDOW_RADIUS)
        
        # Fill with main background color
        painter.fillPath(path, QBrush(DarkTheme.MAIN_BG))
    
    def closeEvent(self, event):
        """Override close event to hide to tray instead of closing."""
        event.ignore()
        self.hide()
        print("[Native] Window hidden to tray")
    
    def showEvent(self, event):
        """Override show event for proper display."""
        super().showEvent(event)
        print("[Native] Window shown")
    
    def close_app(self):
        """Clean shutdown of the application."""
        try:
            print("[Native] Shutting down application...")
            
            # Cleanup model manager
            self.model_manager.unload()
            
            # Cleanup hotkey system
            if hasattr(self, 'hotkey'):
                self.hotkey = None
            
            # Close application
            app = QApplication.instance()
            if app is not None:
                app.quit()
            
        except Exception as e:
            print(f"[Native] Error during cleanup: {e}")
            app = QApplication.instance()
            if app is not None:
                app.quit()
    
    def apply_hotkey_change(self, hk: str):
        """Apply a new hotkey configuration."""
        try:
            if self.hotkey is not None:
                self.hotkey.set_hotkey(hk)
            print(f"[Native] Updated hotkey to: {hk}")
        except Exception as e:
            print(f"[Native] Failed to update hotkey: {e}")
    
    def set_tray_icon(self, tray_icon):
        """Set the system tray icon reference."""
        self.tray_icon = tray_icon