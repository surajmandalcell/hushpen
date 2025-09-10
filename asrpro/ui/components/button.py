from PySide6.QtWidgets import QPushButton
from ..styles.dark_theme import DarkTheme, Fonts


class MacButton(QPushButton):
    def __init__(self, text: str = "", parent=None, *, primary: bool = True):
        super().__init__(text, parent)
        self.primary = primary
        self._apply_style()

    def _apply_style(self):
        base_bg = DarkTheme.ACCENT_BLUE.name() if self.primary else DarkTheme.BUTTON_BG.name()
        hover_bg = DarkTheme.ACCENT_BLUE.name() if self.primary else DarkTheme.BUTTON_HOVER_BG.name()
        fg = "#ffffff" if self.primary else DarkTheme.PRIMARY_TEXT.name()
        self.setStyleSheet(
            f"""
            QPushButton {{
                background-color: {base_bg};
                color: {fg};
                border: none;
                padding: 6px 12px;
                border-radius: 8px;
                font-size: {Fonts.BASE_SIZE}px;
            }}
            QPushButton:hover {{
                background-color: {hover_bg};
                filter: brightness(1.02);
            }}
            QPushButton:pressed {{
                transform: translateY(1px);
                filter: brightness(0.96);
            }}
            """
        )
