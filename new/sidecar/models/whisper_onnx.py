"""
ONNX Whisper model loader for ASR Pro Python Sidecar
"""

import tempfile
import os
import logging
from typing import Dict, Any, BinaryIO

from .base import BaseLoader
from utils.audio_converter import convert_to_wav

logger = logging.getLogger(__name__)


class WhisperONNXLoader(BaseLoader):
    """ONNX loader for Whisper models using onnx-asr."""

    def __init__(self, model_id: str, config: Dict[str, Any]):
        super().__init__(model_id, config)
        self.whisper_model = None

    async def load(self) -> bool:
        """Load the ONNX Whisper model."""
        try:
            import onnx_asr

            device = self.config.get("device", "cpu")

            # Map our model IDs to onnx-asr model names
            model_mapping = {
                "whisper-tiny": "whisper-tiny",
                "whisper-base": "whisper-base",
                "whisper-small": "whisper-small",
            }

            onnx_model_name = model_mapping.get(self.model_id, self.model_id)

            logger.info(f"Loading ONNX Whisper model {self.model_id} on {device}")

            # Load ONNX model (device selection is handled internally by onnx-asr)
            self.whisper_model = onnx_asr.load_model(onnx_model_name)

            self.is_loaded = True
            logger.info(
                f"ONNX Whisper model {self.model_id} loaded successfully on {device}"
            )
            return True

        except ImportError as e:
            logger.error(f"onnx-asr not installed: {e}")
            logger.error("Install with: pip install onnx-asr[gpu,hub]")
            return False
        except Exception as e:
            logger.error(f"Failed to load ONNX Whisper model {self.model_id}: {e}")
            return False

    async def unload(self) -> bool:
        """Unload the ONNX Whisper model."""
        try:
            if self.whisper_model:
                del self.whisper_model
                self.whisper_model = None

            self.is_loaded = False
            logger.info(f"ONNX Whisper model {self.model_id} unloaded successfully")
            return True

        except Exception as e:
            logger.error(f"Failed to unload ONNX Whisper model {self.model_id}: {e}")
            return False

    async def transcribe(self, audio_file: BinaryIO) -> Dict[str, Any]:
        """Transcribe audio file using ONNX Whisper."""
        if not self.is_ready():
            raise Exception("Model not loaded")

        try:
            # Convert audio to WAV format
            wav_path = convert_to_wav(audio_file, target_sample_rate=16000)

            try:
                # Transcribe using ONNX Whisper
                transcription = self.whisper_model.recognize(wav_path)

                # Extract text
                text = (
                    transcription
                    if isinstance(transcription, str)
                    else str(transcription)
                )

                # Create segments (ONNX Whisper doesn't provide timestamps by default)
                segments = [{"start": 0.0, "end": 0.0, "text": text}]

                return {
                    "text": text,
                    "segments": segments,
                    "language": "en",  # Default to English
                    "language_probability": 1.0,
                    "duration": 0.0,  # Not provided by ONNX
                }

            finally:
                # Clean up temporary WAV file
                if os.path.exists(wav_path):
                    os.unlink(wav_path)

        except Exception as e:
            logger.error(f"Failed to transcribe with ONNX Whisper: {e}")
            raise
