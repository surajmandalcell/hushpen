"""
ONNX Parakeet model loader for ASR Pro Python Sidecar
"""

import tempfile
import os
import logging
from typing import Dict, Any, BinaryIO

from .base import BaseLoader
from utils.audio_converter import convert_to_wav

logger = logging.getLogger(__name__)


class ParakeetLoader(BaseLoader):
    """ONNX loader for Parakeet models using onnx-asr."""

    def __init__(self, model_id: str, config: Dict[str, Any]):
        super().__init__(model_id, config)
        self.parakeet_model = None

    async def load(self) -> bool:
        """Load the ONNX Parakeet model."""
        try:
            import onnx_asr

            device = self.config.get("device", "cpu")

            # Map model IDs to onnx-asr model names
            model_mapping = {"parakeet-tdt-0.6b-v2": "nemo-parakeet-tdt-0.6b-v2"}

            onnx_model_name = model_mapping.get(self.model_id, self.model_id)

            logger.info(f"Loading ONNX Parakeet model {self.model_id} on {device}")

            # Load ONNX model (device selection is handled internally by onnx-asr)
            self.parakeet_model = onnx_asr.load_model(onnx_model_name)

            self.is_loaded = True
            logger.info(
                f"ONNX Parakeet model {self.model_id} loaded successfully on {device}"
            )
            return True

        except ImportError as e:
            logger.error(f"onnx-asr not installed: {e}")
            logger.error("Install with: pip install onnx-asr[gpu,hub]")
            return False
        except Exception as e:
            logger.error(f"Failed to load ONNX Parakeet model {self.model_id}: {e}")
            return False

    async def unload(self) -> bool:
        """Unload the ONNX Parakeet model."""
        try:
            if self.parakeet_model:
                del self.parakeet_model
                self.parakeet_model = None

            self.is_loaded = False
            logger.info(f"ONNX Parakeet model {self.model_id} unloaded successfully")
            return True

        except Exception as e:
            logger.error(f"Failed to unload ONNX Parakeet model {self.model_id}: {e}")
            return False

    async def transcribe(self, audio_file: BinaryIO) -> Dict[str, Any]:
        """Transcribe audio file using ONNX Parakeet."""
        if not self.is_ready():
            raise Exception("Model not loaded")

        try:
            # Convert audio to WAV format
            wav_path = convert_to_wav(audio_file, target_sample_rate=16000)

            try:
                # Transcribe using ONNX Parakeet
                transcription = self.parakeet_model.recognize(wav_path)

                # Extract text
                text = (
                    transcription
                    if isinstance(transcription, str)
                    else str(transcription)
                )

                # Create segments (ONNX Parakeet doesn't provide timestamps by default)
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
            logger.error(f"Failed to transcribe with ONNX Parakeet: {e}")
            raise
