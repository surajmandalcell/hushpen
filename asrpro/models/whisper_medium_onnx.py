"""Loader for Whisper Medium ONNX via faster-whisper."""

from __future__ import annotations
from .base import BaseLoader


class WhisperMediumOnnxLoader(BaseLoader):
    model_name = "medium"

    def load(self, progress_cb=None):  # pragma: no cover heavy
        if progress_cb:
            progress_cb("Importing faster_whisper (Whisper Medium)...")
        from faster_whisper import WhisperModel  # type: ignore

        # Enhanced device and compute type selection
        if self.device == "cuda":
            compute_type = "float16"
            device = "cuda"
        elif self.device == "vulkan":
            # For Vulkan, we need to use CPU device but with optimized settings
            compute_type = "int8"
            device = "cpu"
            if progress_cb:
                progress_cb("Note: Using CPU backend with Vulkan optimizations...")
        else:
            compute_type = "int8"
            device = "cpu"

        # Try to create model with enhanced error handling
        try:
            model = WhisperModel(
                self.model_name,
                device=device,
                compute_type=compute_type,
                download_root=None,  # Use default cache
                local_files_only=False,
            )
            if progress_cb:
                progress_cb(
                    f"Model loaded successfully on {device} with {compute_type}"
                )
        except Exception as e:
            if progress_cb:
                progress_cb(f"Fallback: Loading with CPU int8 due to: {e}")
            # Fallback to basic CPU configuration
            model = WhisperModel(
                self.model_name,
                device="cpu",
                compute_type="int8",
            )

        class Wrapped:
            def transcribe_file(self, wav_path: str):
                segments, _ = model.transcribe(
                    wav_path,
                    vad_filter=True,
                    beam_size=1,  # Faster inference
                    best_of=1,  # Faster inference
                )
                return [
                    {"start": seg.start, "end": seg.end, "text": seg.text.strip()}
                    for seg in segments
                ]

        return Wrapped(), None

    def transcribe_file(self, wav_path: str):
        raise RuntimeError("Use wrapper")
