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
        current = load_hotkey()
        hk, ok = QInputDialog.getText(
            main_window,
            "Hotkey Configuration",
            "Global hotkey (e.g. <ctrl>+<alt>+t):\n\nSupported keys: <ctrl>, <alt>, <shift>, <cmd>\nCombine with letters, numbers, or F1-F12",
            text=current,
        )
        if ok and hk:
            save_hotkey(hk)
            main_window.apply_hotkey_change(hk)

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

    # Build menu with sections
    menu.addAction("🖥️  Show Window", main_window.show)
    menu.addSeparator()

    menu.addAction("📁  Process Media File", open_file)
    menu.addAction("⌨️  Hotkey Settings", show_hotkey_settings)
    menu.addSeparator()

    menu.addAction("ℹ️  About", show_about)
    menu.addAction("❌  Exit", lambda: main_window.close_app())

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
