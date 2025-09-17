from pathlib import Path
from PySide6.QtWidgets import QWidget, QHBoxLayout, QLineEdit, QPushButton, QFileDialog
from ..styles.dark_theme import DarkTheme, Fonts


class FilePicker(QWidget):
    def __init__(self, parent=None, *, filter: str = "*", placeholder: str = "Choose file"):
        super().__init__(parent)
        self._filter = filter
        self._path: Path | None = None
        self._input = QLineEdit()
        self._input.setPlaceholderText(placeholder)
        self._input.setReadOnly(True)
        self._browse = QPushButton("Browse")
        lay = QHBoxLayout(self)
        lay.setContentsMargins(0, 0, 0, 0)
        lay.setSpacing(8)
        lay.addWidget(self._input, 1)
        lay.addWidget(self._browse)
        self._style()
        self._browse.clicked.connect(self._on_browse)

    def _style(self):
        self._input.setStyleSheet(
            f"""
            QLineEdit {{
                background-color: {DarkTheme.CONTROL_BG.name()};
                color: {DarkTheme.PRIMARY_TEXT.name()};
                border: none;
                padding: 8px 12px;
                border-radius: 8px;
                font-size: {Fonts.CONTROL_SIZE}px;
            }}
            """
        )
        self._browse.setStyleSheet(
            f"""
            QPushButton {{
                background-color: {DarkTheme.BUTTON_BG.name()};
                color: {DarkTheme.PRIMARY_TEXT.name()};
                border: none;
                padding: 8px 12px;
                border-radius: 8px;
                font-size: {Fonts.CONTROL_SIZE}px;
            }}
            QPushButton:hover {{ background-color: {DarkTheme.BUTTON_HOVER_BG.name()}; }}
            QPushButton:pressed {{ padding-top: 9px; padding-bottom: 7px; }}
            """
        )

    def path(self) -> Path | None:
        return self._path

    def _on_browse(self):
        file, _ = QFileDialog.getOpenFileName(self, "Select File", "", self._filter)
        if file:
            self._path = Path(file)
            self._input.setText(str(self._path))
