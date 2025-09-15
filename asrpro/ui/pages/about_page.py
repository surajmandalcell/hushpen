"""About page with application information."""

from PySide6.QtCore import Qt, QUrl
from PySide6.QtGui import QFont, QPainter, QDesktopServices, QPixmap
from PySide6.QtWidgets import (
    QWidget,
    QVBoxLayout,
    QHBoxLayout,
    QLabel,
    QPushButton,
    QSizePolicy,
)
import os

from ..layouts.setting_row import SettingRow
from ..styles.dark_theme import DarkTheme, Dimensions, Fonts, Spacing
from ..utils.icon_loader import IconLoader
from ..utils.invert import invert_icon
from .base_page import BasePage


class InfoCard(QWidget):
    """Card widget for displaying application information."""

    def __init__(self, title: str, content: str, parent=None):
        super().__init__(parent)

        self.title = title
        self.content = content

        self._setup_ui()
        self._apply_styles()

    def _setup_ui(self):
        """Set up the info card layout."""
        layout = QVBoxLayout(self)
        layout.setContentsMargins(
            Spacing.CARD_PADDING,
            Spacing.CARD_PADDING,
            Spacing.CARD_PADDING,
            Spacing.CARD_PADDING,
        )
        layout.setSpacing(8)

        # Title
        self.title_label = QLabel(self.title)
        font = QFont()
        font.setPointSize(Fonts.scaled(Fonts.SETTING_LABEL_SIZE))
        font.setWeight(Fonts.adjust_weight(Fonts.MEDIUM))
        self.title_label.setFont(font)
        layout.addWidget(self.title_label)

        # Content
        self.content_label = QLabel(self.content)
        content_font = QFont()
        content_font.setPointSize(Fonts.scaled(Fonts.DESCRIPTION_SIZE))
        content_font.setWeight(Fonts.adjust_weight(Fonts.NORMAL))
        self.content_label.setFont(content_font)
        self.content_label.setWordWrap(True)
        self.content_label.setTextInteractionFlags(
            Qt.TextInteractionFlag.TextSelectableByMouse
        )
        layout.addWidget(self.content_label)

    def _apply_styles(self):
        """Apply info card styling."""
        self.setStyleSheet(
            f"""
            InfoCard {{
                background-color: {DarkTheme.CARD_BG.name()};
                border: 1px solid {DarkTheme.CARD_BORDER.name()};
                border-radius: {Dimensions.CARD_RADIUS}px;
            }}
        """
        )

        self.title_label.setStyleSheet(f"color: {DarkTheme.PRIMARY_TEXT.name()};")
        self.content_label.setStyleSheet(f"color: {DarkTheme.SECONDARY_TEXT.name()};")


class LinkButton(QPushButton):
    """Styled button that looks like a link."""

    def __init__(self, text: str, url: str, parent=None):
        super().__init__(text, parent)

        self.url = url
        self.clicked.connect(self._open_url)
        self.setCursor(Qt.CursorShape.PointingHandCursor)
        self.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        self._apply_styles()

    def _open_url(self):
        """Open the URL in the default browser."""
        QDesktopServices.openUrl(QUrl(self.url))

    def _apply_styles(self):
        """Apply link button styling."""
        self.setStyleSheet(
            f"""
            LinkButton {{
                background-color: {DarkTheme.CARD_BORDER.name()};
                color: {DarkTheme.SECONDARY_TEXT.name()};
                border: 1px solid {DarkTheme.CARD_BORDER.name()};
                text-align: left;
                padding: 4px 12px;
                font-size: {Fonts.CONTROL_SIZE + 1}px;
                font-weight: 500;
                border-radius: 6px;
                min-height: 20px;
            }}
            LinkButton:hover {{
                background-color: {DarkTheme.ACCENT_BLUE.name()};
                color: {DarkTheme.MAIN_BG.name()};
                border-color: {DarkTheme.ACCENT_BLUE.name()};
            }}
            LinkButton:pressed {{
                background-color: {DarkTheme.ACCENT_BLUE.darker(110).name()};
                color: {DarkTheme.MAIN_BG.name()};
                border-color: {DarkTheme.ACCENT_BLUE.darker(110).name()};
            }}
        """
        )


