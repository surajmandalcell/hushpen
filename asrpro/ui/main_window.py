"""Main window with Models and SRT tabs, global hotkey toggle integration."""

from __future__ import annotations
import tempfile
from pathlib import Path
from typing import Optional
from PySide6.QtCore import Qt
from PySide6.QtGui import QClipboard, QPainter, QPainterPath, QBrush, QColor
from PySide6.QtWidgets import (
    QWidget,
    QVBoxLayout,
    QTabWidget,
    QListWidget,
    QPushButton,
    QLabel,
    QFileDialog,
    QTextEdit,
    QHBoxLayout,
    QApplication,
    QMessageBox,
    QGraphicsDropShadowEffect,
    QSystemTrayIcon,
)
from ..model_manager import ModelManager, MODEL_SPECS
from ..hotkey import ToggleHotkey
from ..audio_recorder import AudioRecorder
from .custom_titlebar import TitleBar
from .overlay import Overlay


class MainWindow(QWidget):  # pragma: no cover
    def __init__(self):
        super().__init__()
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint | Qt.WindowType.Window)
        self.setFixedSize(800, 600)

        # Add drop shadow effect
        self.shadow = QGraphicsDropShadowEffect()
        self.shadow.setBlurRadius(20)
        self.shadow.setColor(QColor(0, 0, 0, 80))
        self.shadow.setOffset(0, 4)
        self.setGraphicsEffect(self.shadow)

        # Set window attributes for rounded corners
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground, True)

        self.model_manager = ModelManager()
        self.recorder = AudioRecorder()
        self.overlay = Overlay()
        self._record_temp = Path(tempfile.gettempdir()) / "asrpro_record.wav"
        self.hotkey = ToggleHotkey(self._on_toggle)
        self.hotkey.start()

        # Initialize UI components that will be created in _build_ui
        self.srt_log: Optional[QTextEdit] = None
        self.btn_choose: Optional[QPushButton] = None
        self.tray_icon: Optional[QSystemTrayIcon] = None

        self._build_ui()

    def paintEvent(self, event):
        """Custom paint event for rounded corners"""
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)

        # Create rounded rectangle path
        path = QPainterPath()
        path.addRoundedRect(self.rect(), 12, 12)

        # Fill with background color
        painter.fillPath(path, QBrush(QColor(13, 17, 23)))  # #0d1117

        # Draw border
        painter.setPen(QColor(33, 38, 45))  # #21262d
        painter.drawPath(path)

    def _build_ui(self):
        root = QVBoxLayout(self)
        root.setContentsMargins(0, 0, 0, 0)
        root.setSpacing(0)

        self.titlebar = TitleBar(self)
        root.addWidget(self.titlebar)

        self.tabs = QTabWidget()
        self.tabs.addTab(self._models_tab(), "Models")
        self.tabs.addTab(self._srt_tab(), "SRT")
        root.addWidget(self.tabs)
        self.setStyleSheet(
            """
        /* Modern Glassmorphism/Neumorphism Theme */
        QWidget { 
            background: qlineargradient(x1:0, y1:0, x2:1, y2:1, 
                stop:0 #1a1a2e, stop:0.5 #16213e, stop:1 #0f3460);
            color: #ffffff; 
            font-family: 'Inter', 'SF Pro Display', -apple-system, system-ui, sans-serif; 
            font-size: 14px;
            font-weight: 400;
        }
        
        /* Main Window */
        MainWindow {
            border-radius: 16px;
            background: qlineargradient(x1:0, y1:0, x2:1, y2:1, 
                stop:0 rgba(26, 26, 46, 0.95), 
                stop:0.5 rgba(22, 33, 62, 0.95), 
                stop:1 rgba(15, 52, 96, 0.95));
        }
        
        /* Tabs - Modern Card Design */
        QTabWidget::pane { 
            border: none; 
            background: transparent;
            border-radius: 12px;
        }
        
        QTabBar {
            background: rgba(255, 255, 255, 0.05);
            border-radius: 12px;
            margin: 8px;
        }
        
        QTabBar::tab { 
            padding: 16px 32px; 
            background: rgba(255, 255, 255, 0.08); 
            border: 1px solid rgba(255, 255, 255, 0.1);
            margin: 4px 2px;
            border-radius: 10px;
            font-weight: 500;
            font-size: 15px;
            min-width: 140px;
            color: rgba(255, 255, 255, 0.8);
        }
        
        QTabBar::tab:selected { 
            background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
                stop:0 rgba(99, 102, 241, 0.8),
                stop:1 rgba(139, 92, 246, 0.8));
            border: 1px solid rgba(255, 255, 255, 0.2);
            color: white;
            font-weight: 600;
        }
        
        QTabBar::tab:hover:!selected { 
            background: rgba(255, 255, 255, 0.12);
            border: 1px solid rgba(255, 255, 255, 0.15);
            color: white;
        }
        
        /* Buttons - Glassmorphism Style */
        QPushButton { 
            background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
                stop:0 rgba(99, 102, 241, 0.8),
                stop:1 rgba(139, 92, 246, 0.8));
            color: white; 
            border: 1px solid rgba(255, 255, 255, 0.2);
            padding: 12px 24px; 
            border-radius: 10px; 
            font-weight: 500;
            font-size: 14px;
            min-height: 20px;
        }
        
        QPushButton:hover { 
            background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
                stop:0 rgba(109, 112, 251, 0.9),
                stop:1 rgba(149, 102, 255, 0.9));
            border: 1px solid rgba(255, 255, 255, 0.3);
        }
        
        QPushButton:pressed { 
            background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
                stop:0 rgba(79, 82, 221, 0.7),
                stop:1 rgba(119, 72, 226, 0.7));
        }
        
        /* List Widgets - Modern Card Design */
        QListWidget { 
            background: rgba(255, 255, 255, 0.05);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 12px;
            padding: 12px;
            selection-background-color: rgba(99, 102, 241, 0.3);
        }
        
        QListWidget::item {
            padding: 12px 16px;
            border-radius: 8px;
            margin: 3px 0;
            background: rgba(255, 255, 255, 0.03);
            border: 1px solid rgba(255, 255, 255, 0.05);
        }
        
        QListWidget::item:hover {
            background: rgba(255, 255, 255, 0.08);
            border: 1px solid rgba(255, 255, 255, 0.1);
        }
        
        QListWidget::item:selected {
            background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
                stop:0 rgba(99, 102, 241, 0.4),
                stop:1 rgba(139, 92, 246, 0.4));
            border: 1px solid rgba(255, 255, 255, 0.2);
            color: white;
            font-weight: 500;
        }
        
        /* Text Areas - Glassmorphism */
        QTextEdit { 
            background: rgba(0, 0, 0, 0.2);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 12px;
            padding: 16px;
            font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', monospace;
            font-size: 13px;
            line-height: 1.5;
            color: rgba(255, 255, 255, 0.9);
        }
        
        QTextEdit:focus {
            border: 1px solid rgba(99, 102, 241, 0.5);
            background: rgba(0, 0, 0, 0.3);
        }
        
        /* Labels - Modern Typography */
        QLabel {
            color: rgba(255, 255, 255, 0.9);
            font-weight: 400;
        }
        
        /* Scrollbars */
        QScrollBar:vertical {
            background: rgba(255, 255, 255, 0.05);
            width: 8px;
            border-radius: 4px;
        }
        
        QScrollBar::handle:vertical {
            background: rgba(255, 255, 255, 0.2);
            border-radius: 4px;
            min-height: 20px;
        }
        
        QScrollBar::handle:vertical:hover {
            background: rgba(255, 255, 255, 0.3);
        }
        
        /* Title Bar */
        #TitleBar { 
            background: qlineargradient(x1:0, y1:0, x2:1, y2:0, 
                stop:0 rgba(26, 26, 46, 0.9), 
                stop:1 rgba(22, 33, 62, 0.9)); 
            border-bottom: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 16px 16px 0px 0px;
        }
        
        #TitleLabel { 
            font-weight: 600; 
            font-size: 16px;
            color: white;
            letter-spacing: 0.5px;
        }
        
        /* Title Bar Buttons */
        #TitleBar QPushButton {
            background: rgba(255, 255, 255, 0.1);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 6px;
            padding: 6px 10px;
            font-size: 14px;
            color: rgba(255, 255, 255, 0.8);
        }
        
        #TitleBar QPushButton:hover {
            background: rgba(255, 255, 255, 0.15);
            color: white;
        }
        """
        )

    def _models_tab(self):
        w = QWidget()
        layout = QVBoxLayout(w)
        layout.setContentsMargins(24, 24, 24, 24)
        layout.setSpacing(20)

        # Header section with modern styling
        header_layout = QVBoxLayout()
        header_layout.setSpacing(12)

        title = QLabel("AI Models")
        title.setStyleSheet(
            "QLabel { "
            "font-size: 28px; "
            "font-weight: 700; "
            "color: white; "
            "margin-bottom: 8px; "
            "letter-spacing: -0.5px; "
            "}"
        )
        header_layout.addWidget(title)

        subtitle = QLabel("Select and load an AI model for transcription")
        subtitle.setStyleSheet(
            "QLabel { "
            "font-size: 16px; "
            "color: rgba(255, 255, 255, 0.7); "
            "margin-bottom: 16px; "
            "font-weight: 400; "
            "}"
        )
        header_layout.addWidget(subtitle)

        layout.addLayout(header_layout)

        # Model list with enhanced styling
        self.model_list = QListWidget()
        for mid in MODEL_SPECS.keys():
            self.model_list.addItem(mid)
        self.model_list.setMaximumHeight(180)
        layout.addWidget(self.model_list)

        # Button row with linear layout
        button_layout = QHBoxLayout()
        button_layout.setSpacing(12)

        self.btn_load = QPushButton("Load Selected Model")
        self.btn_load.setMinimumHeight(36)
        button_layout.addWidget(self.btn_load)

        self.btn_unload = QPushButton("Unload Model")
        self.btn_unload.setMinimumHeight(36)
        self.btn_unload.setStyleSheet(
            "QPushButton { "
            "background: qlineargradient(x1:0, y1:0, x2:1, y2:0, "
            "stop:0 rgba(244, 63, 94, 0.8), "
            "stop:1 rgba(239, 68, 68, 0.8)); "
            "color: white; "
            "border: 1px solid rgba(255, 255, 255, 0.2); "
            "padding: 12px 24px; "
            "border-radius: 10px; "
            "font-weight: 500; "
            "font-size: 14px; "
            "} "
            "QPushButton:hover { "
            "background: qlineargradient(x1:0, y1:0, x2:1, y2:0, "
            "stop:0 rgba(254, 73, 104, 0.9), "
            "stop:1 rgba(249, 78, 78, 0.9)); "
            "border: 1px solid rgba(255, 255, 255, 0.3); "
            "} "
            "QPushButton:pressed { "
            "background: qlineargradient(x1:0, y1:0, x2:1, y2:0, "
            "stop:0 rgba(224, 53, 84, 0.7), "
            "stop:1 rgba(229, 58, 58, 0.7)); "
            "}"
        )
        button_layout.addWidget(self.btn_unload)

        layout.addLayout(button_layout)

        # Device info with modern styling
        info = QLabel(f"Device preference: {self.model_manager.device}")
        info.setStyleSheet(
            "QLabel { "
            "color: rgba(255, 255, 255, 0.6); "
            "font-size: 14px; "
            "padding: 8px 0; "
            "font-weight: 400; "
            "}"
        )
        layout.addWidget(info)

        # Output log with modern styling
        log_label = QLabel("Loading Log")
        log_label.setStyleSheet(
            "QLabel { "
            "font-size: 18px; "
            "font-weight: 600; "
            "color: white; "
            "margin-top: 16px; "
            "margin-bottom: 12px; "
            "}"
        )
        layout.addWidget(log_label)

        self.output = QTextEdit()
        self.output.setReadOnly(True)
        self.output.setPlaceholderText(
            "🤖 Model loading progress will appear here...\n\nSelect a model and click 'Load Selected Model' to begin!"
        )
        layout.addWidget(self.output)

        def log(msg):
            self.output.append(msg)

        def do_load():
            item = self.model_list.currentItem()
            if not item:
                return
            mid = item.text()
            try:
                self.model_manager.load(mid, progress_cb=log)
            except Exception as e:
                log(f"Error: {e}")

        def do_unload():
            self.model_manager.unload()
            log("Unloaded model")

        self.btn_load.clicked.connect(do_load)
        self.btn_unload.clicked.connect(do_unload)
        return w

    def _srt_tab(self):
        class SRTTab(QWidget):
            def __init__(self, outer: "MainWindow"):
                super().__init__()
                self.outer = outer
                self.setAcceptDrops(True)
                layout = QVBoxLayout(self)
                layout.setContentsMargins(24, 24, 24, 24)
                layout.setSpacing(20)

                # Header section with modern styling
                header_layout = QVBoxLayout()
                header_layout.setSpacing(12)

                title = QLabel("SRT Generation")
                title.setStyleSheet(
                    "QLabel { "
                    "font-size: 28px; "
                    "font-weight: 700; "
                    "color: white; "
                    "margin-bottom: 8px; "
                    "letter-spacing: -0.5px; "
                    "}"
                )
                header_layout.addWidget(title)

                subtitle = QLabel("Generate subtitle files from audio/video media")
                subtitle.setStyleSheet(
                    "QLabel { "
                    "font-size: 16px; "
                    "color: rgba(255, 255, 255, 0.7); "
                    "margin-bottom: 16px; "
                    "font-weight: 400; "
                    "}"
                )
                header_layout.addWidget(subtitle)

                layout.addLayout(header_layout)

                # Modern drop zone with glassmorphism
                drop_zone = QWidget()
                drop_zone.setMinimumHeight(160)
                drop_zone.setStyleSheet(
                    "QWidget { "
                    "background: qlineargradient(x1:0, y1:0, x2:1, y2:1, "
                    "stop:0 rgba(255, 255, 255, 0.05), "
                    "stop:1 rgba(255, 255, 255, 0.03)); "
                    "border: 2px dashed rgba(99, 102, 241, 0.4); "
                    "border-radius: 16px; "
                    "}"
                )
                drop_layout = QVBoxLayout(drop_zone)
                drop_layout.setAlignment(Qt.AlignmentFlag.AlignCenter)
                drop_layout.setSpacing(16)

                # Modern upload icon with gradient
                drop_icon = QLabel("⬆️")
                drop_icon.setStyleSheet(
                    "QLabel { "
                    "font-size: 48px; "
                    "border-radius: 50%; "
                    "background: qlineargradient(x1:0, y1:0, x2:1, y2:0, "
                    "stop:0 rgba(99, 102, 241, 0.2), "
                    "stop:1 rgba(139, 92, 246, 0.2)); "
                    "padding: 16px; "
                    "}"
                )
                drop_icon.setAlignment(Qt.AlignmentFlag.AlignCenter)
                drop_layout.addWidget(drop_icon)

                drop_text = QLabel("Drag & drop media files here")
                drop_text.setStyleSheet(
                    "QLabel { "
                    "color: white; "
                    "font-size: 18px; "
                    "font-weight: 600; "
                    "}"
                )
                drop_text.setAlignment(Qt.AlignmentFlag.AlignCenter)
                drop_layout.addWidget(drop_text)

                drop_subtext = QLabel("Support for MP4, WAV, MP3, M4A and more")
                drop_subtext.setStyleSheet(
                    "QLabel { "
                    "color: rgba(255, 255, 255, 0.6); "
                    "font-size: 14px; "
                    "}"
                )
                drop_subtext.setAlignment(Qt.AlignmentFlag.AlignCenter)
                drop_layout.addWidget(drop_subtext)

                layout.addWidget(drop_zone)

                # Modern choose file button
                self.outer.btn_choose = QPushButton("📂  Browse Files")
                self.outer.btn_choose.setMinimumHeight(48)
                self.outer.btn_choose.setStyleSheet(
                    "QPushButton { "
                    "background: qlineargradient(x1:0, y1:0, x2:1, y2:0, "
                    "stop:0 rgba(99, 102, 241, 0.8), "
                    "stop:1 rgba(139, 92, 246, 0.8)); "
                    "color: white; "
                    "border: 1px solid rgba(255, 255, 255, 0.2); "
                    "padding: 12px 32px; "
                    "border-radius: 12px; "
                    "font-weight: 600; "
                    "font-size: 16px; "
                    "} "
                    "QPushButton:hover { "
                    "background: qlineargradient(x1:0, y1:0, x2:1, y2:0, "
                    "stop:0 rgba(109, 112, 251, 0.9), "
                    "stop:1 rgba(149, 102, 255, 0.9)); "
                    "border: 1px solid rgba(255, 255, 255, 0.3); "
                    "} "
                    "QPushButton:pressed { "
                    "background: qlineargradient(x1:0, y1:0, x2:1, y2:0, "
                    "stop:0 rgba(79, 82, 221, 0.7), "
                    "stop:1 rgba(119, 72, 226, 0.7)); "
                    "}"
                )
                self.outer.btn_choose.clicked.connect(self.outer._choose_file)
                layout.addWidget(self.outer.btn_choose)

                # Modern log area with glassmorphism
                log_label = QLabel("Processing Log")
                log_label.setStyleSheet(
                    "QLabel { "
                    "font-size: 18px; "
                    "font-weight: 600; "
                    "color: white; "
                    "margin-top: 16px; "
                    "margin-bottom: 12px; "
                    "}"
                )
                layout.addWidget(log_label)

                self.outer.srt_log = QTextEdit()
                self.outer.srt_log.setReadOnly(True)
                self.outer.srt_log.setPlaceholderText(
                    "🎵 SRT generation progress will appear here...\n\nSelect a media file to get started!"
                )
                self.outer.srt_log.setStyleSheet(
                    "QTextEdit { "
                    "background: rgba(0, 0, 0, 0.2); "
                    "border: 1px solid rgba(255, 255, 255, 0.1); "
                    "border-radius: 12px; "
                    "padding: 16px; "
                    "font-family: 'Segoe UI', system-ui, sans-serif; "
                    "font-size: 14px; "
                    "line-height: 1.6; "
                    "color: rgba(255, 255, 255, 0.9); "
                    "} "
                    "QTextEdit:focus { "
                    "border: 1px solid rgba(99, 102, 241, 0.5); "
                    "background: rgba(0, 0, 0, 0.3); "
                    "}"
                )
                layout.addWidget(self.outer.srt_log)

            def dragEnterEvent(self, event):
                if event.mimeData().hasUrls():
                    event.acceptProposedAction()
                else:
                    event.ignore()

            def dropEvent(self, event):
                for u in event.mimeData().urls():
                    p = Path(u.toLocalFile())
                    if p.exists() and p.is_file():
                        self.outer._generate_srt(p)
                event.acceptProposedAction()

        return SRTTab(self)

    def _choose_file(self):
        file, _ = QFileDialog.getOpenFileName(self, "Select media file")
        if not file:
            return
        self._generate_srt(Path(file))

    def _generate_srt(self, path: Path):
        if not self.model_manager.current_id:
            QMessageBox.warning(self, "Model", "Load a model first")
            return
        if not self.srt_log:
            return
        try:
            result = self.model_manager.transcribe(str(path))
            srt = self.model_manager._to_srt(result)
            srt_path = path.with_suffix(".srt")
            srt_path.write_text(srt, encoding="utf-8")
            self.srt_log.append(f"Created {srt_path}")
        except Exception as e:
            self.srt_log.append(f"Error: {e}")

    def _on_toggle(self, active: bool):
        if active:
            self.overlay.show_smooth()
            self.recorder.start(self._record_temp)
        else:
            wav = self.recorder.stop()
            if wav and self.model_manager.current_id:
                try:
                    segs = self.model_manager.transcribe(str(wav))  # type: ignore
                    text_parts = []
                    for s in segs:  # type: ignore
                        try:
                            if isinstance(s, dict):
                                text_parts.append(s.get("text", ""))
                            else:
                                text_parts.append(str(s))
                        except Exception:
                            continue
                    text = " ".join(tp for tp in text_parts if tp)
                    if text:
                        self._paste_text(text)
                except Exception:
                    pass
            self.overlay.close_smooth()

    def _paste_text(self, text: str):  # pragma: no cover
        cb = QApplication.clipboard()
        cb.setText(text, QClipboard.Mode.Clipboard)
        try:
            from pynput.keyboard import Controller, Key  # type: ignore

            k = Controller()
            k.press(Key.ctrl)
            k.press("v")
            k.release("v")
            k.release(Key.ctrl)
        except Exception:
            pass

    def apply_hotkey_change(self, hk: str):
        self.hotkey.set_hotkey(hk)

    def set_tray_icon(self, tray_icon: QSystemTrayIcon):
        """Store reference to tray icon for theme updates."""
        self.tray_icon = tray_icon

    def refresh_tray_icon_theme(self):
        """Refresh tray icon based on current system theme."""
        if self.tray_icon:
            from .tray import build_tray

            # Get the new icon with updated theme
            new_tray = build_tray(self)
            self.tray_icon.setIcon(new_tray.icon())

    def close_app(self):
        self.model_manager.unload()
        from PySide6.QtWidgets import (
            QApplication,
        )  # local import to avoid cyclic issues

        app = QApplication.instance()
        if app:
            app.quit()
        self.close()
