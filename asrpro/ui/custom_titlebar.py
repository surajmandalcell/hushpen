"""Custom title bar for frameless 800x600 window."""

from __future__ import annotations
from PySide6.QtCore import Qt, QPoint
from PySide6.QtWidgets import QWidget, QLabel, QHBoxLayout, QPushButton


class TitleBar(QWidget):  # pragma: no cover
    def __init__(self, parent=None):
        super().__init__(parent)
        self._mouse_pos = QPoint()
        self.setFixedHeight(34)
        self.setObjectName("TitleBar")
        layout = QHBoxLayout(self)
        layout.setContentsMargins(8, 0, 8, 0)
        self.title_label = QLabel("asrpro")
        self.title_label.setObjectName("TitleLabel")
        layout.addWidget(self.title_label)
        layout.addStretch(1)
        self.btn_close = QPushButton("✕")
        self.btn_close.setFixedSize(28, 24)
        self.btn_close.clicked.connect(self._on_close)
        layout.addWidget(self.btn_close)

    def _on_close(self):
        self.window().hide()

    def mousePressEvent(self, event):
        if event.button() == Qt.LeftButton:
            self._mouse_pos = event.globalPosition().toPoint()
            self._frame_pos = self.window().frameGeometry().topLeft()

    def mouseMoveEvent(self, event):
        if event.buttons() & Qt.LeftButton:
            diff = event.globalPosition().toPoint() - self._mouse_pos
            self.window().move(self._frame_pos + diff)
