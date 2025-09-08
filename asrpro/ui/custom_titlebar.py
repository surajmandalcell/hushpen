"""Custom title bar for frameless 800x600 window."""

from __future__ import annotations
from PySide6.QtCore import Qt, QPoint
from PySide6.QtWidgets import QWidget, QLabel, QHBoxLayout, QPushButton


class TitleBar(QWidget):  # pragma: no cover
    def __init__(self, parent=None):
        super().__init__(parent)
        self._mouse_pos = QPoint()
        self.setFixedHeight(50)
        self.setObjectName("TitleBar")

        layout = QHBoxLayout(self)
        layout.setContentsMargins(24, 0, 16, 0)
        layout.setSpacing(16)

        # Modern app icon/logo
        self.icon_label = QLabel("🎵")
        self.icon_label.setStyleSheet(
            "QLabel { "
            "color: rgba(99, 102, 241, 1); "
            "font-size: 20px; "
            "font-weight: bold; "
            "}"
        )
        layout.addWidget(self.icon_label)

        self.title_label = QLabel("asrpro")
        self.title_label.setObjectName("TitleLabel")
        self.title_label.setStyleSheet(
            "#TitleLabel { "
            "color: white; "
            "font-size: 18px; "
            "font-weight: 700; "
            "letter-spacing: 0.5px; "
            "}"
        )
        layout.addWidget(self.title_label)

        # Version/status indicator
        self.status_label = QLabel("AI Speech Recognition")
        self.status_label.setStyleSheet(
            "QLabel { "
            "color: rgba(255, 255, 255, 0.6); "
            "font-size: 12px; "
            "font-weight: 400; "
            "}"
        )
        layout.addWidget(self.status_label)

        layout.addStretch(1)

        # Modern minimize button
        self.btn_minimize = QPushButton("−")
        self.btn_minimize.setFixedSize(40, 32)
        self.btn_minimize.setStyleSheet(
            "QPushButton { "
            "background: rgba(255, 255, 255, 0.1); "
            "border: 1px solid rgba(255, 255, 255, 0.1); "
            "border-radius: 8px; "
            "font-size: 18px; "
            "color: rgba(255, 255, 255, 0.8); "
            "font-weight: 300; "
            "} "
            "QPushButton:hover { "
            "background: rgba(255, 255, 255, 0.15); "
            "color: white; "
            "}"
        )
        self.btn_minimize.clicked.connect(self._on_minimize)
        layout.addWidget(self.btn_minimize)

        # Modern close button
        self.btn_close = QPushButton("×")
        self.btn_close.setFixedSize(40, 32)
        self.btn_close.setStyleSheet(
            "QPushButton { "
            "background: rgba(255, 255, 255, 0.1); "
            "border: 1px solid rgba(255, 255, 255, 0.1); "
            "border-radius: 8px; "
            "font-size: 18px; "
            "color: rgba(255, 255, 255, 0.8); "
            "font-weight: 300; "
            "} "
            "QPushButton:hover { "
            "background: rgba(239, 68, 68, 0.8); "
            "color: white; "
            "border: 1px solid rgba(255, 255, 255, 0.2); "
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
