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
        Path(__file__).parent / ".." / ".." / "assets" / "icon.svg",
    ]:
        print(f"[Tray] Checking icon path: {candidate.resolve()}")
        if candidate.exists():
            print(f"[Tray] Found icon file: {candidate}")
            # Load the original icon
            original_pixmap = QPixmap(str(candidate.resolve()))
            
            if original_pixmap.isNull():
                print(f"[Tray] Failed to load icon from: {candidate}")
                continue

            # Ensure proper sizing for tray icon
            if original_pixmap.width() > 32 or original_pixmap.height() > 32:
                original_pixmap = original_pixmap.scaled(32, 32, Qt.AspectRatioMode.KeepAspectRatio, Qt.TransformationMode.SmoothTransformation)

            # Check if we need to invert for dark theme
            if is_dark_theme() and candidate.suffix.lower() in [".png", ".svg"]:
                print("[Tray] Applying dark theme inversion")
                # Invert colors for dark theme
                inverted_pixmap = invert_icon(original_pixmap)
                icon = QIcon(inverted_pixmap)
            else:
                print("[Tray] Using original icon")
                icon = QIcon(original_pixmap)

            icon_found = True
            print(f"[Tray] Icon loaded successfully from: {candidate}")
            break
        else:
            print(f"[Tray] Icon file not found: {candidate}")

    if not icon_found:
        # Create a simple fallback icon if no icon file found
        fallback_pixmap = QPixmap(32, 32)
        fallback_pixmap.fill(Qt.GlobalColor.transparent)
        painter = QPainter(fallback_pixmap)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        painter.setBrush(
            Qt.GlobalColor.gray if not is_dark_theme() else Qt.GlobalColor.lightGray
        )
        painter.drawEllipse(4, 4, 24, 24)
        painter.end()
        icon = QIcon(fallback_pixmap)
        print("[Tray] Using fallback icon")

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
        # Show the main window and navigate to keyboard settings
        main_window.show()
        main_window.raise_()
        main_window.activateWindow()
        
        # Navigate to keyboard section (native UI)
        try:
            if hasattr(main_window, "sidebar"):
                main_window.sidebar.set_active_section("keyboard")
            if hasattr(main_window, "content_area"):
                main_window.content_area.show_page("keyboard")
        except Exception as e:
            print(f"[Tray] Failed to navigate to keyboard page: {e}")

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

    # Apply icons for menu actions with enhanced handling
    def load_menu_icon(name: str) -> QIcon:
        """Load an icon for menu items with proper sizing and theme handling."""
        icon_candidates = [
            Path(__file__).parent / ".." / ".." / "assets" / "icons" / f"{name}.svg",
            Path(__file__).parent / ".." / ".." / "assets" / "icons" / f"{name}.png",
        ]
        
        for icon_path in icon_candidates:
            if icon_path.exists():
                pixmap = QPixmap(str(icon_path.resolve()))
                if not pixmap.isNull():
                    # Scale to appropriate menu icon size
                    if pixmap.width() > 16 or pixmap.height() > 16:
                        pixmap = pixmap.scaled(16, 16, Qt.AspectRatioMode.KeepAspectRatio, Qt.TransformationMode.SmoothTransformation)
                    
                    # Apply theme-appropriate coloring for SVG icons
                    if is_dark_theme() and icon_path.suffix.lower() == ".svg":
                        # For dark theme, ensure icons are visible
                        pixmap = invert_icon(pixmap) if name not in ["circle-check"] else pixmap
                    
                    return QIcon(pixmap)
        
        # Return empty icon if no file found
        print(f"[Tray] Warning: No icon found for {name}")
        return QIcon()

    def set_icon_for_action(action_text: str, icon_name: str):
        """Set icon for a menu action by matching action text."""
        icon = load_menu_icon(icon_name)
        if not icon.isNull():
            for act in menu.actions():
                if action_text in act.text():
                    act.setIcon(icon)
                    print(f"[Tray] Applied {icon_name} icon to '{act.text()}' action")
                    break

    # Set icons for menu items
    set_icon_for_action("Show Window", "monitor")
    set_icon_for_action("Process Media File", "folder-open") 
    set_icon_for_action("Hotkey Settings", "keyboard")
    set_icon_for_action("About", "info")
    set_icon_for_action("Exit", "x")

    # Authentic macOS context menu styling
    menu.setStyleSheet(
        """
        QMenu { 
            background-color: rgba(242, 242, 247, 0.78);
            color: #1d1d1f; 
            border: 0.5px solid rgba(0, 0, 0, 0.04);
            border-radius: 8px; 
            padding: 4px 0;
            font-size: 13px;
            font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", sans-serif;
            font-weight: 400;
            min-width: 180px;
        }
        QMenu[darkMode="true"] { 
            background-color: rgba(30, 30, 30, 0.9);
            color: #f2f2f7;
            border: 0.5px solid rgba(255, 255, 255, 0.04);
        }
        QMenu::item { 
            padding: 6px 14px 6px 32px;
            border: none;
            margin: 1px 2px;
            border-radius: 4px;
            min-height: 18px;
            font-weight: 400;
        }
        QMenu::item:selected { 
            background-color: rgba(0, 122, 255, 1.0);
            color: #ffffff;
        }
        QMenu::item:disabled {
            color: rgba(29, 29, 31, 0.3);
        }
        QMenu[darkMode="true"]::item:disabled {
            color: rgba(242, 242, 247, 0.3);
        }
        QMenu::separator { 
            height: 0.5px; 
            background: rgba(0, 0, 0, 0.1); 
            margin: 4px 6px; 
            border: none;
        }
        QMenu[darkMode="true"]::separator {
            background: rgba(255, 255, 255, 0.1);
        }
        QMenu::icon {
            padding-left: 6px;
            width: 16px;
            height: 16px;
            left: 8px;
            position: absolute;
        }
        """
    )
    # Apply dark mode styling if system is using dark theme
    if is_dark_theme():
        menu.setProperty("darkMode", "true")
        menu.setStyleSheet(menu.styleSheet() + """
        QMenu[darkMode="true"] { 
            background-color: rgba(30, 30, 30, 0.9);
            color: #f2f2f7;
            border: 0.5px solid rgba(255, 255, 255, 0.04);
        }
        QMenu[darkMode="true"]::item:disabled {
            color: rgba(242, 242, 247, 0.3);
        }
        QMenu[darkMode="true"]::separator {
            background: rgba(255, 255, 255, 0.1);
        }
        """)
        menu.style().polish(menu)  # Force style refresh
    
    # Ensure menu is properly styled before setting
    menu.setAttribute(Qt.WidgetAttribute.WA_StyledBackground, True)
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
