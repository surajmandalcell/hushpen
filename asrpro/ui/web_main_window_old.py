"""Frameless WebEngine-based main window that loads temp/index.html.
Replicates the HTML UI exactly, adds subtle animations and translucent sidebar.
Maintains public methods expected by tray.py (set_tray_icon, close_app, apply_hotkey_change,
and generate_srt_from_file for processing media files)."""

from __future__ import annotations

import os
from pathlib import Path
from typing import Optional

# Mitigate potential blank screen on some Windows GPUs
if os.name == "nt":
    os.environ.setdefault(
        "QTWEBENGINE_CHROMIUM_FLAGS", "--disable-gpu --disable-software-rasterizer"
    )

from PySide6.QtCore import Qt, QUrl, QPropertyAnimation, QEasingCurve
from PySide6.QtWidgets import (
    QWidget,
    QVBoxLayout,
    QGraphicsDropShadowEffect,
    QApplication,
    QFrame,
)
from PySide6.QtGui import QColor
try:
    from PySide6.QtWebEngineWidgets import QWebEngineView  # type: ignore
except Exception:  # pragma: no cover
    QWebEngineView = None  # type: ignore

from ..model_manager import ModelManager
from ..hotkey import ToggleHotkey


class MainWindow(QWidget):  # pragma: no cover
    def __init__(self):
        super().__init__()

        # Core logic pieces we keep available for tray actions
        self.model_manager = ModelManager()
        self.tray_icon = None
        self.hotkey = ToggleHotkey(self._on_toggle)
        self.hotkey.start()

        # Window appearance: frameless, fixed size to match HTML
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint | Qt.WindowType.Window)
        self.resize(1080, 720)
        
        # Simplified layout without problematic shadow effects
        root = QVBoxLayout(self)
        root.setContentsMargins(0, 0, 0, 0)  # Remove margins to prevent overlay issues
        root.setSpacing(0)

        if QWebEngineView is not None:
            self.web = QWebEngineView(self)
            self.web.setMinimumSize(1080, 720)  # Set to full window size
            from PySide6.QtWidgets import QSizePolicy
            self.web.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Expanding)
            
            # Enable console message logging
            try:
                from PySide6.QtWebEngineCore import QWebEnginePage
                
                class DebugWebPage(QWebEnginePage):
                    def javaScriptConsoleMessage(self, level, message, line, source):
                        print(f"[WebEngine Console] {message} (line {line})")
                        super().javaScriptConsoleMessage(level, message, line, source)
                
                debug_page = DebugWebPage(self.web)  # Pass parent to ensure proper lifecycle
                self.web.setPage(debug_page)
                print("[WebEngine] Debug page with console logging enabled")
            except Exception as e:
                print(f"[WebEngine] Could not enable console logging: {e}")
            
            root.addWidget(self.web)
        else:
            # Minimal fallback when QtWebEngine is unavailable
            from PySide6.QtWidgets import QLabel

            self.web = None
            lbl = QLabel(
                "Qt WebEngine is not available.\nInstall PySide6 with WebEngine support."
            )
            lbl.setAlignment(Qt.AlignmentFlag.AlignCenter)
            root.addWidget(lbl)

        # Load the provided HTML design
        if QWebEngineView is not None and self.web is not None:
            html_path = Path.cwd() / "temp" / "index.html"
            self.web.setContextMenuPolicy(Qt.ContextMenuPolicy.NoContextMenu)
            try:
                # Ensure a non-white background while loading
                if self.web.page():
                    self.web.page().setBackgroundColor(QColor("#1e1e1e"))
                    print(f"[WebEngine] Background color set to #1e1e1e")
            except Exception as e:
                print(f"[WebEngine] Failed to set background color: {e}")
            
            if html_path.exists():
                # Write debug info to a log file
                with open("webengine_debug.log", "w") as f:
                    f.write(f"[WebEngine] Loading HTML from: {html_path}\n")
                    f.write(f"[WebEngine] HTML file exists: {html_path.exists()}\n")
                    f.write(f"[WebEngine] Current working directory: {Path.cwd()}\n")
                    f.flush()
                
                # Load, strip external icon CDN, and replace icon placeholders with local assets
                try:
                    raw = html_path.read_text(encoding="utf-8")
                    
                    with open("webengine_debug.log", "a") as f:
                        f.write(f"[WebEngine] HTML file loaded, size: {len(raw)} characters\n")
                        f.flush()
                    
                    import re
                    # Remove lucide CDN script
                    original_scripts = len(re.findall(r'<script[^>]+src="https?://[^>]*lucide[^>]*></script>', raw, flags=re.IGNORECASE))
                    raw = re.sub(r'<script[^>]+src="https?://[^>]*lucide[^>]*></script>', "", raw, flags=re.IGNORECASE)
                    print(f"[WebEngine] Removed {original_scripts} Lucide CDN scripts")
                    
                    # Replace <i data-lucide="name"> with local SVG <img>
                    def _repl(m):
                        name = m.group(1)
                        aria_match = re.search(r'aria-label="([^"]+)"', m.group(0))
                        aria = aria_match.group(1) if aria_match else f"{name} icon"
                        style_match = re.search(r'style="([^"]+)"', m.group(0))
                        style = f' style="{style_match.group(1)}"' if style_match else ''
                        return f'<img class="icon" src="assets/icons/{name}.svg" alt="{aria}"{style}>'
                    
                    icon_count = len(re.findall(r'<i\s+data-lucide="([a-z0-9\-]+)"[^>]*></i>', raw, flags=re.IGNORECASE))
                    raw = re.sub(r'<i\s+data-lucide="([a-z0-9\-]+)"[^>]*></i>', _repl, raw, flags=re.IGNORECASE)
                    print(f"[WebEngine] Replaced {icon_count} Lucide icons with SVG images")
                    
                    # Stub lucide.createIcons() calls to no-op
                    raw = raw.replace('lucide.createIcons();', 'console.log("Lucide icons replaced with SVG images");')
                    
                    # Add base styles for replaced icons with better filter for dark theme
                    icon_styles = """
.icon {
    width: 16px;
    height: 16px;
    vertical-align: middle;
    filter: brightness(0) saturate(100%) invert(85%) sepia(8%) saturate(359%) hue-rotate(183deg) brightness(90%) contrast(87%);
    display: inline-block;
}

/* Fix window sizing to use full viewport */
.window {
    width: 100vw !important;
    height: 100vh !important;
    border-radius: 0 !important;
    box-shadow: none !important;
}
"""
                    raw = raw.replace('</style>', icon_styles + '\n</style>')
                    
                    # Add debug console logging
                    debug_script = """
<script>
console.log('HTML loaded successfully');
document.addEventListener('DOMContentLoaded', function() {
    console.log('DOM content loaded');
    console.log('Body style:', window.getComputedStyle(document.body).background);
});
window.addEventListener('load', function() {
    console.log('Window loaded');
});
</script>
"""
                    raw = raw.replace('</head>', debug_script + '\n</head>')
                    
                    base = QUrl.fromLocalFile(str(html_path.parent.resolve()) + "/")
                    print(f"[WebEngine] Base URL: {base.toString()}")
                    
                    if self.web:
                        self.web.setHtml(raw, base)
                        self.web.loadFinished.connect(self._post_load)
                    
                    with open("webengine_debug.log", "a") as f:
                        f.write(f"[WebEngine] HTML set to WebEngine view\n")
                        f.write(f"[WebEngine] Base URL: {base.toString()}\n")
                        f.flush()
                    
                except Exception as e:
                    print(f"[WebEngine] Error processing HTML: {e}")
                    if self.web:
                        self.web.setHtml(f"<html><body style='background:#1e1e1e;color:#fff;font-family:system-ui;padding:20px'><h1>Error loading UI</h1><p>{str(e)}</p></body></html>")
            else:
                print(f"[WebEngine] HTML file not found at: {html_path}")
                # Fallback content if missing
                if self.web:
                    self.web.setHtml(
                        """
                        <html><body style='display:flex;align-items:center;justify-content:center;height:100vh;background:linear-gradient(135deg,#667eea,#764ba2);color:#fff;font-family:system-ui'>
                        <div style='text-align:center'>
                            <h1>UI template not found</h1>
                            <p>Expected at temp/index.html</p>
                        </div>
                        </body></html>
                        """
                    )

        # Simple fade-in animation on show
        self._fade = QPropertyAnimation(self, b"windowOpacity")
        self._fade.setDuration(250)
        self._fade.setEasingCurve(QEasingCurve.Type.InOutQuad)

        # Drag support for frameless window
        self._drag_pos = None

    # ---- Window behavior -------------------------------------------------
    def showEvent(self, event):
        self.setWindowOpacity(0.0)
        super().showEvent(event)
        self._fade.stop()
        self._fade.setStartValue(0.0)
        self._fade.setEndValue(1.0)
        self._fade.start()

    def mousePressEvent(self, event):
        if event.button() == Qt.MouseButton.LeftButton:
            self._drag_pos = event.globalPosition().toPoint() - self.frameGeometry().topLeft()
            event.accept()
        else:
            super().mousePressEvent(event)

    def mouseMoveEvent(self, event):
        if self._drag_pos and event.buttons() & Qt.MouseButton.LeftButton:
            self.move(event.globalPosition().toPoint() - self._drag_pos)
            event.accept()
        else:
            super().mouseMoveEvent(event)

    def mouseReleaseEvent(self, event):
        self._drag_pos = None
        super().mouseReleaseEvent(event)

    # ---- Post-load polish with debugging ---------------------
    def _post_load(self, ok: bool):
        with open("webengine_debug.log", "a") as f:
            f.write(f"[WebEngine] Load finished, success: {ok}\n")
            f.flush()
        
        if not ok:
            with open("webengine_debug.log", "a") as f:
                f.write("[WebEngine] Page failed to load!\n")
                f.flush()
            return
        
        css = r"""
        .window { animation: fadeSlideIn 200ms ease-in-out; }
        @keyframes fadeSlideIn { from { opacity: 0.98; transform: translateY(4px); } to { opacity: 1; transform: translateY(0); } }
        """
        js = (
            "console.log('Post-load script running');"
            "var s=document.createElement('style');s.type='text/css';"
            f"s.appendChild(document.createTextNode(`{css}`));document.head.appendChild(s);"
            "console.log('Animation styles added');"
        )
        try:
            if self.web and self.web.page():
                self.web.page().runJavaScript(js)
                print("[WebEngine] Post-load JavaScript executed")
        except Exception as e:
            print(f"[WebEngine] Failed to execute post-load JavaScript: {e}")
        
        # Check if page is actually visible
        def check_visibility():
            visibility_js = """
            console.log('Checking visibility...');
            console.log('Document ready state:', document.readyState);
            console.log('Body exists:', !!document.body);
            console.log('Window dimensions:', window.innerWidth, 'x', window.innerHeight);
            var style = window.getComputedStyle(document.body);
            console.log('Body background:', style.background);
            console.log('Body visibility:', style.visibility);
            console.log('Body display:', style.display);
            """
            try:
                if self.web and self.web.page():
                    self.web.page().runJavaScript(visibility_js)
            except Exception as e:
                print(f"[WebEngine] Failed to run visibility check: {e}")
        
        # Run visibility check after a short delay
        from PySide6.QtCore import QTimer
        QTimer.singleShot(500, check_visibility)

    # ---- Tray integration API -------------------------------------------
    def set_tray_icon(self, tray_icon):
        self.tray_icon = tray_icon

    def refresh_tray_icon_theme(self):
        # no-op hook to keep compatibility; tray handles theme switching
        pass

    def apply_hotkey_change(self, hk: str):
        try:
            self.hotkey.set_hotkey(hk)
        except Exception:
            pass

    def close_app(self):
        try:
            self.model_manager.unload()
        except Exception:
            pass
        app = QApplication.instance()
        if app:
            app.quit()
        self.close()

    # Backward/explicit method used by tray to process media files
    def generate_srt_from_file(self, path: Path) -> Optional[Path]:
        try:
            # Ensure a model is loaded; if not, pick a default
            if not self.model_manager.current_id:
                # Default to Whisper Medium ONNX if available
                default_id = "whisper-medium-onnx"
                try:
                    self.model_manager.load(default_id)
                except Exception:
                    # Fallback to any available model
                    avail = [m["id"] for m in self.model_manager.list_models()]
                    if not avail:
                        return None
                    self.model_manager.load(avail[0])

            # Run transcription and write .srt next to source
            srt_text = self.model_manager.transcribe(str(path), return_srt=True)
            out = Path(str(path))
            out = out.with_suffix(".srt")
            out.write_text(srt_text, encoding="utf-8")
            return out
        except Exception:
            return None

    # Legacy alias used by older tray action
    def _generate_srt(self, path: Path):
        return self.generate_srt_from_file(path)

    # Hotkey toggle handler (starts/stops recording in the classic UI). Here it's a stub.
    def _on_toggle(self, recording: bool):
        # This web UI version does not implement live recording yet.
        pass


__all__ = ["MainWindow"]
