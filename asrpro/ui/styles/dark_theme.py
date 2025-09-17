"""Dark theme color constants and styling utilities."""

from PySide6.QtGui import QColor, QPalette
from PySide6.QtCore import QObject


class DarkTheme:
    """Centralized color constants matching the original HTML/CSS design."""

    # Background Colors
    MAIN_BG = QColor("#1e1e1e")
    SIDEBAR_BG = QColor(26, 26, 26)  # Opaque sidebar background
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
    HOVER_WHITE_BG = QColor(255, 255, 255, 20)  # rgba(255,255,255,0.08)

    # Traffic Light Colors
    CLOSE_BTN = QColor("#ff5f57")
    MINIMIZE_BTN = QColor("#ffbd2e")
    MAXIMIZE_BTN = QColor("#28ca42")

    # Status Colors
    SUCCESS_GREEN = QColor("#28ca42")
    SUCCESS_BG = QColor(40, 202, 66, 25)  # rgba(40,202,66,0.1)
    WARNING_YELLOW = QColor("#ffbd2e")
    WARNING_BG = QColor(255, 189, 46, 25)  # rgba(255,189,46,0.1)
    ERROR_RED = QColor("#ff453a")

    # Borders and Lines
    SUBTLE_BORDER = QColor(255, 255, 255, 13)  # rgba(255,255,255,0.05)
    CARD_BORDER = QColor("#333333")
    FOCUS_BORDER = QColor("#0a84ff")
    SECTION_BORDER = QColor("#333333")

    # Scrollbar Colors
    SCROLLBAR_TRACK = QColor(0, 0, 0, 0)  # transparent
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
        palette.setColor(
            QPalette.ColorGroup.Disabled,
            QPalette.ColorRole.WindowText,
            cls.SECONDARY_TEXT,
        )
        palette.setColor(
            QPalette.ColorGroup.Disabled, QPalette.ColorRole.Text, cls.SECONDARY_TEXT
        )
        palette.setColor(
            QPalette.ColorGroup.Disabled,
            QPalette.ColorRole.ButtonText,
            cls.SECONDARY_TEXT,
        )

        return palette


class Dimensions:
    """Exact dimensions from the original HTML/CSS design."""

    # Main Window
    WINDOW_WIDTH = 1080
    WINDOW_HEIGHT = 720
    WINDOW_RADIUS = 12
    # CSS-like shadow params (0 22px 70px 4px rgba(0,0,0,0.56))
    SHADOW_OFFSET_X = 0
    SHADOW_OFFSET_Y = 0
    SHADOW_BLUR = 24
    SHADOW_SPREAD = -1
    SHADOW_ALPHA = 0.04

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
    """Font specifications following web/CSS standard naming conventions."""
    import platform
    
    # macOS fonts render larger/bolder, so we adjust sizes
    is_macos = platform.system() == 'Darwin'
    
    # Standard typography scale (adjusted for macOS)
    # Primary headings (app name, major titles)
    H1 = 22 if is_macos else 24
    # Secondary headings
    H2 = 17 if is_macos else 18
    # Page titles, tertiary headings
    H3 = 13 if is_macos else 14
    # Section headings
    H4 = 11 if is_macos else 10
    # Subsection headings
    H5 = 12 if is_macos else 11
    # Minor headings
    H6 = 11 if is_macos else 10

    # Body text hierarchy
    # Large body text
    LG = 13 if is_macos else 12
    # Default body text, controls, buttons
    BASE = 11 if is_macos else 10
    # Small text, captions, descriptions
    SM = 10 if is_macos else 9
    # Extra small text, fine print
    XS = 9 if is_macos else 8

    # Legacy aliases (for backward compatibility - remove after migration)
    # 24px - App name, major headings
    H1_SIZE = H1
    # 14px - Main page titles
    PAGE_TITLE_SIZE = H3
    # 10px - Section headings within pages
    SECTION_TITLE_SIZE = H6
    # 11px - Setting names, card titles, item labels
    ITEM_TITLE_SIZE = H5
    # 10px - Buttons, inputs, form controls
    CONTROL_SIZE = BASE + 2
    # 9px - Descriptions, body text, status text
    BODY_SIZE = SM
    # 11px - Navigation menu items
    NAVIGATION_SIZE = H5
    # 12px - Logo/branding text
    LOGO_SIZE = H4
    # 11px - Link buttons and clickable text
    LINK_SIZE = H5
    # 10px - for UI controls
    BASE_SIZE = BASE
    # 11px - item/setting titles
    SETTING_LABEL_SIZE = H5
    # 9px - descriptions and body text
    DESCRIPTION_SIZE = SM
    # 9px - status text
    STATUS_SIZE = SM

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

    @classmethod
    def get_platform_font_scale(cls) -> float:
        """Get platform-specific font scaling factor."""
        import platform

        system = platform.system().lower()

        if system == "darwin":  # macOS - needs larger fonts for Retina displays
            return 1.4
        elif system == "windows":
            return 1.2
        else:  # Linux and others
            return 1.0

    @classmethod
    def scaled(cls, size: int) -> int:
        """Get platform-scaled font size."""
        return int(size * cls.get_platform_font_scale())

    @classmethod
    def adjust_weight(cls, weight) -> "QFont.Weight":
        """Adjust font weight for platform-specific rendering differences."""
        import platform
        from PySide6.QtGui import QFont

        system = platform.system().lower()

        if system == "darwin":  # macOS - reduce weight by ~100 (one step lighter)
            weight_map = {
                QFont.Weight.Thin: QFont.Weight.Thin,  # Already lightest
                QFont.Weight.ExtraLight: QFont.Weight.Thin,
                QFont.Weight.Light: QFont.Weight.ExtraLight,
                QFont.Weight.Normal: QFont.Weight.Normal,
                QFont.Weight.Medium: QFont.Weight.Normal,
                QFont.Weight.DemiBold: QFont.Weight.Medium,
                QFont.Weight.Bold: QFont.Weight.DemiBold,
                QFont.Weight.ExtraBold: QFont.Weight.Bold,
                QFont.Weight.Black: QFont.Weight.ExtraBold,
            }
            return weight_map.get(weight, weight)
        else:
            return weight  # No adjustment for Windows/Linux


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
    SECTION_GAP = 16

    # Page spacing
    PAGE_PADDING = 24

    # Component spacing
    CARD_GAP = 16
    CARD_PADDING = 20
    FORM_ROW_SPACING = 12
