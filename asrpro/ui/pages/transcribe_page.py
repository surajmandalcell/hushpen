"""Beautiful drag and drop transcribe page."""

import os
import platform
import threading
from pathlib import Path
from typing import Optional

from PySide6.QtCore import Qt, Signal, QTimer, QPropertyAnimation, QRect, QEasingCurve, Property
from PySide6.QtGui import QFont, QPainter, QColor, QPen, QBrush, QLinearGradient, QDragEnterEvent, QDropEvent
from PySide6.QtWidgets import (
    QWidget, QVBoxLayout, QHBoxLayout, QLabel, 
    QPushButton, QProgressBar, QFileDialog, QMessageBox
)

from .base_page import BasePage
from ..styles.dark_theme import DarkTheme, Fonts
from ...model_manager import ModelManager
from ...models import WhisperMediumOnnxLoader
import tempfile
import subprocess


class DragDropZone(QWidget):
    """Beautiful animated drag and drop zone."""
    
    files_dropped = Signal(list)
    
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setAcceptDrops(True)
        self.is_hovering = False
        self._animation_progress = 0.0
        
        # Animation timer
        self.animation_timer = QTimer()
        self.animation_timer.timeout.connect(self._update_animation)
        self.animation_timer.start(30)  # 33 FPS
        
        self._setup_ui()
    
    def _setup_ui(self):
        """Setup the drag drop zone UI."""
        self.setMinimumHeight(300)
        
        layout = QVBoxLayout(self)
        layout.setContentsMargins(40, 40, 40, 40)
        layout.setSpacing(20)
        
        # Icon placeholder (you could add an actual icon here)
        self.icon_label = QLabel("📁")
        self.icon_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        icon_font = QFont("SF Pro Display", 48)
        self.icon_label.setFont(icon_font)
        
        # Main text
        self.main_text = QLabel("Drop media files here")
        self.main_text.setAlignment(Qt.AlignmentFlag.AlignCenter)
        main_font = QFont("SF Pro Display", 20)
        main_font.setWeight(QFont.Weight.Light)
        self.main_text.setFont(main_font)
        self.main_text.setStyleSheet("color: white;")
        
        # Sub text
        self.sub_text = QLabel("or click to browse")
        self.sub_text.setAlignment(Qt.AlignmentFlag.AlignCenter)
        sub_font = QFont("SF Pro Text", 14)
        sub_font.setWeight(QFont.Weight.Light)
        self.sub_text.setFont(sub_font)
        self.sub_text.setStyleSheet("color: rgba(255, 255, 255, 0.6);")
        
        # Supported formats
        self.formats_text = QLabel("Supports: MP4, MOV, AVI, MKV, MP3, WAV, M4A")
        self.formats_text.setAlignment(Qt.AlignmentFlag.AlignCenter)
        formats_font = QFont("SF Pro Text", 11)
        formats_font.setWeight(QFont.Weight.Light)
        self.formats_text.setFont(formats_font)
        self.formats_text.setStyleSheet("color: rgba(255, 255, 255, 0.4);")
        
        layout.addStretch()
        layout.addWidget(self.icon_label)
        layout.addWidget(self.main_text)
        layout.addWidget(self.sub_text)
        layout.addWidget(self.formats_text)
        layout.addStretch()
        
        # Make the whole widget clickable
        self.setCursor(Qt.CursorShape.PointingHandCursor)
    
    def paintEvent(self, event):
        """Custom paint for animated gradient background."""
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        
        rect = self.rect()
        
        # Background gradient
        gradient = QLinearGradient(rect.topLeft(), rect.bottomRight())
        
        if self.is_hovering:
            # Animated colors when hovering
            base_hue = (200 + self._animation_progress * 30) % 360
            color1 = QColor.fromHslF(base_hue / 360, 0.5, 0.25, 1.0)
            color2 = QColor.fromHslF((base_hue + 20) / 360, 0.4, 0.20, 1.0)
        else:
            # Subtle gradient when not hovering
            color1 = QColor(40, 40, 45)
            color2 = QColor(30, 30, 35)
        
        gradient.setColorAt(0, color1)
        gradient.setColorAt(1, color2)
        
        # Draw rounded rectangle
        painter.setPen(QPen(Qt.PenStyle.NoPen))
        painter.setBrush(QBrush(gradient))
        painter.drawRoundedRect(rect, 20, 20)
        
        # Draw dashed border
        if self.is_hovering:
            border_color = QColor.fromHslF((200 + self._animation_progress * 30) % 360 / 360, 0.6, 0.5, 0.8)
        else:
            border_color = QColor(255, 255, 255, 30)
        
        painter.setPen(QPen(border_color, 2, Qt.PenStyle.DashLine))
        painter.setBrush(Qt.BrushStyle.NoBrush)
        painter.drawRoundedRect(rect.adjusted(1, 1, -1, -1), 20, 20)
    
    def dragEnterEvent(self, event: QDragEnterEvent):
        """Handle drag enter."""
        if event.mimeData().hasUrls():
            event.acceptProposedAction()
            self.is_hovering = True
            self.update()
    
    def dragLeaveEvent(self, event):
        """Handle drag leave."""
        self.is_hovering = False
        self.update()
    
    def dropEvent(self, event: QDropEvent):
        """Handle file drop."""
        files = []
        for url in event.mimeData().urls():
            file_path = url.toLocalFile()
            if os.path.isfile(file_path):
                files.append(file_path)
        
        if files:
            self.files_dropped.emit(files)
        
        self.is_hovering = False
        self.update()
    
    def mousePressEvent(self, event):
        """Handle click to browse."""
        if event.button() == Qt.MouseButton.LeftButton:
            self._browse_files()
    
    def _browse_files(self):
        """Open file browser."""
        files, _ = QFileDialog.getOpenFileNames(
            self,
            "Select Media Files",
            "",
            "Media Files (*.mp4 *.mov *.avi *.mkv *.mp3 *.wav *.m4a *.aac *.flac);;All Files (*.*)"
        )
        if files:
            self.files_dropped.emit(files)
    
    def _update_animation(self):
        """Update animation progress."""
        self._animation_progress += 0.02
        if self._animation_progress > 1.0:
            self._animation_progress = 0.0
        if self.is_hovering:
            self.update()


