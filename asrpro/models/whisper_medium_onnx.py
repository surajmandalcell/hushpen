"""Loader for Whisper Medium ONNX via faster-whisper."""

from __future__ import annotations
from .base import BaseLoader


class WhisperMediumOnnxLoader(BaseLoader):
    model_name = "medium"

    def load(self, progress_cb=None):  # pragma: no cover heavy
        if progress_cb:
            progress_cb("Importing faster_whisper (Whisper Medium)...")
        from faster_whisper import WhisperModel  # type: ignore

        compute_type = "float16" if self.device == "cuda" else "int8"
        model = WhisperModel(
            self.model_name,
            device=self.device if self.device != "vulkan" else "cpu",
            compute_type=compute_type,
        )

        class Wrapped:
            def transcribe_file(self, wav_path: str):
                segments, _ = model.transcribe(wav_path, vad_filter=True)
                return [
                    {"start": seg.start, "end": seg.end, "text": seg.text.strip()}
                    for seg in segments
                ]

        return Wrapped(), None

    def transcribe_file(self, wav_path: str):
        raise RuntimeError("Use wrapper")
