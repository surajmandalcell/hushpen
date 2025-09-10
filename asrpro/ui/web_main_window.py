"""Modern WebEngine-based main window implementation.

This module provides a clean, modern implementation of the main application window
using PySide6 WebEngine with proper practices and error handling.
"""

from __future__ import annotations

import os
import re
from pathlib import Path
from typing import Optional, TYPE_CHECKING

# Set WebEngine flags for better Windows compatibility
if os.name == "nt":
    os.environ.setdefault("QTWEBENGINE_CHROMIUM_FLAGS", "--disable-gpu")

from PySide6.QtCore import Qt, QUrl, QTimer, Signal, QObject, Slot, QPoint
from PySide6.QtWidgets import QWidget, QVBoxLayout
from PySide6.QtGui import QColor, QCloseEvent

# Handle optional WebEngine import
if TYPE_CHECKING:
    from PySide6.QtWebEngineWidgets import QWebEngineView
    from PySide6.QtWebEngineCore import QWebEnginePage, QWebEngineSettings
    from PySide6.QtWebChannel import QWebChannel
else:
    # Create stub classes for runtime
    QWebEngineView = None
    QWebEnginePage = None
    QWebChannel = None
    QWebEngineSettings = None

try:
    from PySide6.QtWebEngineWidgets import QWebEngineView as _QWebEngineView
    from PySide6.QtWebEngineCore import QWebEnginePage as _QWebEnginePage, QWebEngineSettings as _QWebEngineSettings
    from PySide6.QtWebChannel import QWebChannel as _QWebChannel
    WEBENGINE_AVAILABLE = True
    # Only reassign at runtime, not during TYPE_CHECKING
    if not TYPE_CHECKING:
        QWebEngineView = _QWebEngineView
        QWebEnginePage = _QWebEnginePage 
        QWebChannel = _QWebChannel
        QWebEngineSettings = _QWebEngineSettings
except ImportError:
    WEBENGINE_AVAILABLE = False
    _QWebEngineView = None
    _QWebEnginePage = None
    _QWebChannel = None
    _QWebEngineSettings = None

from ..model_manager import ModelManager
from .overlay import Overlay
from ..hotkey import ToggleHotkey
from .components import SettingsManager, RecordingManager, KeyboardManager, AboutManager


