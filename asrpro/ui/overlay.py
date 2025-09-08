"""Recording overlay floating widget with fade in/out animations.

This recreates the previously implemented Overlay that was accidentally
emptied. It shows a small semi–transparent pill in the center/top area
while recording so the user has visual feedback of the active hotkey
session without stealing window focus.
"""

from __future__ import annotations

from PySide6.QtCore import Qt, QPropertyAnimation, QEasingCurve, QEvent
from PySide6.QtWidgets import QWidget, QLabel, QVBoxLayout, QHBoxLayout


class Overlay(QWidget):  # pragma: no cover - UI only
    def __init__(self):
        super().__init__()
        # Window flags (names under Qt.WindowType) but exposed at Qt namespace for convenience
        self.setWindowFlags(
            Qt.WindowType.FramelessWindowHint
            | Qt.WindowType.Tool
            | Qt.WindowType.WindowStaysOnTopHint
        )
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground, True)
        self.setAttribute(Qt.WidgetAttribute.WA_ShowWithoutActivating, True)
        self.setFocusPolicy(Qt.FocusPolicy.NoFocus)
        self._build()
        self._anim = QPropertyAnimation(self, b"windowOpacity", self)
        self._anim.setDuration(180)
        self._anim.setEasingCurve(QEasingCurve.Type.InOutQuad)

    def _build(self):
        box = QVBoxLayout(self)
        box.setContentsMargins(24, 16, 24, 16)
        box.setSpacing(8)

        # Modern recording indicator with pulse effect
        indicator_layout = QHBoxLayout()
        indicator_layout.setSpacing(12)

        # Animated recording dot
        self.status_dot = QLabel("🔴")
        self.status_dot.setStyleSheet(
            "QLabel { "
            "color: rgba(239, 68, 68, 1); "
            "font-size: 20px; "
            "font-weight: bold; "
            "}"
        )
        self.status_dot.setAlignment(Qt.AlignmentFlag.AlignCenter)
        indicator_layout.addWidget(self.status_dot)

        self.label = QLabel("Recording in progress...")
        self.label.setStyleSheet(
            "QLabel { "
            "color: white; "
            "font-size: 16px; "
            "font-weight: 600; "
            "letter-spacing: 0.3px; "
            "margin: 0px;"
            "}"
        )
        self.label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        indicator_layout.addWidget(self.label)

        box.addLayout(indicator_layout)

        # Subtle instruction text
        instruction = QLabel("Press hotkey again to stop")
        instruction.setStyleSheet(
            "QLabel { "
            "color: rgba(255, 255, 255, 0.8); "
            "font-size: 12px; "
            "font-weight: 400; "
            "margin-top: 4px; "
            "}"
        )
        instruction.setAlignment(Qt.AlignmentFlag.AlignCenter)
        box.addWidget(instruction)

        self.setStyleSheet(
            """
            QWidget { 
                background: qlineargradient(x1:0, y1:0, x2:1, y2:0, 
                    stop:0 rgba(26, 26, 46, 0.95), 
                    stop:1 rgba(22, 33, 62, 0.95)); 
                border-radius: 16px; 
                border: 1px solid rgba(255, 255, 255, 0.2);
            }
            """
        )
        self.resize(280, 90)

    # Public API ---------------------------------------------------------
    def show_smooth(self):
        self._place()
        self.setWindowOpacity(0.0)
        super().show()
        self.raise_()
        self._anim.stop()
        self._anim.setStartValue(0.0)
        self._anim.setEndValue(1.0)
        self._anim.start()

    def close_smooth(self):
        if not self.isVisible():
            return
        self._anim.stop()
        self._anim.setStartValue(self.windowOpacity())
        self._anim.setEndValue(0.0)
        self._anim.finished.connect(self._finish_close)
        self._anim.start()

    # Helpers ------------------------------------------------------------
    def _finish_close(self):
        try:
            self._anim.finished.disconnect(self._finish_close)
        except Exception:
            pass
        self.hide()

    def _place(self):  # Center top third of primary screen
        screen = self.screen() or (
            self.windowHandle().screen() if self.windowHandle() else None
        )
        if not screen:
            return
        g = screen.availableGeometry()
        x = g.x() + (g.width() - self.width()) // 2
        y = g.y() + int(g.height() * 0.18)
        self.move(x, y)

    # Overridden so it never steals focus when shown
    def event(self, e):  # type: ignore
        if e.type() in (QEvent.Type.Show, QEvent.Type.WindowActivate):
            self.setFocus(Qt.FocusReason.NoFocusReason)
        return super().event(e)
