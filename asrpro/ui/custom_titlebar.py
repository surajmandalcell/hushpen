"""Custom title bar for frameless 800x600 window."""

from __future__ import annotations
from PySide6.QtCore import Qt, QPoint
from PySide6.QtWidgets import QWidget, QLabel, QHBoxLayout, QPushButton


class TitleBar(QWidget):  # pragma: no cover
    def __init__(self, parent=None):
        super().__init__(parent)
        self._mouse_pos = QPoint()
        self.setFixedHeight(40)
        self.setObjectName("TitleBar")

        layout = QHBoxLayout(self)
        layout.setContentsMargins(16, 0, 12, 0)
        layout.setSpacing(12)

        # App icon/logo (optional)
        self.icon_label = QLabel("⬤")
        self.icon_label.setStyleSheet(
            "QLabel { color: #2ea043; font-size: 12px; font-weight: bold; }"
        )
        layout.addWidget(self.icon_label)

        self.title_label = QLabel("asrpro")
        self.title_label.setObjectName("TitleLabel")
        self.title_label.setStyleSheet(
            "#TitleLabel { "
            "color: #f0f6fc; "
            "font-size: 16px; "
            "font-weight: 600; "
            "}"
        )
        layout.addWidget(self.title_label)

        layout.addStretch(1)

        # Minimize button (hide window)
        self.btn_minimize = QPushButton("⎼")
        self.btn_minimize.setFixedSize(32, 28)
        self.btn_minimize.setStyleSheet(
            "QPushButton { "
            "background: transparent; "
            "border: none; "
            "border-radius: 4px; "
            "font-size: 16px; "
            "color: #6e7681; "
            "font-weight: bold; "
            "} "
            "QPushButton:hover { "
            "background: #30363d; "
            "color: #f0f6fc; "
            "}"
        )
        self.btn_minimize.clicked.connect(self._on_minimize)
        layout.addWidget(self.btn_minimize)

        self.btn_close = QPushButton("✕")
        self.btn_close.setFixedSize(32, 28)
        self.btn_close.setStyleSheet(
            "QPushButton { "
            "background: transparent; "
            "border: none; "
            "border-radius: 4px; "
            "font-size: 14px; "
            "color: #6e7681; "
            "font-weight: bold; "
            "} "
            "QPushButton:hover { "
            "background: #da3633; "
            "color: white; "
            "}"
        )
        self.btn_close.clicked.connect(self._on_close)
        layout.addWidget(self.btn_close)

    def _on_minimize(self):
        self.window().hide()

    def _on_close(self):
        self.window().hide()

    def mousePressEvent(self, event):
        if event.button() == Qt.MouseButton.LeftButton:
            self._mouse_pos = event.globalPosition().toPoint()
            self._frame_pos = self.window().frameGeometry().topLeft()

    def mouseMoveEvent(self, event):
        if event.buttons() & Qt.MouseButton.LeftButton:
            diff = event.globalPosition().toPoint() - self._mouse_pos
            self.window().move(self._frame_pos + diff)