class WindowBridge(QObject):
    """Bridge class for JavaScript-Python communication."""
    
    def __init__(self, main_window):
        super().__init__()
        self.main_window = main_window
    
    @Slot()
    def hideWindow(self):
        """Hide window to system tray."""
        print("[Bridge] Hide window called")
        self.main_window.hide()
    
    @Slot() 
    def minimizeWindow(self):
        """Minimize window."""
        print("[Bridge] Minimize window called")
        self.main_window.showMinimized()


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
        self.overlay = Overlay()
        
        # Enterprise component managers
        self.settings_manager = SettingsManager(self)
        self.recording_manager = RecordingManager(self)
        self.keyboard_manager = KeyboardManager(self)
        self.about_manager = AboutManager(self)
        
        # Performance tracking
        import time
        self._start_time = time.time()
        
        # No window dragging
        
        # Window configuration
        self._setup_window()
        self._setup_ui()
        self._load_content()
        
        # Initialize enterprise components
        self._initialize_components()
        
        # Start hotkey monitoring
        self.hotkey.start()
    
    def _setup_window(self) -> None:
        """Configure the main window properties with enhanced macOS-like styling."""
        self.setWindowTitle("ASR Pro")
        self.setFixedSize(1080, 720)
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint | Qt.WindowType.Window)
        
        # Enable translucent background and depth effects
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        self.setAttribute(Qt.WidgetAttribute.WA_NoSystemBackground, True)
        
        # Enhanced window styling with deeper shadows and transparency
        self.setStyleSheet("""
            MainWindow {
                background-color: transparent;
            }
            QWidget {
                background-color: transparent;
                border-radius: 12px;
            }
        """)
        
        # Add macOS-style drop shadow
        self._add_window_shadow()
        
        # Create rounded window mask
        self._create_rounded_mask()
    
    def _add_window_shadow(self) -> None:
        """Add macOS-style drop shadow effect using QGraphicsDropShadowEffect."""
        try:
            from PySide6.QtWidgets import QGraphicsDropShadowEffect
            from PySide6.QtCore import QPointF
            from PySide6.QtGui import QColor
            
            # Create multiple shadow layers for depth
            shadow_effect = QGraphicsDropShadowEffect()
            shadow_effect.setBlurRadius(30)  # Large blur for soft shadow
            shadow_effect.setOffset(QPointF(0, 8))  # Slight downward offset
            shadow_effect.setColor(QColor(0, 0, 0, 120))  # Semi-transparent black
            
            # Apply shadow to the web view if available
            if hasattr(self, 'web_view') and self.web_view:
                self.web_view.setGraphicsEffect(shadow_effect)
            else:
                # Apply to main window as fallback
                self.setGraphicsEffect(shadow_effect)
                
        except Exception as e:
            print(f"[Window] Failed to add shadow effect: {e}")
    
    def _create_rounded_mask(self) -> None:
        """Create a rounded window mask to match the CSS border radius."""
        from PySide6.QtGui import QRegion, QPainterPath
        
        # Create a rounded rectangle path
        path = QPainterPath()
        path.addRoundedRect(self.rect(), 12, 12)  # 12px radius to match CSS
        
        # Create region from the path and set as window mask
        region = QRegion(path.toFillPolygon().toPolygon())
        self.setMask(region)
    
    def resizeEvent(self, event):
        """Override resize event to update the window mask."""
        super().resizeEvent(event)
        self._create_rounded_mask()
    
    # No mouse drag event handlers
    
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
    
    def _initialize_components(self) -> None:
        """Initialize all enterprise components with proper error handling."""
        try:
            print("[Components] Initializing enterprise component system...")
            
            # Initialize components in dependency order
            components = [
                self.settings_manager,
                self.recording_manager, 
                self.keyboard_manager,
                self.about_manager
            ]
            
            initialized_count = 0
            for component in components:
                try:
                    if component.initialize():
                        initialized_count += 1
                        print(f"[Components] {component.component_id} initialized successfully")
                    else:
                        print(f"[Components] {component.component_id} failed to initialize")
                except Exception as e:
                    print(f"[Components] {component.component_id} initialization error: {e}")
            
            print(f"[Components] Initialized {initialized_count}/{len(components)} components")
            
        except Exception as e:
            print(f"[Components] Component initialization failed: {e}")
    
    def _setup_webengine(self, layout: QVBoxLayout) -> None:
        """Setup the WebEngine view with proper configuration."""
        if not WEBENGINE_AVAILABLE or _QWebEngineView is None:
            print("[WebEngine] QWebEngineView not available")
            self._setup_fallback(layout)
            return
            
        self.web_view = _QWebEngineView()
        
        if not self.web_view:
            print("[WebEngine] Failed to create QWebEngineView")
            self._setup_fallback(layout)
            return
        
        # Configure WebEngine settings
        if hasattr(self.web_view, 'settings') and _QWebEngineSettings:
            settings = self.web_view.settings()
            if settings:
                settings.setAttribute(_QWebEngineSettings.WebAttribute.LocalContentCanAccessRemoteUrls, True)
                settings.setAttribute(_QWebEngineSettings.WebAttribute.LocalContentCanAccessFileUrls, True)
                settings.setAttribute(_QWebEngineSettings.WebAttribute.AllowRunningInsecureContent, True)
        
        # Set up the custom bridge page immediately
        self._setup_bridge_page()
        
        # Connect signals
        if self.web_view and hasattr(self.web_view, 'loadFinished'):
            self.web_view.loadFinished.connect(self._on_load_finished)
        
        # Add to layout
        layout.addWidget(self.web_view)
        
        print("[WebEngine] WebEngine view initialized successfully")
    
    def _setup_bridge_page(self) -> None:
        """Setup the custom bridge page for JavaScript-Python communication."""
        if not self.web_view:
            print("[Bridge] WebView not available for bridge setup")
            return
            
        if not WEBENGINE_AVAILABLE:
            return
            
        from PySide6.QtWebEngineCore import QWebEngineScript, QWebEnginePage
        
        class BridgeWebPage(QWebEnginePage):
            def __init__(self, main_window, parent=None):
                super().__init__(parent)
                self.main_window = main_window
            
            def javaScriptConsoleMessage(self, level, message, line, source):
                # Enhanced JavaScript-Python bridge with enterprise component system
                
                # Window control signals
                if message == "HIDE_WINDOW_SIGNAL":
                    print("[Bridge] Hide window signal received")
                    self.main_window.hide()
                elif message == "MINIMIZE_WINDOW_SIGNAL":
                    print("[Bridge] Minimize window signal received") 
                    self.main_window.showMinimized()
                elif message == "EXIT_APP_SIGNAL":
                    print("[Bridge] Exit app signal received")
                    self.main_window.close_app()
                
                # No window dragging signals
                
                # Enterprise component signal routing
                elif message.endswith("_SIGNAL"):
                    self.main_window._route_component_signal(message)
                
                else:
                    # Pass through other console messages (only log non-bridge messages)
                    if not message.endswith("_SIGNAL"):
                        print(f"[WebEngine Console] {message} (line {line})")
                
                super().javaScriptConsoleMessage(level, message, line, source)
        
        # Create and set the bridge page
        bridge_page = BridgeWebPage(self, self.web_view)
        bridge_page.setBackgroundColor(QColor("#1e1e1e"))
        
        if self.web_view and hasattr(self.web_view, 'setPage'):
            self.web_view.setPage(bridge_page)
        
        # Inject JavaScript bridge functions
        bridge_script = QWebEngineScript()
        bridge_script.setSourceCode("""
        // Window control functions that signal to Python
        window.hideWindow = function() {
            console.log('HIDE_WINDOW_SIGNAL');
        };
        
        window.minimizeWindow = function() {
            console.log('MINIMIZE_WINDOW_SIGNAL');
        };
        
        window.exitApp = function() {
            console.log('EXIT_APP_SIGNAL');
        };
        
        console.log('JavaScript bridge functions installed');
        """)
        bridge_script.setName("JSBridge")
        bridge_script.setWorldId(QWebEngineScript.ScriptWorldId.MainWorld)
        bridge_script.setInjectionPoint(QWebEngineScript.InjectionPoint.DocumentCreation)
        
        if hasattr(bridge_page, 'scripts'):
            bridge_page.scripts().insert(bridge_script)
        print("[Bridge] JavaScript bridge page setup completed")
    
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
        
        html_file = Path(__file__).parent / "templates" / "base.html"
        
        if not html_file.exists():
            self._load_error_page("HTML file not found at asrpro/ui/templates/base.html")
            return
        
        try:
            # Read and process HTML content
            html_content = html_file.read_text(encoding="utf-8")
            processed_html = self._process_html(html_content)
            
            # Set base URL for relative resources
            base_url = QUrl.fromLocalFile(str(html_file.parent.absolute()) + "/")
            
            # Load content
            if hasattr(self.web_view, 'setHtml'):
                self.web_view.setHtml(processed_html, base_url)
            
            print(f"[WebEngine] HTML content loaded from {html_file}")
            
        except Exception as e:
            print(f"[WebEngine] Error loading HTML: {e}")
            self._load_error_page(f"Error loading HTML: {str(e)}")
    
    def _process_html(self, html_content: str) -> str:
        """Process HTML content for proper display in WebEngine."""
        
        # Keep external CDN scripts - don't remove Lucide
        # html_content = re.sub(
        #     r'<script[^>]+src="https?://[^"]*lucide[^"]*"[^>]*></script>',
        #     '',
        #     html_content,
        #     flags=re.IGNORECASE
        # )
        
        # Replace Lucide icons with dynamically generated SVGs
        def replace_icon(match):
            icon_name = match.group(1)
            # Extract any existing classes and styles from the original <i> tag
            class_match = re.search(r'class="([^"]*)"', match.group(0))
            style_match = re.search(r'style="([^"]*)"', match.group(0))
            
            classes = f'lucide-icon {class_match.group(1)}' if class_match else 'lucide-icon'
            style = f' style="{style_match.group(1)}"' if style_match else ''
            
            try:
                # Generate SVG using python-lucide library
                from lucide import lucide_icon
                
                # Try the original name first
                try:
                    svg_content = lucide_icon(icon_name, width=16, height=16, cls=classes.strip())
                except Exception:
                    # Try common name mappings for icons that might have different names
                    icon_mappings = {
                        'check-circle': 'circle-check',
                        'globe-2': 'globe',
                        'settings-2': 'settings',
                        'file-text': 'file-text',
                        'file-plus': 'file-plus',
                        'folder-open': 'folder-open',
                        'refresh-cw': 'refresh-cw',
                        # Add more mappings as needed
                    }
                    
                    fallback_name = icon_mappings.get(icon_name, icon_name)
                    svg_content = lucide_icon(fallback_name, width=16, height=16, cls=classes.strip())
                
                # Add style attribute if present
                if style:
                    svg_content = svg_content.replace('<svg', f'<svg{style}', 1)
                return svg_content
            except Exception as e:
                print(f"[Icons] Failed to generate {icon_name} icon: {e}")
                # Fallback to a simple placeholder
                return f'<svg class="{classes}" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"{style}><circle cx="12" cy="12" r="1"/></svg>'
        
        # Use CDN Lucide icons instead of Python generation
        # html_content = re.sub(
        #     r'<i[^>]+data-lucide="([^"]+)"[^>]*></i>',
        #     replace_icon,
        #     html_content,
        #     flags=re.IGNORECASE
        # )
        
        # Keep lucide.createIcons() calls - needed for CDN icons
        # html_content = html_content.replace('lucide.createIcons();', '')
        
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

