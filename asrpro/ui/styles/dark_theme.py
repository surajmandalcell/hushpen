"""Dark theme color constants and styling utilities."""

from PySide6.QtGui import QColor, QPalette
from PySide6.QtCore import QObject


class DarkTheme:
    """Centralized color constants matching the original HTML/CSS design."""
    
    # Background Colors
    MAIN_BG = QColor("#1e1e1e")
    SIDEBAR_BG = QColor(26, 26, 26, 200)  # Translucent for Mac-like effect
    CARD_BG = QColor("#252525")
    CONTROL_BG = QColor("#333333")
    HOVER_BG = QColor("#2a2a2a")
    BUTTON_BG = QColor("#444444")
    BUTTON_HOVER_BG = QColor("#555555")
    
    # Text Colors
    PRIMARY_TEXT = QColor("#ffffff")
    SECONDARY_TEXT = QColor("#999999")
    MUTED_TEXT = QColor("#cccccc")
    
    # Accent Colors
    ACCENT_BLUE = QColor("#0a84ff")
    ACCENT_BLUE_BG = QColor(10, 132, 255, 38)  # rgba(10,132,255,0.15)
    HOVER_WHITE_BG = QColor(255, 255, 255, 20) # rgba(255,255,255,0.08)
    
    # Traffic Light Colors
    CLOSE_BTN = QColor("#ff5f57")
    MINIMIZE_BTN = QColor("#ffbd2e") 
    MAXIMIZE_BTN = QColor("#28ca42")
    
    # Status Colors
    SUCCESS_GREEN = QColor("#28ca42")
    SUCCESS_BG = QColor(40, 202, 66, 25)     # rgba(40,202,66,0.1)
    WARNING_YELLOW = QColor("#ffbd2e")
    WARNING_BG = QColor(255, 189, 46, 25)   # rgba(255,189,46,0.1)
    ERROR_RED = QColor("#ff453a")
    
    # Borders and Lines
    SUBTLE_BORDER = QColor(255, 255, 255, 13)  # rgba(255,255,255,0.05)
    CARD_BORDER = QColor("#333333")
    FOCUS_BORDER = QColor("#0a84ff")
    SECTION_BORDER = QColor("#333333")
    
    # Scrollbar Colors
    SCROLLBAR_TRACK = QColor(0, 0, 0, 0)      # transparent
    SCROLLBAR_THUMB = QColor("#444444")
    SCROLLBAR_THUMB_HOVER = QColor("#555555")

    @classmethod
    def get_palette(cls) -> QPalette:
        """Create a QPalette with dark theme colors."""
        palette = QPalette()
        
        # Window and base colors
        palette.setColor(QPalette.ColorRole.Window, cls.MAIN_BG)
        palette.setColor(QPalette.ColorRole.WindowText, cls.PRIMARY_TEXT)
        palette.setColor(QPalette.ColorRole.Base, cls.CARD_BG)
        palette.setColor(QPalette.ColorRole.AlternateBase, cls.SIDEBAR_BG)
        
        # Text colors
        palette.setColor(QPalette.ColorRole.Text, cls.PRIMARY_TEXT)
        palette.setColor(QPalette.ColorRole.BrightText, cls.PRIMARY_TEXT)
        palette.setColor(QPalette.ColorRole.PlaceholderText, cls.SECONDARY_TEXT)
        
        # Button colors
        palette.setColor(QPalette.ColorRole.Button, cls.CONTROL_BG)
        palette.setColor(QPalette.ColorRole.ButtonText, cls.PRIMARY_TEXT)
        
        # Highlight colors
        palette.setColor(QPalette.ColorRole.Highlight, cls.ACCENT_BLUE)
        palette.setColor(QPalette.ColorRole.HighlightedText, cls.PRIMARY_TEXT)
        
        # Disabled states
        palette.setColor(QPalette.ColorGroup.Disabled, QPalette.ColorRole.WindowText, cls.SECONDARY_TEXT)
        palette.setColor(QPalette.ColorGroup.Disabled, QPalette.ColorRole.Text, cls.SECONDARY_TEXT)
        palette.setColor(QPalette.ColorGroup.Disabled, QPalette.ColorRole.ButtonText, cls.SECONDARY_TEXT)
        
        return palette