class AboutPage(BasePage):
    """About page with application information and credits."""

    def __init__(self, parent=None):
        super().__init__("About ASR Pro", parent)
        self._create_content()

    def _create_content(self):
        """Create about page content."""
        # App Logo and Name section
        header_container = QWidget()
        header_layout = QHBoxLayout(header_container)
        header_layout.setContentsMargins(0, 0, 0, 24)
        header_layout.setSpacing(16)

        # App icon
        icon_label = QLabel()
        icon_label.setFixedSize(64, 64)
        icon_label.setAlignment(Qt.AlignmentFlag.AlignCenter)

        # Load larger app icon - try PNG first, fallback to mic icon
        try:
            from PySide6.QtGui import QPixmap, QPainter
            import os

            icon_path = os.path.join(
                os.path.dirname(__file__), "..", "..", "..", "assets", "icon.png"
            )
            if os.path.exists(icon_path):
                pixmap = QPixmap(icon_path)
                if not pixmap.isNull():
                    # Scale the pixmap
                    scaled_pixmap = pixmap.scaled(
                        64,
                        64,
                        Qt.AspectRatioMode.KeepAspectRatio,
                        Qt.TransformationMode.SmoothTransformation,
                    )
                    # Invert colors for dark theme
                    inverted_pixmap = invert_icon(scaled_pixmap)
                    icon_label.setPixmap(inverted_pixmap)
                else:
                    raise FileNotFoundError()
            else:
                raise FileNotFoundError()
        except:
            # Fallback to mic icon
            icon = IconLoader.load_icon("mic", 64, DarkTheme.ACCENT_BLUE.name())
            icon_label.setPixmap(icon.pixmap(64, 64))
        header_layout.addWidget(icon_label)

        # App info
        info_layout = QVBoxLayout()
        info_layout.setSpacing(4)

        # App name
        app_name = QLabel("ASR Pro")
        name_font = QFont()
        name_font.setPointSize(Fonts.scaled(Fonts.H1))
        name_font.setWeight(Fonts.adjust_weight(Fonts.BOLD))
        app_name.setFont(name_font)
        app_name.setStyleSheet(f"color: {DarkTheme.PRIMARY_TEXT.name()};")
        info_layout.addWidget(app_name)

        # Description
        desc_label = QLabel("Professional Speech Recognition and Transcription")
        desc_font = QFont()
        desc_font.setPointSize(Fonts.scaled(Fonts.CONTROL_SIZE))
        desc_font.setWeight(Fonts.adjust_weight(Fonts.NORMAL))
        desc_label.setFont(desc_font)
        desc_label.setStyleSheet(f"color: {DarkTheme.SECONDARY_TEXT.name()};")
        info_layout.addWidget(desc_label)

        header_layout.addLayout(info_layout, 1)
        self.add_content_widget(header_container)

        # Application Information
        app_info = InfoCard(
            "Application Information",
            "ASR Pro is a professional speech recognition and transcription application built with Python and PySide6. "
            "It uses OpenAI's Whisper models for accurate speech-to-text conversion with support for multiple languages "
            "and real-time processing capabilities.",
        )
        self.add_content_widget(app_info)

        # Developer info
        developer_text = "Developed by Suraj Mandal"

        developer_info = InfoCard("Developer", developer_text)
        self.add_content_widget(developer_info)

        # Links section - cleaner styling
        links_container = QWidget()
        links_layout = QVBoxLayout(links_container)
        links_layout.setContentsMargins(
            Spacing.CARD_PADDING,
            Spacing.CARD_PADDING,
            Spacing.CARD_PADDING,
            Spacing.CARD_PADDING,
        )
        links_layout.setSpacing(12)

        # Links title
        links_title = QLabel("Links & Resources")
        links_font = QFont()
        links_font.setPointSize(Fonts.SETTING_LABEL_SIZE)
        links_font.setWeight(Fonts.adjust_weight(Fonts.MEDIUM))
        links_title.setFont(links_font)
        links_title.setStyleSheet(
            f"color: {DarkTheme.PRIMARY_TEXT.name()}; margin-bottom: 8px;"
        )
        links_layout.addWidget(links_title)

        # Links in horizontal layout
        links_row_layout = QHBoxLayout()
        links_row_layout.setSpacing(12)

        # GitHub link
        github_link = LinkButton(
            "GitHub Repository", "https://github.com/surajmandalcell/asrpro"
        )
        links_row_layout.addWidget(github_link)

        # Contact link
        contact_link = LinkButton("Contact Developer", "mailto:me@mandalsuraj.com")
        links_row_layout.addWidget(contact_link)

        # Add stretch to keep buttons at content size
        links_row_layout.addStretch()

        links_layout.addLayout(links_row_layout)

        # Style the links container - remove border, just background
        links_container.setStyleSheet(
            f"""
            QWidget {{
                background-color: {DarkTheme.CARD_BG.name()};
                border-radius: {Dimensions.CARD_RADIUS}px;
            }}
        """
        )

        self.add_content_widget(links_container)

        # Add stretch to push content to top
        self.add_stretch()
