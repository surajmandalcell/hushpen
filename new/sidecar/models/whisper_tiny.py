"""
ONNX Whisper Tiny model loader for ASR Pro Python Sidecar
"""

import tempfile
import os
import logging
from typing import Dict, Any, BinaryIO

from .base import BaseLoader
from utils.audio_converter import convert_to_wav

logger = logging.getLogger(__name__)


class WhisperTinyLoader(BaseLoader):
    """ONNX loader for Whisper Tiny model with multiple backend support."""

    def __init__(self, model_id: str, config: Dict[str, Any]):
        super().__init__(model_id, config)
        self.whisper_model = None
        self.current_backend = None

    async def load(self) -> bool:
        """Load the ONNX Whisper Tiny model."""
        try:
            import onnx_asr

            device = self.config.get("device", "cpu")
            backend = self.config.get("backend", "auto")

            logger.info(
                f"Loading ONNX Whisper whisper-tiny model on {device} with {backend} backend"
            )

            # Disable TensorRT to avoid nvinfer_10.dll issues
            os.environ["ONNXRT_DISABLE_TENSORRT"] = "1"
            os.environ["ONNXRUNTIME_DISABLE_TENSORRT"] = "1"

            # Map model IDs to onnx-asr model names
            model_mapping = {
                "whisper-tiny": "whisper-tiny",
            }

            onnx_model_name = model_mapping.get(self.model_id, "whisper-tiny")

            # Load ONNX model with explicit providers to avoid TensorRT
            providers = ["CUDAExecutionProvider", "CPUExecutionProvider"]
            self.whisper_model = onnx_asr.load_model(
                onnx_model_name, providers=providers
            )
            self.current_backend = backend

            self.is_loaded = True
            logger.info(
                f"ONNX Whisper Tiny model loaded successfully on {device} ({backend})"
            )
            return True

        except ImportError as e:
            logger.error(f"onnx-asr not installed: {e}")
            logger.error("Install with: pip install onnx-asr[gpu,hub]")
            return False
        except Exception as e:
            logger.error(f"Failed to load ONNX Whisper Tiny model: {e}")
            return False

    async def unload(self) -> bool:
        """Unload the ONNX Whisper Tiny model."""
        try:
            if self.whisper_model:
                del self.whisper_model
                self.whisper_model = None

            self.is_loaded = False
            self.current_backend = None
            logger.info("ONNX Whisper Tiny model unloaded successfully")
            return True

        except Exception as e:
            logger.error(f"Failed to unload ONNX Whisper Tiny model: {e}")
            return False

    async def transcribe_cuda(self, audio_file: BinaryIO) -> Dict[str, Any]:
        """Transcribe using CUDA backend."""
        if not self.is_ready():
            raise Exception("Model not loaded")

        try:
            # Convert audio to WAV format
            wav_path = convert_to_wav(audio_file, target_sample_rate=16000)

            try:
                # Transcribe using ONNX Whisper with CUDA
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
                    "backend": "cuda",
                }

            finally:
                # Clean up temporary WAV file
                if os.path.exists(wav_path):
                    os.unlink(wav_path)

        except Exception as e:
            logger.error(f"Failed to transcribe with ONNX Whisper Tiny (CUDA): {e}")
            raise

    async def transcribe_vulkan(self, audio_file: BinaryIO) -> Dict[str, Any]:
        """Transcribe using Vulkan backend."""
        if not self.is_ready():
            raise Exception("Model not loaded")

        try:
            # Convert audio to WAV format
            wav_path = convert_to_wav(audio_file, target_sample_rate=16000)

            try:
                # Transcribe using ONNX Whisper with Vulkan
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
                    "backend": "vulkan",
                }

            finally:
                # Clean up temporary WAV file
                if os.path.exists(wav_path):
                    os.unlink(wav_path)

        except Exception as e:
            logger.error(f"Failed to transcribe with ONNX Whisper Tiny (Vulkan): {e}")
            raise

    async def transcribe_cpu(self, audio_file: BinaryIO) -> Dict[str, Any]:
        """Transcribe using CPU backend."""
        if not self.is_ready():
            raise Exception("Model not loaded")

        try:
            # Convert audio to WAV format
            wav_path = convert_to_wav(audio_file, target_sample_rate=16000)

            try:
                # Transcribe using ONNX Whisper with CPU
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
                    "backend": "cpu",
                }

            finally:
                # Clean up temporary WAV file
                if os.path.exists(wav_path):
                    os.unlink(wav_path)

        except Exception as e:
            logger.error(f"Failed to transcribe with ONNX Whisper Tiny (CPU): {e}")
            raise

    async def transcribe(self, audio_file: BinaryIO) -> Dict[str, Any]:
        """Transcribe using the configured backend."""
        backend = self.current_backend or "cpu"

        if backend == "cuda":
            return await self.transcribe_cuda(audio_file)
        elif backend == "vulkan":
            return await self.transcribe_vulkan(audio_file)
        else:
            return await self.transcribe_cpu(audio_file)
