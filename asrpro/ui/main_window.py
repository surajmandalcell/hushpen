"""Main native PyQt window implementation.

Implements pixel-snapped, antialiased rounded corners and a custom
soft shadow painted on a translucent background for a mac-like look.
"""

from PySide6.QtCore import Qt, QTimer
from PySide6.QtGui import QPainter, QBrush, QPainterPath, QPixmap, QColor, QPen
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

        # Use our own painter-based soft shadow instead of widget effect
    
    def _setup_ui(self):
        """Set up the main UI layout."""
        # Main horizontal layout
        main_layout = QHBoxLayout(self)
        # Reserve room for the painted shadow so content doesn't overlap
        m = Dimensions.WINDOW_SHADOW_MARGIN
        main_layout.setContentsMargins(m, m, m, m)
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
        """(Deprecated) We now paint a custom soft shadow in paintEvent."""
        return
    
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
        """Paint soft outer shadow and inner rounded background with AA."""
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing, True)
        painter.setRenderHint(QPainter.RenderHint.HighQualityAntialiasing, True)
        painter.setRenderHint(QPainter.RenderHint.SmoothPixmapTransform, True)

        r = Dimensions.WINDOW_RADIUS
        m = Dimensions.WINDOW_SHADOW_MARGIN
        outer_rect = self.rect()
        inner_rect = outer_rect.adjusted(m, m, -m, -m)

        # Soft shadow: layered expanded rounded-rect rings with fading alpha
        self._paint_soft_shadow(painter, inner_rect, r)

        # Fill inner rounded rectangle (pixel-snapped to reduce jaggies)
        path = QPainterPath()
        rr = inner_rect.adjusted(0.5, 0.5, -0.5, -0.5)
        path.addRoundedRect(rr, r, r)
        painter.fillPath(path, QBrush(DarkTheme.MAIN_BG))

        # Subtle inner border to visually reinforce rounded edge
        pen = QPen(QColor(255, 255, 255, 20))
        pen.setWidth(1)
        painter.setPen(pen)
        painter.drawRoundedRect(rr, r, r)

    def resizeEvent(self, event):
        """No mask — only repaint on resize for smooth edges."""
        super().resizeEvent(event)

    # Drag support for frameless window (mac-like behavior)
    def mousePressEvent(self, event):
        if event.button() == Qt.MouseButton.LeftButton:
            wh = self.windowHandle()
            if wh is not None:
                try:
                    # Qt 6 API on supported platforms
                    wh.startSystemMove()
                    event.accept()
                    return
                except Exception:
                    pass
            # Fallback manual drag
            self._drag_origin = event.globalPosition().toPoint()
            self._window_origin = self.frameGeometry().topLeft()
        super().mousePressEvent(event)

    def mouseMoveEvent(self, event):
        if event.buttons() & Qt.MouseButton.LeftButton:
            if hasattr(self, "_drag_origin") and hasattr(self, "_window_origin"):
                delta = event.globalPosition().toPoint() - self._drag_origin
                self.move(self._window_origin + delta)
                event.accept()
                return
        super().mouseMoveEvent(event)

    def mouseReleaseEvent(self, event):
        if hasattr(self, "_drag_origin"):
            delattr(self, "_drag_origin")
        if hasattr(self, "_window_origin"):
            delattr(self, "_window_origin")
        super().mouseReleaseEvent(event)

    def _paint_soft_shadow(self, painter: QPainter, inner_rect, radius: int):
        """Paints a soft, mac-like shadow around inner_rect.

        We layer a few expanded rounded-rect rings with decreasing alpha
        to approximate a blurred shadow without expensive offscreen blur.
        """
        # Shadow parameters (tuned for subtle, wide feather)
        spread = 18   # how far the shadow extends
        steps = 12    # number of gradient rings
        base_alpha = 120  # max alpha at the edge

        for i in range(steps):
            t = i / (steps - 1)
            # Ease-out curve for nicer falloff
            alpha = int(base_alpha * (1.0 - t) ** 2)
            if alpha <= 0:
                continue
            grow = int(spread * (t + 0.2))  # small offset to avoid harsh edge
            outer = inner_rect.adjusted(-grow, -grow, grow, grow)
            outer_r = radius + grow

            outer_path = QPainterPath()
            outer_path.addRoundedRect(outer, outer_r, outer_r)
            inner_path = QPainterPath()
            inner_path.addRoundedRect(inner_rect, radius, radius)
            ring = outer_path.subtracted(inner_path)

            color = QColor(0, 0, 0, alpha)
            painter.fillPath(ring, color)
    
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
