"""Icon color inversion utilities for dark theme compatibility."""

from PySide6.QtCore import Qt
from PySide6.QtGui import QPainter, QPixmap


def invert_icon(pixmap: QPixmap) -> QPixmap:
    """Invert the colors of a QPixmap for dark theme compatibility while preserving transparency.
    
    Args:
        pixmap: The QPixmap to invert
        
    Returns:
        QPixmap: The inverted pixmap with preserved alpha channel
    """
    inverted = QPixmap(pixmap.size())
    inverted.fill(Qt.GlobalColor.transparent)

    painter = QPainter(inverted)
    painter.setRenderHint(QPainter.RenderHint.Antialiasing)
    painter.setCompositionMode(QPainter.CompositionMode.CompositionMode_SourceOver)

    # Draw the original pixmap
    painter.drawPixmap(0, 0, pixmap)

    # Apply color inversion while preserving alpha channel
    painter.setCompositionMode(QPainter.CompositionMode.CompositionMode_Difference)
    painter.fillRect(inverted.rect(), Qt.GlobalColor.white)

    # Restore original alpha channel
    painter.setCompositionMode(QPainter.CompositionMode.CompositionMode_DestinationIn)
    painter.drawPixmap(0, 0, pixmap)

    painter.end()
    return inverted