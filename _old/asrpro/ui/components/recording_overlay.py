"""Minimal recording overlay that appears when hotkey is pressed."""

from PySide6.QtCore import Qt, QTimer, QPropertyAnimation, QRect, Property, Signal, QPoint
from PySide6.QtGui import QPainter, QColor, QBrush, QPen, QLinearGradient, QFont
from PySide6.QtWidgets import QWidget, QLabel, QVBoxLayout, QGraphicsDropShadowEffect, QApplication
import math


class RecordingOverlay(QWidget):
    """Beautiful minimal overlay for recording status."""
    
    # Signals
    recording_stopped = Signal()
    recording_cancelled = Signal()
    
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setWindowFlags(
            Qt.WindowType.FramelessWindowHint |
            Qt.WindowType.WindowStaysOnTopHint |
            Qt.WindowType.Tool |  # Prevents showing in dock/taskbar
            Qt.WindowType.WindowTransparentForInput |  # Allow clicks to pass through
            Qt.WindowType.X11BypassWindowManagerHint  # For Linux
        )
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        self.setAttribute(Qt.WidgetAttribute.WA_ShowWithoutActivating)
        
        # Animation properties
        self._pulse = 0.0
        self._gradient_offset = 0.0
        
        # Setup UI
        self._setup_ui()
        
        # Animation timers
        self.pulse_timer = QTimer()
        self.pulse_timer.timeout.connect(self._update_pulse)
        
        self.gradient_timer = QTimer()
        self.gradient_timer.timeout.connect(self._update_gradient)
        
        # Position at bottom center of screen
        self._position_overlay()
    
    def _setup_ui(self):
        """Setup the UI components."""
        self.setFixedSize(280, 80)
        
        # Main layout
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        
        # Container widget with background
        self.container = QWidget()
        self.container.setObjectName("container")
        container_layout = QVBoxLayout(self.container)
        container_layout.setContentsMargins(24, 20, 24, 20)
        container_layout.setSpacing(4)
        
        # Status text
        self.status_label = QLabel("Listening...")
        self.status_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        
        font = QFont("SF Pro Display", 16)
        font.setWeight(QFont.Weight.Light)
        self.status_label.setFont(font)
        self.status_label.setStyleSheet("color: white;")
        
        # Hint text
        self.hint_label = QLabel("Press hotkey again to transcribe • ESC to cancel")
        self.hint_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        
        hint_font = QFont("SF Pro Text", 10)
        hint_font.setWeight(QFont.Weight.Light)
        self.hint_label.setFont(hint_font)
        self.hint_label.setStyleSheet("color: rgba(255, 255, 255, 0.6);")
        
        container_layout.addWidget(self.status_label)
        container_layout.addWidget(self.hint_label)
        
        layout.addWidget(self.container)
        
        # Apply shadow effect
        shadow = QGraphicsDropShadowEffect()
        shadow.setBlurRadius(30)
        shadow.setColor(QColor(0, 0, 0, 80))
        shadow.setOffset(0, 10)
        self.container.setGraphicsEffect(shadow)
    
    def _position_overlay(self):
        """Position the overlay at bottom center of primary screen."""
        if QApplication.primaryScreen():
            screen_rect = QApplication.primaryScreen().geometry()
            x = (screen_rect.width() - self.width()) // 2
            y = screen_rect.height() - self.height() - 100  # 100px from bottom
            self.move(x, y)
    
    def paintEvent(self, event):
        """Custom paint for animated gradient background."""
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        
        # Create rounded rect path
        rect = self.rect().adjusted(10, 10, -10, -10)
        painter.setPen(QPen(Qt.PenStyle.NoPen))
        
        # Animated gradient background
        gradient = QLinearGradient(rect.topLeft(), rect.bottomRight())
        
        # Create animated colors based on gradient offset
        base_hue = (120 + self._gradient_offset * 60) % 360  # Green to blue spectrum
        color1 = QColor.fromHslF(base_hue / 360, 0.7, 0.5, 0.95)
        color2 = QColor.fromHslF((base_hue + 30) / 360, 0.6, 0.4, 0.95)
        color3 = QColor.fromHslF((base_hue + 60) / 360, 0.7, 0.45, 0.95)
        
        gradient.setColorAt(0, color1)
        gradient.setColorAt(0.5, color2)
        gradient.setColorAt(1, color3)
        
        painter.setBrush(QBrush(gradient))
        painter.drawRoundedRect(rect, 20, 20)
        
        # Draw pulse ring effect
        if self._pulse > 0:
            pulse_color = QColor(255, 255, 255, int(50 * (1 - self._pulse)))
            painter.setPen(QPen(pulse_color, 2))
            painter.setBrush(Qt.BrushStyle.NoBrush)
            
            pulse_rect = rect.adjusted(
                -20 * self._pulse,
                -20 * self._pulse,
                20 * self._pulse,
                20 * self._pulse
            )
            painter.drawRoundedRect(pulse_rect, 20 + 10 * self._pulse, 20 + 10 * self._pulse)
    
    def show_overlay(self):
        """Show the overlay with animation."""
        self.show()
        self.raise_()
        
        # Start animations
        self.pulse_timer.start(50)  # 20 FPS for pulse
        self.gradient_timer.start(30)  # ~33 FPS for gradient
        
        # Ensure window can't receive focus
        self.setWindowFlags(self.windowFlags() | Qt.WindowType.WindowDoesNotAcceptFocus)
        self.show()  # Re-show after flag change
    
    def hide_overlay(self):
        """Hide the overlay and stop animations."""
        self.pulse_timer.stop()
        self.gradient_timer.stop()
        self.hide()
    
    def set_transcribing(self):
        """Update UI to show transcribing state."""
        self.status_label.setText("Transcribing...")
        self.hint_label.setText("Processing your speech")
        self.update()
    
    def _update_pulse(self):
        """Update pulse animation."""
        self._pulse += 0.05
        if self._pulse > 1.0:
            self._pulse = 0.0
        self.update()
    
    def _update_gradient(self):
        """Update gradient animation."""
        self._gradient_offset += 0.02
        if self._gradient_offset > 1.0:
            self._gradient_offset = 0.0
        self.update()
    
    def keyPressEvent(self, event):
        """Handle key press events."""
        if event.key() == Qt.Key.Key_Escape:
            self.recording_cancelled.emit()
            self.hide_overlay()
        else:
            super().keyPressEvent(event)