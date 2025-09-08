"""System tray builder."""

from __future__ import annotations
from PySide6.QtGui import QIcon
from pathlib import Path
from PySide6.QtWidgets import QSystemTrayIcon, QMenu, QFileDialog, QInputDialog
from ..hotkey import load_hotkey, save_hotkey


def build_tray(main_window):  # pragma: no cover
    # Try to locate an icon inside a conventional assets folder; fall back to a simple empty icon.
    icon = QIcon()
    for candidate in [
        Path(__file__).parent / ".." / ".." / "assets" / "icon.png",
        Path(__file__).parent / ".." / ".." / "assets" / "icon.ico",
    ]:
        if candidate.exists():
            icon = QIcon(str(candidate.resolve()))
            break

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
