"""About page with application information."""

from PySide6.QtCore import Qt, QUrl
from PySide6.QtGui import QFont, QPainter, QDesktopServices
from PySide6.QtWidgets import QWidget, QVBoxLayout, QHBoxLayout, QLabel, QPushButton

from ..components.setting_row import SettingRow
from ..styles.dark_theme import DarkTheme, Dimensions, Fonts, Spacing
from ..utils.icon_loader import IconLoader
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
        layout.setContentsMargins(Spacing.CARD_PADDING, Spacing.CARD_PADDING,
                                 Spacing.CARD_PADDING, Spacing.CARD_PADDING)
        layout.setSpacing(8)
        
        # Title
        self.title_label = QLabel(self.title)
        font = QFont()
        font.setPointSize(Fonts.SETTING_LABEL_SIZE)
        font.setWeight(Fonts.MEDIUM)
        self.title_label.setFont(font)
        layout.addWidget(self.title_label)
        
        # Content
        self.content_label = QLabel(self.content)
        content_font = QFont()
        content_font.setPointSize(Fonts.BASE_SIZE)
        content_font.setWeight(Fonts.NORMAL)
        self.content_label.setFont(content_font)
        self.content_label.setWordWrap(True)
        self.content_label.setTextInteractionFlags(Qt.TextInteractionFlag.TextSelectableByMouse)
        layout.addWidget(self.content_label)
    
    def _apply_styles(self):
        """Apply info card styling."""
        self.setStyleSheet(f"""
            InfoCard {{
                background-color: {DarkTheme.CARD_BG.name()};
                border: 1px solid {DarkTheme.CARD_BORDER.name()};
                border-radius: {Dimensions.CARD_RADIUS}px;
            }}
        """)
        
        self.title_label.setStyleSheet(f"color: {DarkTheme.PRIMARY_TEXT.name()};")
        self.content_label.setStyleSheet(f"color: {DarkTheme.SECONDARY_TEXT.name()};")


