"""Main window with Models and SRT tabs, global hotkey toggle integration."""

from __future__ import annotations
import tempfile
from pathlib import Path
from PySide6.QtCore import Qt
from PySide6.QtGui import QClipboard
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
)
from ..model_manager import ModelManager, MODEL_SPECS
from ..hotkey import ToggleHotkey
from ..audio_recorder import AudioRecorder
from .custom_titlebar import TitleBar
from .overlay import Overlay


class MainWindow(QWidget):  # pragma: no cover
    def __init__(self):
        super().__init__()
        self.setWindowFlags(Qt.FramelessWindowHint | Qt.Window)
        self.setFixedSize(800, 600)
        self.model_manager = ModelManager()
        self.recorder = AudioRecorder()
        self.overlay = Overlay()
        self._record_temp = Path(tempfile.gettempdir()) / "asrpro_record.wav"
        self.hotkey = ToggleHotkey(self._on_toggle)
        self.hotkey.start()
        self._build_ui()

    def _build_ui(self):
        root = QVBoxLayout(self)
        root.setContentsMargins(0, 0, 0, 0)
        self.titlebar = TitleBar(self)
        root.addWidget(self.titlebar)
        self.tabs = QTabWidget()
        self.tabs.addTab(self._models_tab(), "Models")
        self.tabs.addTab(self._srt_tab(), "SRT")
        root.addWidget(self.tabs)
        self.setStyleSheet(
            """
        QWidget { background:#1e1f22; color:#e8e8e8; font-family:'Segoe UI',sans-serif; }
        QTabBar::tab { padding:6px 12px; background:#2a2c30; border-radius:4px; margin-right:4px; }
        QTabBar::tab:selected { background:#3c7cff; }
        QPushButton { background:#3c7cff; color:white; border:none; padding:6px 12px; border-radius:4px; }
        QPushButton:hover { background:#285fd1; }
        QListWidget { background:#232528; border:1px solid #2f3336; }
        QTextEdit { background:#232528; border:1px solid #2f3336; }
        #TitleBar { background:#161719; }
        #TitleLabel { font-weight:600; }
        """
        )

    def _models_tab(self):
        w = QWidget()
        layout = QVBoxLayout(w)
        self.model_list = QListWidget()
        for mid in MODEL_SPECS.keys():
            self.model_list.addItem(mid)
        layout.addWidget(QLabel("Select a model to load (only one stays in memory)."))
        layout.addWidget(self.model_list)
        self.btn_load = QPushButton("Load Model")
        self.btn_unload = QPushButton("Unload")
        info = QLabel(f"Device preference: {self.model_manager.device}")
        info.setStyleSheet("color:#9aa0a6; font-size:12px")
        hl = QHBoxLayout()
        hl.addWidget(self.btn_load)
        hl.addWidget(self.btn_unload)
        layout.addLayout(hl)
        layout.addWidget(info)
        self.output = QTextEdit()
        self.output.setReadOnly(True)
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
                layout.addWidget(
                    QLabel(
                        "Drag & drop or click to choose media file; SRT will be created next to it."
                    )
                )
                self.btn_choose = QPushButton("Choose File")
                self.btn_choose.clicked.connect(self.outer._choose_file)
                layout.addWidget(self.btn_choose)
                self.outer.srt_log = QTextEdit()
                self.outer.srt_log.setReadOnly(True)
                self.outer.srt_log.setPlaceholderText("Drop media here…")
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
                            text_parts.append(s.get("text", ""))
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
        cb.setText(text, QClipboard.Clipboard)
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
