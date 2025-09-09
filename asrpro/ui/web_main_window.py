"""Modern WebEngine-based main window implementation.

This module provides a clean, modern implementation of the main application window
using PySide6 WebEngine with proper practices and error handling.
"""

from __future__ import annotations

import os
import re
from pathlib import Path
from typing import Optional

# Set WebEngine flags for better Windows compatibility
if os.name == "nt":
    os.environ.setdefault("QTWEBENGINE_CHROMIUM_FLAGS", "--disable-gpu")

from PySide6.QtCore import Qt, QUrl, QTimer, Signal
from PySide6.QtWidgets import QWidget, QVBoxLayout
from PySide6.QtGui import QColor

# Handle optional WebEngine import
try:
    from PySide6.QtWebEngineWidgets import QWebEngineView
    from PySide6.QtWebEngineCore import QWebEnginePage, QWebEngineSettings
    WEBENGINE_AVAILABLE = True
except ImportError:
    WEBENGINE_AVAILABLE = False
    QWebEngineView = None
    QWebEnginePage = None

from ..model_manager import ModelManager
from ..hotkey import ToggleHotkey


class MainWindow(QWidget):
    """Modern main application window with WebEngine UI."""
    
    # Signals for tray integration
    window_ready = Signal()
    
    def __init__(self):
        super().__init__()
        
        # Core application components
        self.model_manager = ModelManager()
        self.tray_icon = None
        self.hotkey = ToggleHotkey(self._on_hotkey_toggle)
        self.web_view: Optional[QWebEngineView] = None
        
        # Window configuration
        self._setup_window()
        self._setup_ui()
        self._load_content()
        
        # Start hotkey monitoring
        self.hotkey.start()
    
    def _setup_window(self) -> None:
        """Configure the main window properties."""
        self.setWindowTitle("Spokenly")
        self.setFixedSize(1080, 720)
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint | Qt.WindowType.Window)
        
        # Set window background for consistency
        self.setStyleSheet("QWidget { background-color: #1e1e1e; }")
    
    def _setup_ui(self) -> None:
        """Initialize the user interface components."""
        # Main layout
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)
        
        # Setup WebEngine view if available
        if WEBENGINE_AVAILABLE:
            self._setup_webengine(layout)
        else:
            self._setup_fallback(layout)
    
    def _setup_webengine(self, layout: QVBoxLayout) -> None:
        """Setup the WebEngine view with proper configuration."""
        self.web_view = QWebEngineView()
        
        # Configure WebEngine settings
        settings = self.web_view.settings()
        settings.setAttribute(QWebEngineSettings.WebAttribute.LocalContentCanAccessRemoteUrls, True)
        settings.setAttribute(QWebEngineSettings.WebAttribute.LocalContentCanAccessFileUrls, True)
        settings.setAttribute(QWebEngineSettings.WebAttribute.AllowRunningInsecureContent, True)
        
        # Set up the web page
        page = QWebEnginePage(self.web_view)
        page.setBackgroundColor(QColor("#1e1e1e"))
        self.web_view.setPage(page)
        
        # Connect signals
        self.web_view.loadFinished.connect(self._on_load_finished)
        
        # Add to layout
        layout.addWidget(self.web_view)
        
        print("[WebEngine] WebEngine view initialized successfully")
    
    def _setup_fallback(self, layout: QVBoxLayout) -> None:
        """Setup fallback UI when WebEngine is not available."""
        from PySide6.QtWidgets import QLabel
        
        label = QLabel("WebEngine not available.\nPlease install PySide6 with WebEngine support.")
        label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        label.setStyleSheet("color: white; font-size: 16px;")
        layout.addWidget(label)
        
        print("[WebEngine] Fallback UI initialized (WebEngine not available)")
    
    def _load_content(self) -> None:
        """Load the HTML content into the WebEngine view."""
        if not self.web_view:
            return
        
        html_file = Path.cwd() / "temp" / "index.html"
        
        if not html_file.exists():
            self._load_error_page("HTML file not found at temp/index.html")
            return
        
        try:
            # Read and process HTML content
            html_content = html_file.read_text(encoding="utf-8")
            processed_html = self._process_html(html_content)
            
            # Set base URL for relative resources
            base_url = QUrl.fromLocalFile(str(html_file.parent.absolute()) + "/")
            
            # Load content
            self.web_view.setHtml(processed_html, base_url)
            
            print(f"[WebEngine] HTML content loaded from {html_file}")
            
        except Exception as e:
            print(f"[WebEngine] Error loading HTML: {e}")
            self._load_error_page(f"Error loading HTML: {str(e)}")
    
    def _process_html(self, html_content: str) -> str:
        """Process HTML content for proper display in WebEngine."""
        
        # Remove external CDN scripts
        html_content = re.sub(
            r'<script[^>]+src="https?://[^"]*lucide[^"]*"[^>]*></script>',
            '',
            html_content,
            flags=re.IGNORECASE
        )
        
        # Replace Lucide icons with local SVG images
        def replace_icon(match):
            icon_name = match.group(1)
            return f'<img class="lucide-icon" src="assets/icons/{icon_name}.svg" alt="{icon_name} icon">'
        
        html_content = re.sub(
            r'<i[^>]+data-lucide="([^"]+)"[^>]*></i>',
            replace_icon,
            html_content,
            flags=re.IGNORECASE
        )
        
        # Remove lucide.createIcons() calls
        html_content = html_content.replace('lucide.createIcons();', '')
        
        # Add custom styles for proper display
        custom_styles = """
/* Ensure full viewport usage */
html, body {
    margin: 0;
    padding: 0;
    width: 100%;
    height: 100%;
    overflow: hidden;
}

/* Icon styling */
.lucide-icon {
    width: 16px;
    height: 16px;
    display: inline-block;
    vertical-align: middle;
    filter: brightness(0) invert(1);
}

/* Window adjustments for WebEngine */
.window {
    width: 100vw !important;
    height: 100vh !important;
    border-radius: 0 !important;
    box-shadow: none !important;
    margin: 0 !important;
}
"""
        
        # Insert custom styles
        html_content = html_content.replace('</style>', custom_styles + '\n</style>')
        
        return html_content
    
    def _load_error_page(self, error_message: str) -> None:
        """Load an error page when content loading fails."""
        if not self.web_view:
            return
        
        error_html = f"""
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body {{
                    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
                    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
                    color: white;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    height: 100vh;
                    margin: 0;
                    text-align: center;
                }}
                .error-container {{
                    background: rgba(0, 0, 0, 0.3);
                    padding: 40px;
                    border-radius: 10px;
                    max-width: 500px;
                }}
                h1 {{ color: #ff6b6b; margin-bottom: 20px; }}
                p {{ font-size: 16px; line-height: 1.5; }}
            </style>
        </head>
        <body>
            <div class="error-container">
                <h1>Unable to Load UI</h1>
                <p>{error_message}</p>
                <p>Please check that the temp/index.html file exists and is accessible.</p>
            </div>
        </body>
        </html>
        """
        
        self.web_view.setHtml(error_html)
    
    def _on_load_finished(self, success: bool) -> None:
        """Handle WebEngine load completion."""
        if success:
            print("[WebEngine] Content loaded successfully")
            self.window_ready.emit()
            
            # Add a slight delay before running post-load scripts
            QTimer.singleShot(100, self._post_load_setup)
        else:
            print("[WebEngine] Content failed to load")
    
    def _post_load_setup(self) -> None:
        """Perform post-load setup and verification."""
        if not self.web_view or not self.web_view.page():
            return
        
        # JavaScript to verify content is properly displayed
        verify_script = """
        console.log('=== WebEngine Content Verification ===');
        console.log('Document ready state:', document.readyState);
        console.log('Viewport size:', window.innerWidth + 'x' + window.innerHeight);
        console.log('Body computed style:', getComputedStyle(document.body).background);
        console.log('Main elements found:', document.querySelectorAll('.window, .sidebar, .nav-item').length);
        """
        
        self.web_view.page().runJavaScript(verify_script)
    
    def _on_hotkey_toggle(self, recording: bool) -> None:
        """Handle hotkey toggle events."""
        # Placeholder for hotkey functionality
        print(f"[Hotkey] Recording: {recording}")
    
    # Public API methods for tray integration
    
    def set_tray_icon(self, tray_icon) -> None:
        """Set the system tray icon reference."""
        self.tray_icon = tray_icon
    
    def apply_hotkey_change(self, hotkey: str) -> None:
        """Apply a new hotkey configuration."""
        try:
            self.hotkey.set_hotkey(hotkey)
            print(f"[Hotkey] Updated to: {hotkey}")
        except Exception as e:
            print(f"[Hotkey] Failed to update: {e}")
    
    def close_app(self) -> None:
        """Clean shutdown of the application."""
        try:
            self.model_manager.unload()
        except Exception as e:
            print(f"[Cleanup] Model manager error: {e}")
        
        self.close()
    
    def generate_srt_from_file(self, file_path: Path) -> Optional[Path]:
        """Generate SRT file from media file (for tray integration)."""
        try:
            if not self.model_manager.current_id:
                # Load default model if none loaded
                available_models = [m["id"] for m in self.model_manager.list_models()]
                if available_models:
                    self.model_manager.load(available_models[0])
                else:
                    return None
            
            # Transcribe and save SRT
            srt_content = self.model_manager.transcribe(str(file_path), return_srt=True)
            srt_path = file_path.with_suffix(".srt")
            srt_path.write_text(srt_content, encoding="utf-8")
            return srt_path
            
        except Exception as e:
            print(f"[Transcription] Error: {e}")
            return None
    
    # Legacy compatibility methods
    
    def _generate_srt(self, file_path: Path) -> Optional[Path]:
        """Legacy method alias for SRT generation."""
        return self.generate_srt_from_file(file_path)
    
    def refresh_tray_icon_theme(self) -> None:
        """Legacy method for tray icon theme refresh."""
        pass


__all__ = ["MainWindow"]