class TranscribePage(BasePage):
    """Transcribe media files page."""
    
    def __init__(self, model_manager: ModelManager, parent=None):
        super().__init__("Transcribe Files", parent)
        self.model_manager = model_manager
        self.current_file = None
        self._create_content()
    
    def _create_content(self):
        """Create the transcribe page content."""
        # Drag and drop zone
        self.drop_zone = DragDropZone()
        self.drop_zone.files_dropped.connect(self._on_files_dropped)
        self.add_content_widget(self.drop_zone)
        
        # Progress section (initially hidden)
        self.progress_widget = QWidget()
        self.progress_widget.setVisible(False)
        progress_layout = QVBoxLayout(self.progress_widget)
        progress_layout.setSpacing(10)
        
        # File name label
        self.file_label = QLabel()
        file_font = QFont("SF Pro Text", 14)
        file_font.setWeight(QFont.Weight.Light)
        self.file_label.setFont(file_font)
        self.file_label.setStyleSheet("color: white;")
        progress_layout.addWidget(self.file_label)
        
        # Progress bar
        self.progress_bar = QProgressBar()
        self.progress_bar.setStyleSheet("""
            QProgressBar {
                background-color: rgba(255, 255, 255, 0.1);
                border: none;
                border-radius: 4px;
                height: 8px;
            }
            QProgressBar::chunk {
                background: qlineargradient(x1: 0, y1: 0, x2: 1, y2: 0,
                    stop: 0 #0a84ff, stop: 1 #00d4ff);
                border-radius: 4px;
            }
        """)
        progress_layout.addWidget(self.progress_bar)
        
        # Status label
        self.status_label = QLabel("Preparing...")
        status_font = QFont("SF Pro Text", 12)
        status_font.setWeight(QFont.Weight.Light)
        self.status_label.setFont(status_font)
        self.status_label.setStyleSheet("color: rgba(255, 255, 255, 0.6);")
        progress_layout.addWidget(self.status_label)
        
        self.add_content_widget(self.progress_widget)
        
        # Add stretch
        self.add_stretch()
    
    def _on_files_dropped(self, files):
        """Handle dropped files."""
        if not files:
            return
        
        # Process first file (you could extend this to handle multiple)
        self.current_file = Path(files[0])
        
        # Show progress
        self.drop_zone.setVisible(False)
        self.progress_widget.setVisible(True)
        
        # Update UI
        self.file_label.setText(f"Processing: {self.current_file.name}")
        self.progress_bar.setValue(0)
        self.status_label.setText("Converting to audio...")
        
        # Process in background
        threading.Thread(target=self._process_file, daemon=True).start()
    
    def _process_file(self):
        """Process the media file and generate SRT."""
        try:
            # Update progress
            self.progress_bar.setValue(20)
            self.status_label.setText("Extracting audio...")
            
            # Convert to WAV using ffmpeg
            with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as tmp:
                temp_wav = Path(tmp.name)
            
            # Use ffmpeg to extract audio
            cmd = [
                "ffmpeg", "-i", str(self.current_file),
                "-vn",  # No video
                "-acodec", "pcm_s16le",  # PCM 16-bit
                "-ar", "16000",  # 16kHz sample rate
                "-ac", "1",  # Mono
                "-y",  # Overwrite
                str(temp_wav)
            ]
            
            result = subprocess.run(cmd, capture_output=True, text=True)
            if result.returncode != 0:
                raise Exception(f"FFmpeg failed: {result.stderr}")
            
            # Update progress
            self.progress_bar.setValue(40)
            self.status_label.setText("Loading Whisper model...")
            
            # Ensure model is loaded
            if self.model_manager.current_id != "whisper-medium-onnx":
                self.model_manager.load("whisper-medium-onnx")
            
            # Update progress
            self.progress_bar.setValue(60)
            self.status_label.setText("Transcribing audio...")
            
            # Transcribe
            srt_content = self.model_manager.transcribe(
                str(temp_wav),
                model_id="whisper-medium-onnx",
                return_srt=True
            )
            
            # Update progress
            self.progress_bar.setValue(80)
            self.status_label.setText("Saving SRT file...")
            
            # Determine output directory
            if platform.system() == 'Darwin':
                # macOS: ~/Movies/asrpro/
                output_dir = Path.home() / "Movies" / "asrpro"
            else:
                # Other OS: project folder/output
                output_dir = Path.cwd() / "output"
            
            # Create output directory
            output_dir.mkdir(parents=True, exist_ok=True)
            
            # Create subdirectory for this file
            file_dir = output_dir / self.current_file.stem
            file_dir.mkdir(exist_ok=True)
            
            # Save SRT
            srt_path = file_dir / f"{self.current_file.stem}.srt"
            srt_path.write_text(srt_content, encoding='utf-8')
            
            # Clean up temp file
            temp_wav.unlink()
            
            # Update progress
            self.progress_bar.setValue(100)
            self.status_label.setText(f"Completed! Saved to: {srt_path}")
            
            # Reset after delay
            QTimer.singleShot(3000, self._reset_ui)
            
        except Exception as e:
            self.status_label.setText(f"Error: {str(e)}")
            QTimer.singleShot(3000, self._reset_ui)
    
    def _reset_ui(self):
        """Reset UI to initial state."""
        self.drop_zone.setVisible(True)
        self.progress_widget.setVisible(False)
        self.progress_bar.setValue(0)