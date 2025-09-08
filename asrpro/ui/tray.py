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
        QFileDialog.getOpenFileName(main_window, "Open Media")

    def show_hotkey_settings():
        current = load_hotkey()
        hk, ok = QInputDialog.getText(
            main_window, "Hotkey", "Toggle Hotkey (e.g. <ctrl>+<alt>+t):", text=current
        )
        if ok and hk:
            save_hotkey(hk)
            main_window.apply_hotkey_change(hk)

    menu.addAction("Open Window", main_window.show)
    menu.addAction("Open File", open_file)
    menu.addAction("Hotkey Settings", show_hotkey_settings)
    menu.addSeparator()
    menu.addAction("Exit", lambda: main_window.close_app())
    tray.setContextMenu(menu)
    tray.setToolTip("asrpro")
    return tray
