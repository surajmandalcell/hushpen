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
        # Layout: add a rounded frame with drop shadow containing the WebEngine view
        root = QVBoxLayout(self)
        root.setContentsMargins(16, 16, 16, 16)
        root.setSpacing(0)

        frame = QFrame(self)
        frame.setObjectName("WebFrame")
        frame.setStyleSheet("#WebFrame { background: #1e1e1e; border-radius: 10px; }")

        shadow = QGraphicsDropShadowEffect(frame)
        shadow.setBlurRadius(30)
        shadow.setOffset(0, 14)
        shadow.setColor(QColor(0, 0, 0, 150))
        frame.setGraphicsEffect(shadow)

        inner = QVBoxLayout(frame)
        inner.setContentsMargins(0, 0, 0, 0)
        inner.setSpacing(0)

        if QWebEngineView is not None:
            self.web = QWebEngineView(frame)
            inner.addWidget(self.web)
        else:
            # Minimal fallback when QtWebEngine is unavailable
            from PySide6.QtWidgets import QLabel

            self.web = None
            lbl = QLabel(
                "Qt WebEngine is not available.\nInstall PySide6 with WebEngine support."
            )
            lbl.setAlignment(Qt.AlignmentFlag.AlignCenter)
            inner.addWidget(lbl)

        root.addWidget(frame)

        # Load the provided HTML design
        if QWebEngineView is not None:
            html_path = Path.cwd() / "temp" / "index.html"
            self.web.setContextMenuPolicy(Qt.ContextMenuPolicy.NoContextMenu)
            try:
                # Ensure a non-white background while loading
                self.web.page().setBackgroundColor(QColor("#1e1e1e"))
            except Exception:
                pass
            if html_path.exists():
                # Load, strip external icon CDN, and replace icon placeholders with local assets
                raw = html_path.read_text(encoding="utf-8")
                import re
                # Remove lucide CDN script
                raw = re.sub(r'<script[^>]+src="https?://[^>]*lucide[^>]*></script>', "", raw, flags=re.IGNORECASE)
                # Replace <i data-lucide="name"> with local SVG <img>
                def _repl(m):
                    name = m.group(1)
                    aria_match = re.search(r'aria-label="([^"]+)"', m.group(0))
                    aria = aria_match.group(1) if aria_match else f"{name} icon"
                    style_match = re.search(r'style="([^"]+)"', m.group(0))
                    style = f' style="{style_match.group(1)}"' if style_match else ''
                    return f'<img class="icon" src="assets/icons/{name}.svg" alt="{aria}"{style}>'
                raw = re.sub(r'<i\s+data-lucide="([a-z0-9\-]+)"[^>]*></i>', _repl, raw, flags=re.IGNORECASE)
                # Stub lucide.createIcons() calls to no-op
                raw = raw.replace('lucide.createIcons();', '// lucide icons replaced with SVG images')
                # Add base styles for replaced icons with better filter for dark theme
                icon_styles = """
.icon {
    width: 16px;
    height: 16px;
    vertical-align: middle;
    filter: brightness(0) saturate(100%) invert(85%) sepia(8%) saturate(359%) hue-rotate(183deg) brightness(90%) contrast(87%);
    display: inline-block;
}
"""
                raw = raw.replace('</style>', icon_styles + '\n</style>')
                base = QUrl.fromLocalFile(str(html_path.parent.resolve()) + "/")
                self.web.setHtml(raw, base)
                self.web.loadFinished.connect(self._post_load)
            else:
                # Fallback content if missing
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

    # ---- Minimal post-load polish (no overlays/blur) ---------------------
    def _post_load(self, ok: bool):
        if not ok:
            return
        css = r"""
        .window { animation: fadeSlideIn 200ms ease-in-out; }
        @keyframes fadeSlideIn { from { opacity: 0.98; transform: translateY(4px); } to { opacity: 1; transform: translateY(0); } }
        """
        js = (
            "var s=document.createElement('style');s.type='text/css';"
            f"s.appendChild(document.createTextNode(`{css}`));document.head.appendChild(s);"
        )
        try:
            self.web.page().runJavaScript(js)
        except Exception:
            pass

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
