"""
Base loader for ASR Pro Python Sidecar
"""

import logging
import os
from abc import ABC, abstractmethod
from typing import Dict, Any, BinaryIO, Optional
from utils.audio_converter import convert_to_wav

logger = logging.getLogger(__name__)


class ONNXBaseLoader(ABC):
    """Base class for ONNX model loaders with common functionality."""

    def __init__(self, model_id: str, config: Dict[str, Any]):
        self.model_id = model_id
        self.config = config
        self.model = None
        self.current_backend = None
        self.is_loaded = False

    async def load(self) -> bool:
        """Load the ONNX model attempting GPU-first with CPU fallback."""
        try:
            import onnx_asr

            preferred = self.config.get("backend", self.config.get("device", "cpu"))

            logger.info(
                f"Loading ONNX {self.model_id} model with preferred backend '{preferred}'"
            )

            # Disable TensorRT to avoid nvinfer_10.dll issues
            os.environ["ONNXRT_DISABLE_TENSORRT"] = "1"
            os.environ["ONNXRUNTIME_DISABLE_TENSORRT"] = "1"

            model_name_or_list = self._get_model_name()

            # Normalize to list of candidate names (supports quantized variants)
            model_candidates = (
                model_name_or_list
                if isinstance(model_name_or_list, list)
                else [model_name_or_list]
            )

            # Try GPU-first providers, then fall back to CPU
            provider_sets = []
            if preferred in ("cuda", "vulkan", "mps"):
                # OnnxRuntime common GPU providers: CUDA, CoreML/MPS (limited), fallback always include CPU
                if preferred == "cuda":
                    provider_sets.append(
                        ["CUDAExecutionProvider", "CPUExecutionProvider"]
                    )
                elif preferred == "mps":
                    provider_sets.append(
                        ["CoreMLExecutionProvider", "CPUExecutionProvider"]
                    )
                else:
                    # Vulkan not directly supported in onnxruntime; fallback to CPU
                    provider_sets.append(
                        ["CPUExecutionProvider"]
                    )  # placeholder until supported
            # Always ensure CPU-only as last resort
            provider_sets.append(["CPUExecutionProvider"])

            last_error: Optional[Exception] = None
            for providers in provider_sets:
                for candidate_name in model_candidates:
                    try:
                        self.model = onnx_asr.load_model(
                            candidate_name, providers=providers
                        )
                        if "CUDAExecutionProvider" in providers:
                            self.current_backend = "cuda"
                        elif "CoreMLExecutionProvider" in providers:
                            self.current_backend = "mps"
                        else:
                            self.current_backend = "cpu"
                        self.is_loaded = True
                        logger.info(
                            f"ONNX {self.model_id} model loaded successfully as '{candidate_name}' ({self.current_backend})"
                        )
                        return True
                    except Exception as e:
                        last_error = e
                        logger.warning(
                            f"Load failed for {candidate_name} with providers {providers}: {e}"
                        )

            if last_error:
                raise last_error
            return False

        except ImportError as e:
            logger.error(f"onnx-asr not installed: {e}")
            logger.error("Install with: pip install onnx-asr[gpu,hub]")
            return False
        except Exception as e:
            logger.error(f"Failed to load ONNX {self.model_id} model: {e}")
            return False

    async def unload(self) -> bool:
        """Unload the ONNX model."""
        try:
            if self.model:
                del self.model
                self.model = None

            self.is_loaded = False
            self.current_backend = None
            logger.info(f"ONNX {self.model_id} model unloaded successfully")
            return True

        except Exception as e:
            logger.error(f"Failed to unload ONNX {self.model_id} model: {e}")
            return False

    def _transcribe_common(self, audio_file: BinaryIO, backend: str) -> Dict[str, Any]:
        """Common transcription logic for all backends."""
        if not self.is_ready():
            raise Exception("Model not loaded")

        try:
            # Convert audio to WAV format
            wav_path = convert_to_wav(audio_file, target_sample_rate=16000)

            try:
                # Transcribe using ONNX model
                transcription = self.model.recognize(wav_path)

                # Extract text
                text = (
                    transcription
                    if isinstance(transcription, str)
                    else str(transcription)
                )

                # Create segments (ONNX models don't provide timestamps by default)
                segments = [{"start": 0.0, "end": 0.0, "text": text}]

                return {
                    "text": text,
                    "segments": segments,
                    "language": "en",  # Default to English
                    "language_probability": 1.0,
                    "duration": 0.0,  # Not provided by ONNX
                    "backend": backend,
                }

            finally:
                # Clean up temporary WAV file
                if os.path.exists(wav_path):
                    os.unlink(wav_path)

        except Exception as e:
            logger.error(
                f"Failed to transcribe with ONNX {self.model_id} ({backend}): {e}"
            )
            raise

    async def transcribe_cuda(self, audio_file: BinaryIO) -> Dict[str, Any]:
        """Transcribe using CUDA backend."""
        return self._transcribe_common(audio_file, "cuda")

    async def transcribe_vulkan(self, audio_file: BinaryIO) -> Dict[str, Any]:
        """Transcribe using Vulkan backend."""
        return self._transcribe_common(audio_file, "vulkan")

    async def transcribe_cpu(self, audio_file: BinaryIO) -> Dict[str, Any]:
        """Transcribe using CPU backend."""
        return self._transcribe_common(audio_file, "cpu")

    async def transcribe(self, audio_file: BinaryIO) -> Dict[str, Any]:
        """Transcribe using the active backend, prefer GPU if available."""
        backend = self.current_backend or self.config.get("backend", "cpu")
        if backend == "cuda":
            return await self.transcribe_cuda(audio_file)
        return await self.transcribe_cpu(audio_file)

    def is_ready(self) -> bool:
        """Check if the model is ready for use."""
        return self.is_loaded

    @abstractmethod
    def _get_model_name(self) -> str:
        """Get the model name for onnx-asr."""
        pass


class BaseLoader(ABC):
    """Base class for model loaders."""

    def __init__(self, model_id: str, config: Dict[str, Any]):
        self.model_id = model_id
        self.config = config
        self.is_loaded = False
        self.model = None

    @abstractmethod
    async def load(self) -> bool:
        """Load the model."""
        pass

    @abstractmethod
    async def unload(self) -> bool:
        """Unload the model."""
        pass

    @abstractmethod
    async def transcribe(self, audio_file: BinaryIO) -> Dict[str, Any]:
        """Transcribe audio file."""
        pass

    def is_ready(self) -> bool:
        """Check if the model is ready."""
        return self.is_loaded

    def get_model_info(self) -> Dict[str, Any]:
        """Get model information."""
        return {
            "model_id": self.model_id,
            "is_loaded": self.is_loaded,
            "config": self.config,
        }
