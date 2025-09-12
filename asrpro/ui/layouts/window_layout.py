from __future__ import annotations

from PySide6.QtCore import QRect
from PySide6.QtGui import QPainterPath, QRegion
from PySide6.QtWidgets import QWidget, QHBoxLayout

from ..styles.dark_theme import Dimensions


def compute_shadow_margins() -> tuple[int, int, int, int]:
    """Compute directional margins to reserve space for the painted shadow."""
    blur = Dimensions.SHADOW_BLUR
    spread = Dimensions.SHADOW_SPREAD
    offx = Dimensions.SHADOW_OFFSET_X
    offy = Dimensions.SHADOW_OFFSET_Y
    left = blur + spread + max(0, -offx)
    right = blur + spread + max(0, offx)
    top = blur + spread + max(0, -offy)
    bottom = blur + spread + max(0, offy)
    return left, top, right, bottom


def apply_rounded_clip(widget: QWidget, radius: int) -> None:
    """Apply a rounded mask to a widget to clip its children."""
    rect: QRect = widget.rect()
    if rect.isEmpty():
        return
    path = QPainterPath()
    path.addRoundedRect(rect, radius, radius)
    region = QRegion(path.toFillPolygon().toPolygon())
    widget.setMask(region)


def build_root_and_frame(parent: QWidget) -> tuple[QHBoxLayout, QWidget, QHBoxLayout]:
    """Create root layout with shadow margins and an inner frame with zero margins.

    Returns (root_layout, frame_widget, frame_layout).
    """
    root_layout = QHBoxLayout(parent)
    left, top, right, bottom = compute_shadow_margins()
    root_layout.setContentsMargins(left, top, right, bottom)
    root_layout.setSpacing(0)

    frame = QWidget(parent)
    frame_layout = QHBoxLayout(frame)
    frame_layout.setContentsMargins(0, 0, 0, 0)
    frame_layout.setSpacing(0)

    root_layout.addWidget(frame)
    apply_rounded_clip(frame, Dimensions.WINDOW_RADIUS)
    return root_layout, frame, frame_layout

