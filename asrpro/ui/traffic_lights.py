"""macOS-style traffic light window controls."""

from PySide6.QtCore import Qt, Signal, QRect, QPoint
from PySide6.QtGui import QPainter, QPen, QBrush, QPainterPath, QFont
from PySide6.QtWidgets import QWidget, QHBoxLayout

from .styles.dark_theme import DarkTheme, Dimensions


class TrafficLightButton(QWidget):
    """Individual traffic light button with hover states."""
    
    clicked = Signal()
    
    def __init__(self, color: str, hover_symbol: str = "", parent=None):
        super().__init__(parent)
        self.color = color
        self.hover_symbol = hover_symbol
        self.is_hovered = False
        
        # Fixed size matching original design
        size = Dimensions.TRAFFIC_LIGHT_SIZE
        self.setFixedSize(size, size)
        self.setToolTip(self._get_tooltip())
        
        # Enable mouse tracking for hover effects
        self.setMouseTracking(True)
    
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
        self.update()
        super().enterEvent(event)
    
    def leaveEvent(self, event):
        """Handle mouse leave events."""
        self.is_hovered = False
        self.update()
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
        
        # Draw center dot (always visible)
        self._draw_center_dot(painter, rect)
        
        # Draw hover symbol if hovered
        if self.is_hovered and self.hover_symbol:
            self._draw_hover_symbol(painter, rect)
    
    def _draw_center_dot(self, painter: QPainter, rect: QRect):
        """Draw a small centered dot on the traffic light."""
        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(QBrush(Qt.GlobalColor.black))
        
        # Draw small dot in center
        center = rect.center()
        dot_size = 2  # Small dot
        dot_rect = QRect(
            center.x() - dot_size // 2,
            center.y() - dot_size // 2,
            dot_size,
            dot_size
        )
        painter.drawEllipse(dot_rect)
    
    def _draw_hover_symbol(self, painter: QPainter, rect: QRect):
        """Draw the hover symbol (×, −, or fullscreen symbol)."""
        painter.setPen(QPen(Qt.GlobalColor.black, 1.2))
        painter.setBrush(Qt.BrushStyle.NoBrush)
        
        center = rect.center()
        size = 3  # Symbol size
        
        if self.hover_symbol == "×":  # Close
            # Draw X
            painter.drawLine(center.x() - size, center.y() - size,
                           center.x() + size, center.y() + size)
            painter.drawLine(center.x() + size, center.y() - size,
                           center.x() - size, center.y() + size)
        
        elif self.hover_symbol == "−":  # Minimize
            # Draw horizontal line
            painter.drawLine(center.x() - size, center.y(),
                           center.x() + size, center.y())
        
        elif self.hover_symbol == "⛶":  # Hide/Maximize
            # Draw two small rectangles (stacked windows symbol)
            rect1 = QRect(center.x() - size + 1, center.y() - size + 1, 
                         size, size - 1)
            rect2 = QRect(center.x() - size + 2, center.y() - size + 2,
                         size, size - 1)
            painter.drawRect(rect1)
            painter.drawRect(rect2)


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