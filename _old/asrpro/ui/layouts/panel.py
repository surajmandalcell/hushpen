from PySide6.QtWidgets import QWidget
from ..styles.dark_theme import DarkTheme


class Panel(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setStyleSheet(
            f"""
            Panel {{
                background-color: {DarkTheme.CARD_BG.name()};
                border: 1px solid {DarkTheme.CARD_BORDER.name()};
                border-radius: 10px;
            }}
            """
        )