/* Icon styling for both img and inline SVG */
.lucide-icon, svg.lucide-icon {
    width: 16px !important;
    height: 16px !important;
    display: inline-block;
    vertical-align: middle;
    flex-shrink: 0;
}

/* For inline SVG icons (from lucide-py) */
svg.lucide-icon {
    stroke: currentColor;
    fill: none;
}

/* For fallback img icons */
img.lucide-icon {
    filter: brightness(0) saturate(100%) invert(85%) sepia(8%) saturate(359%) hue-rotate(183deg) brightness(90%) contrast(87%);
}

/* Context-specific styling */
.text-gray-400 svg.lucide-icon {
    color: #9ca3af;
}

.text-white svg.lucide-icon {
    color: #ffffff;
}

.text-blue-400 svg.lucide-icon {
    color: #60a5fa;
}

/* Fallback img styling for different contexts */
img.lucide-icon.text-gray-400 {
    filter: brightness(0) saturate(100%) invert(65%) sepia(8%) saturate(359%) hue-rotate(183deg) brightness(70%) contrast(87%);
}

img.lucide-icon.text-white {
    filter: brightness(0) invert(1);
}

img.lucide-icon.text-blue-400 {
    filter: brightness(0) saturate(100%) invert(58%) sepia(96%) saturate(2073%) hue-rotate(197deg) brightness(101%) contrast(101%);
}