class Dimensions:
    """Exact dimensions from the original HTML/CSS design."""
    
    # Main Window
    WINDOW_WIDTH = 1080
    WINDOW_HEIGHT = 720
    WINDOW_RADIUS = 12
    
    # Sidebar
    SIDEBAR_WIDTH = 240
    SIDEBAR_HEADER_HEIGHT = 32
    SIDEBAR_PADDING_V = 8
    SIDEBAR_PADDING_H = 0
    
    # Traffic Lights
    TRAFFIC_LIGHT_SIZE = 12
    TRAFFIC_LIGHT_GAP = 8
    TRAFFIC_LIGHT_CONTAINER_PADDING = 20
    
    # Logo Section
    LOGO_PADDING = 16
    LOGO_ICON_SIZE = 20
    LOGO_GAP = 8
    
    # Navigation Items
    NAV_ITEM_PADDING_V = 8
    NAV_ITEM_PADDING_H = 16
    NAV_ITEM_ICON_SIZE = 16
    NAV_ITEM_GAP = 8
    
    # Content Area
    CONTENT_PADDING_V = 40
    CONTENT_PADDING_H = 20
    CONTENT_HEADER_HEIGHT = 38
    
    # Components
    TOGGLE_WIDTH = 42
    TOGGLE_HEIGHT = 24
    TOGGLE_KNOB_SIZE = 18
    TOGGLE_KNOB_OFFSET = 3
    
    CARD_PADDING = 16
    CARD_RADIUS = 8
    CARD_BORDER_WIDTH = 1
    
    SETTING_ROW_PADDING_V = 12
    SETTING_CONTROL_MARGIN = 20
    
    # Scrollbar
    SCROLLBAR_WIDTH = 6
    SCROLLBAR_RADIUS = 3


class Fonts:
    """Font specifications matching the original design."""
    
    # Base font size
    BASE_SIZE = 13
    
    # Specific sizes
    H1_SIZE = 24
    PAGE_TITLE_SIZE = 20
    SECTION_TITLE_SIZE = 16
    SETTING_LABEL_SIZE = 14
    DESCRIPTION_SIZE = 12
    STATUS_SIZE = 11
    
    # Font weights (using QFont.Weight enum)
    from PySide6.QtGui import QFont
    NORMAL = QFont.Weight.Normal
    MEDIUM = QFont.Weight.Medium  
    SEMIBOLD = QFont.Weight.DemiBold
    BOLD = QFont.Weight.Bold
    
    @classmethod
    def get_system_font(cls) -> str:
        """Get the appropriate system font family."""
        import platform
        system = platform.system().lower()
        
        if system == "darwin":  # macOS
            return "SF Pro Text"
        elif system == "windows":
            return "Segoe UI"
        else:  # Linux and others
            return "system-ui"


class Spacing:
    """Consistent spacing values from the original design."""
    
    # Container spacing
    CONTAINER_PADDING_H = 20
    CONTAINER_PADDING_V = 40
    
    # Item spacing
    ITEM_PADDING_V = 8
    ITEM_PADDING_H = 16
    CONTROL_MARGIN = 20
    
    # Icon and text spacing
    ICON_TEXT_GAP = 10
    BUTTON_GAP = 8
    
    # Section spacing
    SECTION_MARGIN_TOP = 25
    SECTION_MARGIN_BOTTOM = 15
    SECTION_PADDING_BOTTOM = 8
    SECTION_GAP = 24
    
    # Page spacing
    PAGE_PADDING = 32
    
    # Component spacing
    CARD_GAP = 16
    CARD_PADDING = 20
    FORM_ROW_SPACING = 12