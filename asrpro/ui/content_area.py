"""Content area widget with page management."""

from PySide6.QtCore import Qt
from PySide6.QtWidgets import QWidget, QStackedWidget, QVBoxLayout

from .styles.dark_theme import DarkTheme
from .pages.general_page import GeneralPage
from .pages.models_page import ModelsPage
from .pages.keyboard_page import KeyboardPage
from .pages.microphone_page import MicrophonePage
from .pages.about_page import AboutPage


class ContentArea(QWidget):
    """Main content area with page switching functionality."""
    
    def __init__(self, parent=None):
        super().__init__(parent)
        
        self.pages = {}
        self._setup_ui()
        self._apply_styles()
    
    def _setup_ui(self):
        """Set up the content area layout."""
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)
        
        # Create stacked widget for page switching
        self.stacked_widget = QStackedWidget()
        layout.addWidget(self.stacked_widget)
        
        # Create and add all pages
        self._create_pages()
    
    def _create_pages(self):
        """Create all page instances and add them to the stack."""
        page_classes = {
            "general": GeneralPage,
            "models": ModelsPage, 
            "keyboard": KeyboardPage,
            "microphone": MicrophonePage,
            "about": AboutPage,
        }
        
        for section_id, page_class in page_classes.items():
            page = page_class(self)
            self.pages[section_id] = page
            self.stacked_widget.addWidget(page)
    
    def _apply_styles(self):
        """Apply content area styling."""
        self.setStyleSheet(f"""
            ContentArea {{
                background-color: {DarkTheme.MAIN_BG.name()};
            }}
        """)
    
    def show_page(self, section_id: str):
        """Show the specified page."""
        if section_id in self.pages:
            page = self.pages[section_id]
            self.stacked_widget.setCurrentWidget(page)
            print(f"[ContentArea] Switched to page: {section_id}")
        else:
            print(f"[ContentArea] Warning: Unknown page section: {section_id}")
    
    def get_current_page_id(self) -> str:
        """Get the currently active page ID."""
        current_widget = self.stacked_widget.currentWidget()
        for section_id, page in self.pages.items():
            if page == current_widget:
                return section_id
        return "general"  # Default fallback