/* Window adjustments for WebEngine */
.window {
    width: 100vw !important;
    height: 100vh !important;
    border-radius: 12px !important;
    box-shadow: none !important;
    margin: 0 !important;
    overflow: hidden;
}

/* Mac-style traffic light buttons */
.window-btn {
    position: relative;
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    border: none;
    outline: none;
}

.window-btn:hover {
    box-shadow: 0 0 8px rgba(0, 0, 0, 0.2);
}

.window-btn:active {
    box-shadow: 0 0 4px rgba(0, 0, 0, 0.4);
}

.window-btn:hover::after {
    content: '';
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.6);
    opacity: 1;
    transition: opacity 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.window-btn::after {
    content: '';
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.6);
    opacity: 0;
    transition: opacity 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.window-btn.close:hover::after {
    background: rgba(0, 0, 0, 0.8);
}

.window-btn.minimize:hover::after {
    background: rgba(0, 0, 0, 0.8);
}

.window-btn.maximize:hover::after {
    background: rgba(0, 0, 0, 0.8);
}

/* Animations */
.nav-item {
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.nav-item:hover {
    transform: translateX(2px);
    background-color: rgba(255, 255, 255, 0.05);
}

.toggle {
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.toggle:hover {
    transform: scale(1.05);
}

.model-card {
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.model-card:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
}

.btn-primary, .btn-secondary {
    transition: all 0.2s ease;
}

.btn-primary:hover {
    transform: translateY(-1px);
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.2);
}

.btn-secondary:hover {
    transform: translateY(-1px);
    background-color: rgba(255, 255, 255, 0.1);
}

/* Fade-in animation for the entire window */
@keyframes windowFadeIn {
    from {
        opacity: 0;
        transform: scale(0.95);
    }
    to {
        opacity: 1;
        transform: scale(1);
    }
}

.window {
    animation: windowFadeIn 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

/* Smooth scrolling */
* {
    scroll-behavior: smooth;
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
                <p>Please check that the asrpro/ui/templates/base.html file exists and is accessible.</p>
            </div>
        </body>
        </html>
        """
        
        if self.web_view and hasattr(self.web_view, 'setHtml'):
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
        if not WEBENGINE_AVAILABLE or not self.web_view or not hasattr(self.web_view, 'page') or not self.web_view.page():
            return
        
        # JavaScript bridge is already set up in _setup_bridge_page()
        
        # JavaScript to handle window controls and verification
        setup_script = """
        console.log('=== WebEngine Content Verification ===');
        console.log('Document ready state:', document.readyState);
        console.log('Viewport size:', window.innerWidth + 'x' + window.innerHeight);
        console.log('Body computed style:', getComputedStyle(document.body).background);
        console.log('Main elements found:', document.querySelectorAll('.window, .sidebar, .nav-item').length);
        
        // Add window control handlers
        function setupWindowControls() {
            const closeBtn = document.querySelector('.window-btn.close');
            const minimizeBtn = document.querySelector('.window-btn.minimize');
            const maximizeBtn = document.querySelector('.window-btn.maximize');
            
            if (closeBtn) {
                closeBtn.onclick = function(e) {
                    e.preventDefault();
                    console.log('Close button clicked - hiding to tray');
                    window.hideWindow();
                };
            }
            
            if (minimizeBtn) {
                minimizeBtn.onclick = function(e) {
                    e.preventDefault();
                    console.log('Minimize button clicked');
                    if (window.windowBridge) {
                        window.windowBridge.minimizeWindow();
                    }
                };
            }
            
            if (maximizeBtn) {
                maximizeBtn.onclick = function(e) {
                    e.preventDefault();
                    console.log('Maximize button clicked');
                };
            }
        }
        
        setupWindowControls();

        // Ensure sidebar logo uses local icon.png without touching source HTML
        (function installSidebarLogo(){
            try {
                const sidebar = document.querySelector('.sidebar');
                if (!sidebar) return;
                let logo = sidebar.querySelector('.logo');
                if (!logo) {
                    // Create a logo container if missing
                    logo = document.createElement('div');
                    logo.className = 'logo';
                    logo.style.display = 'flex';
                    logo.style.alignItems = 'center';
                    logo.style.gap = '10px';
                    logo.style.padding = '20px';
                    logo.style.borderBottom = '1px solid #333';
                    sidebar.insertBefore(logo, sidebar.firstChild);
                }
                // Clear any existing icon nodes
                while (logo.firstChild) logo.removeChild(logo.firstChild);
                // Insert image logo and app name
                const img = document.createElement('img');
                img.src = '../assets/icon.png';
                img.alt = 'ASR Pro logo';
                img.width = 20; img.height = 20;
                img.style.display = 'block';
                const span = document.createElement('span');
                span.textContent = 'ASR Pro';
                span.style.fontSize = '15px';
                span.style.fontWeight = '500';
                span.style.color = '#fff';
                logo.appendChild(img);
                logo.appendChild(span);
                console.log('Sidebar logo installed from ../assets/icon.png');
            } catch (e) {
                console.log('Sidebar logo install error:', e);
            }
        })();
        """
        
        if WEBENGINE_AVAILABLE and self.web_view and hasattr(self.web_view, 'page') and self.web_view.page():
            self.web_view.page().runJavaScript(setup_script)
    
    
    def _on_hotkey_toggle(self, recording: bool) -> None:
        """Handle hotkey toggle events."""
        print(f"[Hotkey] Recording: {recording}")
        try:
            if recording:
                self.overlay.show_smooth()
            else:
                self.overlay.close_smooth()
        except Exception as e:
            print(f"[Overlay] Error toggling overlay: {e}")
    
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
    
    # JavaScript Bridge Handler Methods
    
    def _handle_start_recording(self) -> None:
        """Handle start recording signal from JavaScript."""
        try:
            self.overlay.show_smooth()
            print("[Bridge] Recording started via UI")
        except Exception as e:
            print(f"[Bridge] Error starting recording: {e}")
    
    def _handle_stop_recording(self) -> None:
        """Handle stop recording signal from JavaScript."""
        try:
            self.overlay.close_smooth()
            print("[Bridge] Recording stopped via UI")
        except Exception as e:
            print(f"[Bridge] Error stopping recording: {e}")
    
    def _handle_save_settings(self, settings_json: str) -> None:
        """Handle save settings signal from JavaScript."""
        try:
            import json
            settings = json.loads(settings_json)
            print(f"[Bridge] Saving settings: {settings}")
            # TODO: Implement actual settings save logic
        except Exception as e:
            print(f"[Bridge] Error saving settings: {e}")
    
    def _handle_load_settings(self) -> None:
        """Handle load settings signal from JavaScript."""
        try:
            print("[Bridge] Loading settings")
            # TODO: Implement settings loading and inject into UI
        except Exception as e:
            print(f"[Bridge] Error loading settings: {e}")
    
    def _handle_open_media_file(self) -> None:
        """Handle open media file signal from JavaScript."""
        try:
            from PySide6.QtWidgets import QFileDialog
            file, _ = QFileDialog.getOpenFileName(
                self,
                "Select Media File",
                "",
                "Media Files (*.wav *.mp3 *.mp4 *.avi *.mkv *.m4a *.flac *.ogg)"
            )
            if file:
                print(f"[Bridge] Processing media file: {file}")
                self._generate_srt(Path(file))
        except Exception as e:
            print(f"[Bridge] Error opening media file: {e}")
    
    def closeEvent(self, event: QCloseEvent) -> None:
        """Override close event to hide to tray instead of closing."""
        event.ignore()
        self.hide()
        print("[Window] Window hidden to tray")
    
    def showEvent(self, event) -> None:
        """Override show event to ensure content is properly loaded."""
        super().showEvent(event)
        
        # If WebEngine content is not loaded, reload it
        if WEBENGINE_AVAILABLE and self.web_view and hasattr(self.web_view, 'page') and self.web_view.page() and hasattr(self.web_view, 'url'):
            current_url = self.web_view.url()
            if not current_url.isValid() or current_url.isEmpty():
                print("[Window] Reloading content on show")
                QTimer.singleShot(100, self._load_content)
        
        print("[Window] Window shown")
    
    # No drag handlers
    
    def _route_component_signal(self, message: str) -> None:
        """Route signals to appropriate enterprise components."""
        try:
            # Parse signal format: COMPONENT_ACTION_SIGNAL [data]
            signal_parts = message.replace("_SIGNAL", "").split(" ", 1)
            signal_name = signal_parts[0]
            signal_data = signal_parts[1] if len(signal_parts) > 1 else None
            
            # Map signals to components
            component_map = {
                "SAVE_SETTINGS": self.settings_manager,
                "LOAD_SETTINGS": self.settings_manager,
                "START_RECORDING": self.recording_manager,
                "STOP_RECORDING": self.recording_manager,
                "GET_STATUS": self.recording_manager,
                "REGISTER_HOTKEY": self.keyboard_manager,
                "UNREGISTER_HOTKEY": self.keyboard_manager,
                "GET_HOTKEYS": self.keyboard_manager,
                "GET_APP_INFO": self.about_manager,
                "GET_SYSTEM_INFO": self.about_manager,
                "GET_PERFORMANCE_METRICS": self.about_manager,
                "GET_COMPLETE_INFO": self.about_manager,
                "EXPORT_SYSTEM_REPORT": self.about_manager,
            }
            
            # Route to appropriate component
            component = component_map.get(signal_name)
            if component and hasattr(component, 'handle_bridge_signal'):
                print(f"[Bridge] Routing {signal_name} to {component.component_id}")
                component.handle_bridge_signal(signal_name, signal_data)
            else:
                print(f"[Bridge] No handler found for signal: {signal_name}")
                
        except Exception as e:
            print(f"[Bridge] Signal routing error: {e}")
    
    def emit_to_frontend(self, component_id: str, event: str, data: dict = None) -> None:
        """Emit events to the frontend JavaScript."""
        try:
            if self.web_view and hasattr(self.web_view, 'page'):
                # Inject JavaScript to emit event
                js_code = f"""
                if (window.ASRProApp) {{
                    window.ASRProApp.handleComponentEvent('{component_id}', '{event}', {data or '{}'});
                }} else {{
                    console.log('[Frontend] ASRProApp not ready, event queued:', '{component_id}', '{event}');
                }}
                """
                self.web_view.page().runJavaScript(js_code)
                print(f"[Frontend] Emitted {event} to {component_id}")
            else:
                print(f"[Frontend] WebView not available for event: {component_id}.{event}")
                
        except Exception as e:
            print(f"[Frontend] Emission error: {e}")
    
    def close_app(self) -> None:
        """Clean shutdown of the application."""
        try:
            self.model_manager.unload()
        except Exception as e:
            print(f"[Cleanup] Model manager error: {e}")
        
        # Cleanup enterprise components
        try:
            components = [
                self.settings_manager,
                self.recording_manager,
                self.keyboard_manager,
                self.about_manager
            ]
            
            for component in components:
                if hasattr(component, 'cleanup'):
                    component.cleanup()
                    
            print("[Cleanup] Enterprise components cleaned up")
        except Exception as e:
            print(f"[Cleanup] Component cleanup error: {e}")
        
        # Force close the application
        from PySide6.QtWidgets import QApplication
        app = QApplication.instance()
        if app:
            app.quit()
    
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
