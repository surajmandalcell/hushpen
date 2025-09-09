"""System tray builder."""

from __future__ import annotations
from PySide6.QtGui import QIcon, QPixmap, QPainter
from PySide6.QtCore import Qt
from pathlib import Path
from PySide6.QtWidgets import (
    QSystemTrayIcon,
    QMenu,
    QFileDialog,
    QInputDialog,
    QApplication,
)
from ..hotkey import load_hotkey, save_hotkey


def is_dark_theme():
    """Detect if the system is using dark theme."""
    try:
        # Check Windows dark mode via registry
        import winreg

        try:
            key = winreg.OpenKey(
                winreg.HKEY_CURRENT_USER,
                r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            )
            value, _ = winreg.QueryValueEx(key, "SystemUsesLightTheme")
            winreg.CloseKey(key)
            return value == 0  # 0 = dark mode, 1 = light mode
        except (FileNotFoundError, OSError):
            pass

        # Fallback: Check Qt application palette
        app = QApplication.instance()
        if app and isinstance(app, QApplication):
            palette = app.palette()
            window_color = palette.color(palette.ColorRole.Window)
            # If window background is dark, assume dark theme
            return window_color.lightness() < 128

        return False  # Default to light theme if detection fails
    except ImportError:
        # If winreg is not available (non-Windows), use Qt palette
        app = QApplication.instance()
        if app and isinstance(app, QApplication):
            palette = app.palette()
            window_color = palette.color(palette.ColorRole.Window)
            return window_color.lightness() < 128
        return False


def invert_icon(pixmap):
    """Invert the colors of a QPixmap for dark theme compatibility while preserving transparency."""
    inverted = QPixmap(pixmap.size())
    inverted.fill(Qt.GlobalColor.transparent)

    painter = QPainter(inverted)
    painter.setRenderHint(QPainter.RenderHint.Antialiasing)
    painter.setCompositionMode(QPainter.CompositionMode.CompositionMode_SourceOver)

    # Draw the original pixmap
    painter.drawPixmap(0, 0, pixmap)

    # Apply color inversion while preserving alpha channel
    painter.setCompositionMode(QPainter.CompositionMode.CompositionMode_Difference)
    painter.fillRect(inverted.rect(), Qt.GlobalColor.white)

    # Restore original alpha channel
    painter.setCompositionMode(QPainter.CompositionMode.CompositionMode_DestinationIn)
    painter.drawPixmap(0, 0, pixmap)

    painter.end()
    return inverted


def build_tray(main_window):  # pragma: no cover
    # Try to locate an icon inside a conventional assets folder; fall back to a simple empty icon.
    icon = QIcon()
    icon_found = False

    for candidate in [
        Path(__file__).parent / ".." / ".." / "assets" / "icon.png",
        Path(__file__).parent / ".." / ".." / "assets" / "icon.ico",
    ]:
        if candidate.exists():
            # Load the original icon
            original_pixmap = QPixmap(str(candidate.resolve()))

            # Check if we need to invert for dark theme
            if is_dark_theme() and candidate.suffix.lower() == ".png":
                # Invert colors for dark theme
                inverted_pixmap = invert_icon(original_pixmap)
                icon = QIcon(inverted_pixmap)
            else:
                icon = QIcon(str(candidate.resolve()))

            icon_found = True
            break

    if not icon_found:
        # Create a simple fallback icon if no icon file found
        fallback_pixmap = QPixmap(16, 16)
        fallback_pixmap.fill(Qt.GlobalColor.transparent)
        painter = QPainter(fallback_pixmap)
        painter.setBrush(
            Qt.GlobalColor.gray if not is_dark_theme() else Qt.GlobalColor.lightGray
        )
        painter.drawEllipse(2, 2, 12, 12)
        painter.end()
        icon = QIcon(fallback_pixmap)

    tray = QSystemTrayIcon(icon)
    menu = QMenu()

    def open_file():
        file, _ = QFileDialog.getOpenFileName(
            main_window,
            "Select Media File",
            "",
            "Media Files (*.wav *.mp3 *.mp4 *.avi *.mkv *.m4a *.flac *.ogg)",
        )
        if file:
            main_window._generate_srt(Path(file))

    def show_hotkey_settings():
        from .hotkey_dialog import HotkeyDialog
        current = load_hotkey()
        
        dialog = HotkeyDialog(main_window, current)
        
        def on_hotkey_captured(hotkey):
            save_hotkey(hotkey)
            main_window.apply_hotkey_change(hotkey)
        
        dialog.hotkey_captured.connect(on_hotkey_captured)
        dialog.exec()

    def show_about():
        from PySide6.QtWidgets import QMessageBox

        QMessageBox.about(
            main_window,
            "About asrpro",
            "asrpro v0.1\n\n"
            "AI-powered speech recognition and transcription\n"
            "Built with PySide6 and FastAPI\n\n"
            "Supports:\n"
            "• NVIDIA Parakeet TDT models\n"
            "• Whisper ONNX models\n"
            "• OpenAI-compatible API server\n"
            "• Real-time hotkey transcription",
        )

    # Build menu with sections - sleek and minimal
    menu.addAction("Show Window", lambda: (main_window.show(), main_window.raise_(), main_window.activateWindow()))
    menu.addSeparator()

    menu.addAction("Process Media File", open_file)
    menu.addAction("Hotkey Settings", show_hotkey_settings)
    menu.addSeparator()

    menu.addAction("About", show_about)
    menu.addAction("Exit", lambda: main_window.close_app())

    # Try to apply icons for actions if present in assets/icons
    def icon_path(name: str) -> str | None:
        p = Path(__file__).parent / ".." / ".." / "assets" / "icons" / f"{name}.svg"
        return str(p.resolve()) if p.exists() else None

    def set_icon_for_action(action_text: str, filename: str):
        path = icon_path(filename)
        if path:
            from PySide6.QtGui import QIcon as _QIcon

            for act in menu.actions():
                if act.text().endswith(action_text):
                    act.setIcon(_QIcon(path))
                    break

    set_icon_for_action("Show Window", "monitor")
    set_icon_for_action("Process Media File", "folder")
    set_icon_for_action("Hotkey Settings", "keyboard")
    set_icon_for_action("About", "info")
    set_icon_for_action("Exit", "x")

    # Mac-like compact styling
    menu.setStyleSheet(
        """
        QMenu { 
            background-color: rgba(30, 30, 30, 0.95); 
            color: #ffffff; 
            border: 1px solid rgba(255, 255, 255, 0.1); 
            border-radius: 8px; 
            padding: 4px 0;
            font-size: 13px;
            font-family: -apple-system, BlinkMacSystemFont, "SF Pro Display", sans-serif;
        }
        QMenu::item { 
            padding: 6px 14px; 
            border: none;
            margin: 0 2px;
            border-radius: 4px;
            min-height: 18px;
        }
        QMenu::item:selected { 
            background-color: rgba(0, 122, 255, 0.8);
            color: #ffffff;
        }
        QMenu::separator { 
            height: 1px; 
            background: rgba(255, 255, 255, 0.1); 
            margin: 4px 6px; 
            border: none;
        }
        QMenu::icon {
            padding-left: 2px;
            width: 16px;
            height: 16px;
        }
        """
    )
    tray.setContextMenu(menu)
    tray.setToolTip("asrpro - AI Speech Recognition")

    # Add double-click to show window
    tray.activated.connect(
        lambda reason: (
            main_window.show()
            if reason == QSystemTrayIcon.ActivationReason.DoubleClick
            else None
        )
    )

    return tray
