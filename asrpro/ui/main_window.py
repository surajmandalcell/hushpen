"""Main native PyQt window implementation.

Implements pixel-snapped, antialiased rounded corners and a custom
soft shadow painted on a translucent background for a mac-like look.
"""

from PySide6.QtCore import Qt, QTimer, QRectF
from PySide6.QtGui import QPainter, QBrush, QPainterPath, QPixmap, QColor, QPen
from PySide6.QtWidgets import (
    QWidget,
    QHBoxLayout,
    QApplication,
    QGraphicsDropShadowEffect,
)

from .styles.dark_theme import DarkTheme, Dimensions
from .layout.window import build_root_and_frame, compute_shadow_margins, apply_rounded_clip
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
        # Build root + inner frame via helpers
        root_layout, self._frame, main_layout = build_root_and_frame(self)

        # Sidebar
        self.sidebar = Sidebar(self)
        self.sidebar.page_requested.connect(self._on_page_requested)
        self.sidebar.window_action.connect(self._on_window_action)
        main_layout.addWidget(self.sidebar)

        # Content area
        self.content_area = ContentArea(self)
        main_layout.addWidget(self.content_area, 1)  # Take remaining space

        # Root already holds frame; clip applied by helper

    def _apply_theme(self):
        """Apply the dark theme palette and styling."""
        # Set application palette
        palette = DarkTheme.get_palette()
        self.setPalette(palette)

        # Set window-specific styling
        self.setStyleSheet(
            f"""
            NativeMainWindow {{
                background-color: transparent;
            }}
        """
        )

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
        try:
            painter.setRenderHint(QPainter.RenderHint.Antialiasing, True)
            painter.setRenderHint(QPainter.RenderHint.SmoothPixmapTransform, True)

            r = Dimensions.WINDOW_RADIUS
            outer_rect = self.rect()
            left, top, right, bottom = compute_shadow_margins()
            inner_rect = outer_rect.adjusted(left, top, -right, -bottom)

            # Soft shadow: layered expanded rounded-rect rings with fading alpha
            self._paint_soft_shadow(painter, inner_rect, r)

            # Fill inner rounded rectangle (pixel-snapped to reduce jaggies)
            path = QPainterPath()
            rr = QRectF(inner_rect).adjusted(0.5, 0.5, -0.5, -0.5)
            path.addRoundedRect(rr, r, r)
            painter.fillPath(path, QBrush(DarkTheme.MAIN_BG))

            # Subtle inner border to visually reinforce rounded edge
            pen = QPen(QColor(255, 255, 255, 20))
            pen.setWidth(1)
            painter.setPen(pen)
            painter.drawRoundedRect(rr, r, r)
        finally:
            painter.end()

    def resizeEvent(self, event):
        """No mask — only repaint on resize for smooth edges."""
        super().resizeEvent(event)
        apply_rounded_clip(self._frame, Dimensions.WINDOW_RADIUS)

    def _update_frame_mask(self):
        if hasattr(self, "_frame") and self._frame is not None:
            apply_rounded_clip(self._frame, Dimensions.WINDOW_RADIUS)

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
        """Paint a CSS-like shadow: 0 22px 70px 4px rgba(0,0,0,0.56).

        Approximated with layered rounded-rect rings.
        """
        offx = Dimensions.SHADOW_OFFSET_X
        offy = Dimensions.SHADOW_OFFSET_Y
        blur = Dimensions.SHADOW_BLUR
        spread = Dimensions.SHADOW_SPREAD
        target_alpha = int(255 * Dimensions.SHADOW_ALPHA)

        steps = 28

        base_inner = inner_rect.adjusted(-spread, -spread, spread, spread)
        base_inner = base_inner.translated(offx, offy)

        for i in range(steps):
            t = i / (steps - 1)
            grow = int(spread + t * blur)
            # Eased falloff; divide to avoid oversaturation across layers
            alpha = int(target_alpha * (1.0 - t) ** 2 / 2.0)
            if alpha <= 0:
                continue

            outer = inner_rect.adjusted(-grow, -grow, grow, grow).translated(offx, offy)
            outer_r = radius + grow

            outer_path = QPainterPath()
            outer_path.addRoundedRect(outer, outer_r, outer_r)

            inner_path = QPainterPath()
            inner_path.addRoundedRect(base_inner, radius + spread, radius + spread)

            ring = outer_path.subtracted(inner_path)
            painter.fillPath(ring, QColor(0, 0, 0, alpha))

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
            if hasattr(self, "hotkey"):
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
