"""
Whisper model loader for ASR Pro Python Sidecar
"""

import tempfile
import os
import logging
from typing import Dict, Any, BinaryIO

from .base import BaseLoader

logger = logging.getLogger(__name__)


class WhisperLoader(BaseLoader):
    """Loader for Whisper models."""

    def __init__(self, model_id: str, config: Dict[str, Any]):
        super().__init__(model_id, config)
        self.whisper_model = None

    async def load(self) -> bool:
        """Load the Whisper model."""
        try:
            from faster_whisper import WhisperModel

            device = self.config.get("device", "cpu")
            compute_type = self.config.get("compute_type", "float16")

            # Map our model IDs to faster-whisper model names
            model_mapping = {
                "whisper-tiny": "tiny",
                "whisper-base": "base",
                "whisper-small": "small",
            }
            whisper_model_name = model_mapping.get(self.model_id, self.model_id)

            # Try to load with the detected device, fallback to CPU if unsupported
            try:
                logger.info(f"Loading Whisper model {self.model_id} on {device}")
                self.whisper_model = WhisperModel(
                    whisper_model_name,
                    device=device,
                    compute_type=compute_type,
                )
            except Exception as device_error:
                if device != "cpu":
                    logger.warning(
                        f"Failed to load on {device}, falling back to CPU: {device_error}"
                    )
                    device = "cpu"
                    compute_type = "float32"  # Use float32 for CPU
                    logger.info(f"Loading Whisper model {self.model_id} on {device}")
                    self.whisper_model = WhisperModel(
                        whisper_model_name,
                        device=device,
                        compute_type=compute_type,
                    )
                else:
                    raise device_error

            self.is_loaded = True
            logger.info(
                f"Whisper model {self.model_id} loaded successfully on {device}"
            )
            return True

        except Exception as e:
            logger.error(f"Failed to load Whisper model {self.model_id}: {e}")
            return False

    async def unload(self) -> bool:
        """Unload the Whisper model."""
        try:
            if self.whisper_model:
                del self.whisper_model
                self.whisper_model = None

            self.is_loaded = False
            logger.info(f"Whisper model {self.model_id} unloaded successfully")
            return True

        except Exception as e:
            logger.error(f"Failed to unload Whisper model {self.model_id}: {e}")
            return False

    async def transcribe(self, audio_file: BinaryIO) -> Dict[str, Any]:
        """Transcribe audio file using Whisper."""
        if not self.is_ready():
            raise Exception("Model not loaded")

        try:
            # Save audio to temporary file
            with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as temp_file:
                temp_file.write(audio_file.read())
                temp_path = temp_file.name

            try:
                # Transcribe using faster-whisper
                segments, info = self.whisper_model.transcribe(temp_path)

                # Collect segments
                text_segments = []
                for segment in segments:
                    text_segments.append(
                        {
                            "start": segment.start,
                            "end": segment.end,
                            "text": segment.text.strip(),
                        }
                    )

                # Combine all text
                full_text = " ".join([seg["text"] for seg in text_segments])

                return {
                    "text": full_text,
                    "segments": text_segments,
                    "language": info.language,
                    "language_probability": info.language_probability,
                    "duration": info.duration,
                }

            finally:
                # Clean up temporary file
                if os.path.exists(temp_path):
                    os.unlink(temp_path)

        except Exception as e:
            logger.error(f"Failed to transcribe with Whisper: {e}")
            raise
