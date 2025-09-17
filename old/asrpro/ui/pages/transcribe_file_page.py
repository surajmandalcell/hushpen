"""Transcribe File page: pick a media file and generate SRT."""

from __future__ import annotations

from pathlib import Path
from PySide6.QtCore import Qt
from PySide6.QtGui import QFont
from PySide6.QtWidgets import QWidget, QVBoxLayout, QHBoxLayout, QLabel, QFileDialog, QMessageBox

from .base_page import BasePage
from ..styles.dark_theme import DarkTheme, Fonts
from ..components.typography import BodyLabel
from ..components.button import MacButton
from ..components.file_picker import FilePicker
from ..layouts.panel import Panel


class TranscribeFilePage(BasePage):
    """Simple page to select a media file and transcribe it to SRT."""

    def __init__(self, parent=None):
        super().__init__(title="Transcribe File", parent=parent)
        self._selected_path: Path | None = None
        self._build_body()

    def _build_body(self):
        # Instructions
        self.instructions = BodyLabel(
            "Choose an audio/video file and transcribe to SRT using the current model."
        )
        self.instructions.setWordWrap(True)
        self.add_content_widget(self.instructions)

        # File row
        row = Panel()
        row_layout = QHBoxLayout(row)
        row_layout.setContentsMargins(12, 12, 12, 12)
        row_layout.setSpacing(12)
        self.file_picker = FilePicker(self, filter="Media Files (*.wav *.mp3 *.mp4 *.avi *.mkv *.m4a *.flac *.ogg)")
        row_layout.addWidget(self.file_picker)
        self.add_content_widget(row)

        # Action row
        action_row = Panel()
        action_layout = QHBoxLayout(action_row)
        action_layout.setContentsMargins(12, 12, 12, 12)
        action_layout.setSpacing(12)
        self.transcribe_btn = MacButton("Transcribe to SRT", primary=True)
        self.transcribe_btn.clicked.connect(self._transcribe)
        self.status_label = BodyLabel("")

        action_layout.addWidget(self.transcribe_btn)
        action_layout.addWidget(self.status_label, 1)
        self.add_content_widget(action_row)

        self.add_stretch()

    def _pick_file(self):
        file, _ = QFileDialog.getOpenFileName(
            self,
            "Select Media File",
            "",
            "Media Files (*.wav *.mp3 *.mp4 *.avi *.mkv *.m4a *.flac *.ogg)"
        )
        if file:
            self._selected_path = Path(file)

    def _transcribe(self):
        sel = self.file_picker.path()
        if sel is not None:
            self._selected_path = sel
        if not self._selected_path:
            QMessageBox.warning(self, "Transcribe", "Please choose a media file first.")
            return

        self.transcribe_btn.setEnabled(False)
        self.status_label.setText("Transcribing…")
        self.status_label.repaint()

        try:
            # Access main window's model manager
            mw = self.window()
            model_manager = getattr(mw, "model_manager", None)
            if model_manager is None:
                raise RuntimeError("Model manager not available")

            srt_text = model_manager.transcribe(str(self._selected_path), return_srt=True)

            # Suggest SRT output next to source file
            out_path = self._selected_path.with_suffix(".srt")
            # Ask where to save
            save_path, _ = QFileDialog.getSaveFileName(
                self,
                "Save SRT",
                str(out_path),
                "SubRip (*.srt)"
            )
            if save_path:
                Path(save_path).write_text(srt_text, encoding="utf-8")
                QMessageBox.information(self, "Transcribe", f"Saved SRT to:\n{save_path}")
                self.status_label.setText("Done")
            else:
                self.status_label.setText("Cancelled save")
        except Exception as e:
            QMessageBox.critical(self, "Transcribe failed", str(e))
            self.status_label.setText("Failed")
        finally:
            self.transcribe_btn.setEnabled(True)
