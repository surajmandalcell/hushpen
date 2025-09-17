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
from ...hotkey import load_hotkey, save_hotkey
from ..utils.invert import invert_icon


def is_dark_theme():
    """Detect if the system is using dark theme."""
    import platform
    import subprocess
    
    # macOS-specific dark mode detection
    if platform.system() == 'Darwin':
        try:
            # Check macOS appearance setting
            result = subprocess.run(
                ['defaults', 'read', '-g', 'AppleInterfaceStyle'],
                capture_output=True,
                text=True,
                timeout=1
            )
            return 'Dark' in result.stdout
        except (subprocess.TimeoutExpired, subprocess.CalledProcessError, FileNotFoundError):
            # Fall through to Qt detection
            pass
    
    # Windows-specific dark mode detection
    elif platform.system() == 'Windows':
        try:
            import winreg
            key = winreg.OpenKey(
                winreg.HKEY_CURRENT_USER,
                r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            )
            value, _ = winreg.QueryValueEx(key, "SystemUsesLightTheme")
            winreg.CloseKey(key)
            return value == 0  # 0 = dark mode, 1 = light mode
        except (ImportError, FileNotFoundError, OSError):
            # Fall through to Qt detection
            pass
    
    # Fallback: Check Qt application palette
    app = QApplication.instance()
    if app and isinstance(app, QApplication):
        palette = app.palette()
        window_color = palette.color(palette.ColorRole.Window)
        # If window background is dark, assume dark theme
        return window_color.lightness() < 128
    
    return False  # Default to light theme if detection fails




def _assets_dir() -> Path:
    # components/tray.py -> project_root/assets
    return Path(__file__).resolve().parents[3] / "assets"


def build_tray(main_window):  # pragma: no cover
    # Try to locate an icon inside a conventional assets folder; fall back to a simple empty icon.
    icon = QIcon()
    icon_found = False

    assets = _assets_dir()
    for candidate in [
        assets / "icon.png",
        assets / "icon.ico",
        assets / "icon.svg",
    ]:
        print(f"[Tray] Checking icon path: {candidate.resolve()}")
        if candidate.exists():
            print(f"[Tray] Found icon file: {candidate}")
            # Load the original icon
            original_pixmap = QPixmap(str(candidate.resolve()))
            
            if original_pixmap.isNull():
                print(f"[Tray] Failed to load icon from: {candidate}")
                continue

            # Ensure proper sizing for tray icon (macOS needs 22x22 for menu bar)
            import platform
            icon_size = 22 if platform.system() == 'Darwin' else 32
            if original_pixmap.width() > icon_size or original_pixmap.height() > icon_size:
                original_pixmap = original_pixmap.scaled(icon_size, icon_size, Qt.AspectRatioMode.KeepAspectRatio, Qt.TransformationMode.SmoothTransformation)

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
        assets = _assets_dir()
        icon_candidates = [
            assets / "icons" / f"{name}.svg",
            assets / "icons" / f"{name}.png",
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

    # Platform-specific menu styling
    import platform
    if platform.system() == 'Darwin':
        # macOS-specific clean menu (minimal styling, let Qt handle native appearance)
        menu.setStyleSheet(
            """
            QMenu { 
                font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", sans-serif;
                font-size: 13px;
                font-weight: normal;
            }
            QMenu::item { 
                padding: 4px 20px;
                font-weight: normal;
            }
            QMenu::item:selected { 
                background-color: rgba(0, 122, 255, 0.9);
                color: white;
            }
            """
        )
    else:
        # Windows/Linux styling
        menu.setStyleSheet(
            """
            QMenu { 
                background-color: rgba(242, 242, 247, 0.78);
                color: #1d1d1f; 
                border: 0.5px solid rgba(0, 0, 0, 0.04);
                border-radius: 8px; 
                padding: 4px 0;
                font-size: 13px;
                font-family: "Roboto", "Segoe UI", sans-serif;
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
