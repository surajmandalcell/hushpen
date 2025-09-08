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
        /* Linear Modern Theme */
        QWidget { 
            background: #0d1117; 
            color: #f0f6fc; 
            font-family: 'Segoe UI', 'SF Pro Display', system-ui, sans-serif; 
            font-size: 14px;
        }
        
        /* Tabs - Linear horizontal design */
        QTabWidget::pane { 
            border: none; 
            background: #0d1117; 
        }
        QTabBar::tab { 
            padding: 12px 24px; 
            background: transparent; 
            border: none; 
            margin-right: 0px; 
            border-bottom: 2px solid transparent;
            font-weight: 500;
            min-width: 120px;
        }
        QTabBar::tab:selected { 
            background: linear-gradient(90deg, #238636 0%, #2ea043 100%); 
            border-bottom: 2px solid #2ea043;
            color: white;
        }
        QTabBar::tab:hover:!selected { 
            background: #161b22; 
            border-bottom: 2px solid #6e7681;
        }
        
        /* Buttons - Linear style */
        QPushButton { 
            background: linear-gradient(90deg, #238636 0%, #2ea043 100%); 
            color: white; 
            border: none; 
            padding: 10px 20px; 
            border-radius: 6px; 
            font-weight: 500;
            min-height: 16px;
        }
        QPushButton:hover { 
            background: linear-gradient(90deg, #2ea043 0%, #238636 100%); 
            transform: translateY(-1px);
        }
        QPushButton:pressed { 
            background: #1f6f32; 
        }
        
        /* List and Text widgets - Minimal borders */
        QListWidget { 
            background: #161b22; 
            border: 1px solid #30363d; 
            border-radius: 6px;
            padding: 8px;
            selection-background-color: #238636;
        }
        QListWidget::item {
            padding: 8px 12px;
            border-radius: 4px;
            margin: 2px 0;
        }
        QListWidget::item:hover {
            background: #21262d;
        }
        QListWidget::item:selected {
            background: linear-gradient(90deg, #238636 0%, #2ea043 100%);
            color: white;
        }
        
        QTextEdit { 
            background: #161b22; 
            border: 1px solid #30363d; 
            border-radius: 6px;
            padding: 12px;
            font-family: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace;
            font-size: 13px;
            line-height: 1.4;
        }
        
        /* Labels - Better hierarchy */
        QLabel {
            color: #f0f6fc;
            font-weight: 400;
        }
        
        /* Title bar */
        #TitleBar { 
            background: linear-gradient(90deg, #0d1117 0%, #161b22 100%); 
            border-bottom: 1px solid #21262d;
        }
        #TitleLabel { 
            font-weight: 600; 
            font-size: 16px;
            color: #f0f6fc;
        }
        
        /* Close button */
        #TitleBar QPushButton {
            background: transparent;
            border-radius: 4px;
            padding: 4px 8px;
            font-size: 16px;
        }
        #TitleBar QPushButton:hover {
            background: #da3633;
            color: white;
        }
        """
        )

    def _models_tab(self):
        w = QWidget()
        layout = QVBoxLayout(w)
        layout.setContentsMargins(24, 24, 24, 24)
        layout.setSpacing(20)

        # Header section
        header_layout = QVBoxLayout()
        header_layout.setSpacing(8)

        title = QLabel("Model Selection")
        title.setStyleSheet(
            "QLabel { "
            "font-size: 20px; "
            "font-weight: 600; "
            "color: #f0f6fc; "
            "margin-bottom: 4px; "
            "}"
        )
        header_layout.addWidget(title)

        subtitle = QLabel(
            "Choose a model to load. Only one model can be active at a time."
        )
        subtitle.setStyleSheet(
            "QLabel { "
            "font-size: 14px; "
            "color: #7d8590; "
            "margin-bottom: 8px; "
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
            "background: #21262d; "
            "color: #f85149; "
            "border: 1px solid #f85149; "
            "} "
            "QPushButton:hover { "
            "background: #f85149; "
            "color: white; "
            "}"
        )
        button_layout.addWidget(self.btn_unload)

        layout.addLayout(button_layout)

        # Device info
        info = QLabel(f"Device preference: {self.model_manager.device}")
        info.setStyleSheet(
            "QLabel { " "color: #7d8590; " "font-size: 13px; " "padding: 8px 0; " "}"
        )
        layout.addWidget(info)

        # Output log
        log_label = QLabel("Loading Log")
        log_label.setStyleSheet(
            "QLabel { "
            "font-size: 16px; "
            "font-weight: 500; "
            "color: #f0f6fc; "
            "margin-top: 8px; "
            "margin-bottom: 8px; "
            "}"
        )
        layout.addWidget(log_label)

        self.output = QTextEdit()
        self.output.setReadOnly(True)
        self.output.setPlaceholderText("Model loading progress will appear here...")
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

                # Header section
                header_layout = QVBoxLayout()
                header_layout.setSpacing(8)

                title = QLabel("SRT Generation")
                title.setStyleSheet(
                    "QLabel { "
                    "font-size: 20px; "
                    "font-weight: 600; "
                    "color: #f0f6fc; "
                    "margin-bottom: 4px; "
                    "}"
                )
                header_layout.addWidget(title)

                subtitle = QLabel("Generate subtitle files from audio/video media")
                subtitle.setStyleSheet(
                    "QLabel { "
                    "font-size: 14px; "
                    "color: #7d8590; "
                    "margin-bottom: 8px; "
                    "}"
                )
                header_layout.addWidget(subtitle)

                layout.addLayout(header_layout)

                # Drop zone or file chooser
                drop_zone = QWidget()
                drop_zone.setMinimumHeight(120)
                drop_zone.setStyleSheet(
                    "QWidget { "
                    "background: #161b22; "
                    "border: 2px dashed #30363d; "
                    "border-radius: 8px; "
                    "}"
                )
                drop_layout = QVBoxLayout(drop_zone)
                drop_layout.setAlignment(Qt.AlignmentFlag.AlignCenter)

                drop_icon = QLabel("📁")
                drop_icon.setStyleSheet("font-size: 32px;")
                drop_icon.setAlignment(Qt.AlignmentFlag.AlignCenter)
                drop_layout.addWidget(drop_icon)

                drop_text = QLabel("Drag & drop media files here")
                drop_text.setStyleSheet(
                    "QLabel { "
                    "color: #7d8590; "
                    "font-size: 16px; "
                    "font-weight: 500; "
                    "}"
                )
                drop_text.setAlignment(Qt.AlignmentFlag.AlignCenter)
                drop_layout.addWidget(drop_text)

                drop_subtext = QLabel("or click below to browse")
                drop_subtext.setStyleSheet(
                    "QLabel { " "color: #6e7681; " "font-size: 13px; " "}"
                )
                drop_subtext.setAlignment(Qt.AlignmentFlag.AlignCenter)
                drop_layout.addWidget(drop_subtext)

                layout.addWidget(drop_zone)

                # Choose file button
                self.outer.btn_choose = QPushButton("Choose Media File")
                self.outer.btn_choose.setMinimumHeight(36)
                self.outer.btn_choose.clicked.connect(self.outer._choose_file)
                layout.addWidget(self.outer.btn_choose)

                # Log area
                log_label = QLabel("Processing Log")
                log_label.setStyleSheet(
                    "QLabel { "
                    "font-size: 16px; "
                    "font-weight: 500; "
                    "color: #f0f6fc; "
                    "margin-top: 8px; "
                    "margin-bottom: 8px; "
                    "}"
                )
                layout.addWidget(log_label)

                self.outer.srt_log = QTextEdit()
                self.outer.srt_log.setReadOnly(True)
                self.outer.srt_log.setPlaceholderText(
                    "SRT generation progress will appear here..."
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

    def close_app(self):
        self.model_manager.unload()
        from PySide6.QtWidgets import (
            QApplication,
        )  # local import to avoid cyclic issues

        app = QApplication.instance()
        if app:
            app.quit()
        self.close()