class LinkButton(QPushButton):
    """Styled button that looks like a link."""
    
    def __init__(self, text: str, url: str, parent=None):
        super().__init__(text, parent)
        
        self.url = url
        self.clicked.connect(self._open_url)
        self._apply_styles()
    
    def _open_url(self):
        """Open the URL in the default browser."""
        QDesktopServices.openUrl(QUrl(self.url))
    
    def _apply_styles(self):
        """Apply link button styling."""
        self.setStyleSheet(f"""
            LinkButton {{
                background-color: transparent;
                color: {DarkTheme.ACCENT_BLUE.name()};
                border: none;
                text-decoration: underline;
                text-align: left;
                padding: 4px 0px;
                font-size: {Fonts.BASE_SIZE}px;
            }}
            LinkButton:hover {{
                color: {DarkTheme.ACCENT_BLUE.lighter(120).name()};
            }}
            LinkButton:pressed {{
                color: {DarkTheme.ACCENT_BLUE.darker(120).name()};
            }}
        """)


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
        
        # Load larger app icon
        icon = IconLoader.load_icon("mic", 64, DarkTheme.ACCENT_BLUE.name())
        icon_label.setPixmap(icon.pixmap(64, 64))
        header_layout.addWidget(icon_label)
        
        # App info
        info_layout = QVBoxLayout()
        info_layout.setSpacing(4)
        
        # App name
        app_name = QLabel("ASR Pro")
        name_font = QFont()
        name_font.setPointSize(24)
        name_font.setWeight(Fonts.BOLD)
        app_name.setFont(name_font)
        app_name.setStyleSheet(f"color: {DarkTheme.PRIMARY_TEXT.name()};")
        info_layout.addWidget(app_name)
        
        # Version
        version_label = QLabel("Version 1.0.0")
        version_font = QFont()
        version_font.setPointSize(Fonts.BASE_SIZE)
        version_font.setWeight(Fonts.NORMAL)
        version_label.setFont(version_font)
        version_label.setStyleSheet(f"color: {DarkTheme.SECONDARY_TEXT.name()};")
        info_layout.addWidget(version_label)
        
        # Description
        desc_label = QLabel("Professional Speech Recognition and Transcription")
        desc_font = QFont()
        desc_font.setPointSize(Fonts.BASE_SIZE)
        desc_font.setWeight(Fonts.NORMAL)
        desc_label.setFont(desc_font)
        desc_label.setStyleSheet(f"color: {DarkTheme.SECONDARY_TEXT.name()};")
        info_layout.addWidget(desc_label)
        
        header_layout.addLayout(info_layout, 1)
        self.add_content_widget(header_container)
        
        # Application Information
        app_info = InfoCard(
            "Application Information",
            "ASR Pro is a professional speech recognition and transcription application built with Python and PyQt6. "
            "It uses OpenAI's Whisper models for accurate speech-to-text conversion with support for multiple languages "
            "and real-time processing capabilities."
        )
        self.add_content_widget(app_info)
        
        # System Information
        import platform
        import sys
        system_info_text = f"""
Platform: {platform.system()} {platform.release()}
Python Version: {sys.version.split()[0]}
PyQt Version: 6.x
Architecture: {platform.machine()}
        """.strip()
        
        system_info = InfoCard("System Information", system_info_text)
        self.add_content_widget(system_info)
        
        # Features
        features_text = """
• Real-time speech recognition
• Multiple language support
• Offline processing with local models
• Global hotkey support
• Customizable text processing
• Audio device management
• Cross-platform compatibility
        """.strip()
        
        features_info = InfoCard("Key Features", features_text)
        self.add_content_widget(features_info)
        
        # Credits and Acknowledgments
        credits_text = """
Built with Python, PyQt6, and OpenAI Whisper
Special thanks to the open-source community
Icons provided by Lucide Icons
        """.strip()
        
        credits_info = InfoCard("Credits & Acknowledgments", credits_text)
        self.add_content_widget(credits_info)
        
        # Links section
        links_container = QWidget()
        links_layout = QVBoxLayout(links_container)
        links_layout.setContentsMargins(Spacing.CARD_PADDING, Spacing.CARD_PADDING,
                                       Spacing.CARD_PADDING, Spacing.CARD_PADDING)
        links_layout.setSpacing(8)
        
        # Links title
        links_title = QLabel("Links & Resources")
        links_font = QFont()
        links_font.setPointSize(Fonts.SETTING_LABEL_SIZE)
        links_font.setWeight(Fonts.MEDIUM)
        links_title.setFont(links_font)
        links_title.setStyleSheet(f"color: {DarkTheme.PRIMARY_TEXT.name()};")
        links_layout.addWidget(links_title)
        
        # Website link
        website_link = LinkButton("Visit Website", "https://example.com")
        links_layout.addWidget(website_link)
        
        # GitHub link
        github_link = LinkButton("View on GitHub", "https://github.com/example/asrpro")
        links_layout.addWidget(github_link)
        
        # Documentation link
        docs_link = LinkButton("Documentation", "https://docs.example.com")
        links_layout.addWidget(docs_link)
        
        # Support link
        support_link = LinkButton("Support & Bug Reports", "https://github.com/example/asrpro/issues")
        links_layout.addWidget(support_link)
        
        # Style the links container
        links_container.setStyleSheet(f"""
            QWidget {{
                background-color: {DarkTheme.CARD_BG.name()};
                border: 1px solid {DarkTheme.CARD_BORDER.name()};
                border-radius: {Dimensions.CARD_RADIUS}px;
            }}
        """)
        
        self.add_content_widget(links_container)
        
        # License information
        license_text = """
MIT License

Copyright (c) 2024 ASR Pro

Permission is hereby granted, free of charge, to any person obtaining a copy of this software 
and associated documentation files, to deal in the Software without restriction, including 
without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, 
and/or sell copies of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND.
        """.strip()
        
        license_info = InfoCard("License", license_text)
        self.add_content_widget(license_info)
        
        # Add stretch to push content to top
        self.add_stretch()