from PySide6.QtWidgets import QLabel
from PySide6.QtGui import QFont
from ..styles.dark_theme import DarkTheme, Fonts


class TitleLabel(QLabel):
    def __init__(self, text: str = "", parent=None):
        super().__init__(text, parent)
        f = QFont()
        f.setPointSize(Fonts.scaled(Fonts.PAGE_TITLE_SIZE))
        f.setWeight(Fonts.adjust_weight(Fonts.MEDIUM))
        self.setFont(f)
        self.setStyleSheet(f"color: {DarkTheme.PRIMARY_TEXT.name()};")


class SectionLabel(QLabel):
    def __init__(self, text: str = "", parent=None):
        super().__init__(text, parent)
        f = QFont()
        f.setPointSize(Fonts.scaled(Fonts.SECTION_TITLE_SIZE))
        f.setWeight(Fonts.adjust_weight(Fonts.MEDIUM))
        self.setFont(f)
        self.setStyleSheet(f"color: {DarkTheme.PRIMARY_TEXT.name()};")


class BodyLabel(QLabel):
    def __init__(self, text: str = "", parent=None):
        super().__init__(text, parent)
        f = QFont()
        f.setPointSize(Fonts.scaled(Fonts.DESCRIPTION_SIZE))
        f.setWeight(Fonts.adjust_weight(Fonts.NORMAL))
        self.setFont(f)
        self.setStyleSheet(f"color: {DarkTheme.SECONDARY_TEXT.name()};")
