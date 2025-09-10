"""Custom toggle switch widget matching the original design."""

from PySide6.QtCore import Qt, Signal, QPropertyAnimation, QEasingCurve, Property, QRect
from PySide6.QtGui import QPainter, QBrush, QPen, QColor
from PySide6.QtWidgets import QWidget

from ..styles.dark_theme import DarkTheme, Dimensions


class ToggleSwitch(QWidget):
    """Custom toggle switch with smooth animation."""
    
    toggled = Signal(bool)
    
    def __init__(self, parent=None):
        super().__init__(parent)
        
        self._checked = False
        self._knob_position = Dimensions.TOGGLE_KNOB_OFFSET  # Starting position
        
        # Set fixed size
        self.setFixedSize(Dimensions.TOGGLE_WIDTH, Dimensions.TOGGLE_HEIGHT)
        self.setToolTip("Toggle this setting")
        
        # Animation for smooth knob movement
        self._animation = QPropertyAnimation(self, b"knobPosition")
        self._animation.setDuration(200)  # 0.2s as per original design
        self._animation.setEasingCurve(QEasingCurve.Type.OutCubic)
        
        # Mouse tracking for hover effects (future enhancement)
        self.setMouseTracking(True)
    
    def _get_knob_position(self) -> float:
        """Get the current knob position."""
        return self._knob_position
    
    def _set_knob_position(self, position: float):
        """Set knob position and trigger repaint."""
        self._knob_position = position
        self.update()
    
    # Property for animation
    knobPosition = Property(float, _get_knob_position, _set_knob_position)
    
    def isChecked(self) -> bool:
        """Return current checked state."""
        return self._checked
    
    def setChecked(self, checked: bool, animate: bool = True):
        """Set checked state with optional animation."""
        if self._checked == checked:
            return
            
        self._checked = checked
        
        # Calculate target position
        if checked:
            target_pos = Dimensions.TOGGLE_WIDTH - Dimensions.TOGGLE_KNOB_SIZE - Dimensions.TOGGLE_KNOB_OFFSET
        else:
            target_pos = Dimensions.TOGGLE_KNOB_OFFSET
        
        if animate and self.isVisible():
            # Animate to new position
            self._animation.setStartValue(self._knob_position)
            self._animation.setEndValue(target_pos)
            self._animation.start()
        else:
            # Set immediately without animation
            self._knob_position = target_pos
            self.update()
        
        self.toggled.emit(checked)
    
    def toggle(self):
        """Toggle the current state."""
        self.setChecked(not self._checked)
    
    def mousePressEvent(self, event):
        """Handle mouse press to toggle state."""
        if event.button() == Qt.MouseButton.LeftButton:
            self.toggle()
        super().mousePressEvent(event)
    
    def paintEvent(self, event):
        """Custom paint event for the toggle switch."""
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        
        # Draw track background
        self._draw_track(painter)
        
        # Draw knob
        self._draw_knob(painter)
    
    def _draw_track(self, painter: QPainter):
        """Draw the toggle track background."""
        rect = self.rect()
        radius = Dimensions.TOGGLE_HEIGHT / 2
        
        # Choose color based on state
        if self._checked:
            track_color = DarkTheme.ACCENT_BLUE
        else:
            track_color = DarkTheme.CONTROL_BG
        
        # Draw rounded rectangle track
        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(QBrush(track_color))
        painter.drawRoundedRect(rect, radius, radius)
    
    def _draw_knob(self, painter: QPainter):
        """Draw the toggle knob."""
        knob_size = Dimensions.TOGGLE_KNOB_SIZE
        knob_y = (Dimensions.TOGGLE_HEIGHT - knob_size) / 2
        
        knob_rect = QRect(
            int(self._knob_position),
            int(knob_y),
            knob_size,
            knob_size
        )
        
        # Draw white circular knob
        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(QBrush(DarkTheme.PRIMARY_TEXT))
        painter.drawEllipse(knob_rect)
    
    def sizeHint(self):
        """Return the preferred size."""
        from PySide6.QtCore import QSize
        return QSize(Dimensions.TOGGLE_WIDTH, Dimensions.TOGGLE_HEIGHT)