"""macOS-style traffic light window controls."""

from PySide6.QtCore import Qt, Signal, QRect, QPoint, QPropertyAnimation, QEasingCurve, Property
from PySide6.QtGui import QPainter, QPen, QBrush, QPainterPath, QFont, QColor
from PySide6.QtWidgets import QWidget, QHBoxLayout

from .styles.dark_theme import DarkTheme, Dimensions


class TrafficLightButton(QWidget):
    """Individual traffic light button with hover states."""
    
    clicked = Signal()
    
    def __init__(self, color: str, hover_symbol: str = "", parent=None):
        super().__init__(parent)
        self.color = QColor(color)
        self.hover_symbol = hover_symbol
        self.is_hovered = False
        self._dot_opacity = 0.0  # For animation
        
        # Fixed size matching original design
        size = Dimensions.TRAFFIC_LIGHT_SIZE
        self.setFixedSize(size, size)
        self.setToolTip(self._get_tooltip())
        
        # Enable mouse tracking for hover effects
        self.setMouseTracking(True)
        
        # Animation for dot opacity
        self._animation = QPropertyAnimation(self, b"dotOpacity")
        self._animation.setDuration(200)  # Fast animation like macOS
        self._animation.setEasingCurve(QEasingCurve.Type.OutCubic)
    
    def _get_dot_opacity(self) -> float:
        """Get the current dot opacity."""
        return self._dot_opacity
    
    def _set_dot_opacity(self, opacity: float):
        """Set dot opacity and trigger repaint."""
        self._dot_opacity = opacity
        self.update()
    
    # Property for animation
    dotOpacity = Property(float, _get_dot_opacity, _set_dot_opacity)
    
    def _get_tooltip(self) -> str:
        """Get tooltip text based on button color."""
        if self.color == DarkTheme.CLOSE_BTN.name():
            return "Close"
        elif self.color == DarkTheme.MINIMIZE_BTN.name():
            return "Minimize"
        elif self.color == DarkTheme.MAXIMIZE_BTN.name():
            return "Hide to Tray"
        return ""
    
    def mousePressEvent(self, event):
        """Handle mouse press events."""
        if event.button() == Qt.MouseButton.LeftButton:
            self.clicked.emit()
        super().mousePressEvent(event)
    
    def enterEvent(self, event):
        """Handle mouse enter events."""
        self.is_hovered = True
        # Animate dot appearance
        self._animation.setStartValue(self._dot_opacity)
        self._animation.setEndValue(1.0)
        self._animation.start()
        super().enterEvent(event)
    
    def leaveEvent(self, event):
        """Handle mouse leave events."""
        self.is_hovered = False
        # Animate dot disappearance
        self._animation.setStartValue(self._dot_opacity)
        self._animation.setEndValue(0.0)
        self._animation.start()
        super().leaveEvent(event)
    
    def paintEvent(self, event):
        """Custom paint event for the traffic light button."""
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        
        # Draw the colored circle
        rect = self.rect()
        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(QBrush(self.color))
        painter.drawEllipse(rect)
        
        # Draw animated center dot on hover (macOS style)
        if self._dot_opacity > 0.0:
            self._draw_macos_dot(painter, rect)
    
    def _draw_macos_dot(self, painter: QPainter, rect: QRect):
        """Draw macOS-style 50% darker dot with animation."""
        painter.setPen(Qt.PenStyle.NoPen)
        
        # Create much darker version of the button color (closer to black)
        darker_color = self.color.darker(300)  # Much darker for better visibility
        darker_color.setAlphaF(self._dot_opacity)  # Apply animated opacity
        
        painter.setBrush(QBrush(darker_color))
        
        # Draw properly sized and centered dot (macOS style)
        from PySide6.QtCore import QRectF, QPointF
        
        # Get precise center as floating point
        center_f = QPointF(rect.width() / 2.0, rect.height() / 2.0)
        dot_radius = 2.0  # Proper macOS dot size
        
        # Use QRectF for precise floating point centering - ensure perfect center
        dot_rect_f = QRectF(
            center_f.x() - dot_radius,
            center_f.y() - dot_radius,
            dot_radius * 2,
            dot_radius * 2
        )
        painter.drawEllipse(dot_rect_f)


class TrafficLights(QWidget):
    """Container for all three traffic light buttons."""
    
    close_clicked = Signal()
    minimize_clicked = Signal() 
    hide_clicked = Signal()
    
    def __init__(self, parent=None):
        super().__init__(parent)
        self._setup_ui()
    
    def _setup_ui(self):
        """Set up the traffic lights layout."""
        layout = QHBoxLayout(self)
        layout.setContentsMargins(Dimensions.TRAFFIC_LIGHT_CONTAINER_PADDING, 0, 0, 0)
        layout.setSpacing(Dimensions.TRAFFIC_LIGHT_GAP)
        
        # Create the three buttons
        self.close_btn = TrafficLightButton(DarkTheme.CLOSE_BTN.name(), "×")
        self.minimize_btn = TrafficLightButton(DarkTheme.MINIMIZE_BTN.name(), "−")
        self.hide_btn = TrafficLightButton(DarkTheme.MAXIMIZE_BTN.name(), "⛶")
        
        # Connect signals
        self.close_btn.clicked.connect(self.close_clicked.emit)
        self.minimize_btn.clicked.connect(self.minimize_clicked.emit)
        self.hide_btn.clicked.connect(self.hide_clicked.emit)
        
        # Add to layout
        layout.addWidget(self.close_btn)
        layout.addWidget(self.minimize_btn)
        layout.addWidget(self.hide_btn)
        layout.addStretch()  # Push buttons to the left