"""
Unified model loaders, registry, and manager for ASR Pro Python Sidecar.

Single source of truth for:
- Model loader implementations (ONNX)
- Model registry metadata
- ModelManager lifecycle & backend selection
"""

import logging
import os
from abc import ABC, abstractmethod
from typing import Dict, Any, BinaryIO, Optional, List

from utils.audio_converter import convert_to_wav
from utils.device import DeviceDetector


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
            provider_sets: List[List[str]] = []
            if preferred in ("cuda", "vulkan", "mps"):
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
                    provider_sets.append(["CPUExecutionProvider"])  # placeholder
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
                    "language": "en",
                    "language_probability": 1.0,
                    "duration": 0.0,
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
        return self._transcribe_common(audio_file, "cuda")

    async def transcribe_mps(self, audio_file: BinaryIO) -> Dict[str, Any]:
        return self._transcribe_common(audio_file, "mps")

    async def transcribe_vulkan(self, audio_file: BinaryIO) -> Dict[str, Any]:
        return self._transcribe_common(audio_file, "vulkan")

    async def transcribe_cpu(self, audio_file: BinaryIO) -> Dict[str, Any]:
        return self._transcribe_common(audio_file, "cpu")

    async def transcribe(self, audio_file: BinaryIO) -> Dict[str, Any]:
        backend = self.current_backend or self.config.get("backend", "cpu")
        if backend == "cuda":
            return await self.transcribe_cuda(audio_file)
        if backend == "mps":
            return await self.transcribe_mps(audio_file)
        return await self.transcribe_cpu(audio_file)

    def is_ready(self) -> bool:
        return self.is_loaded

    @abstractmethod
    def _get_model_name(self):
        """Return str or list[str] with candidate model names for onnx-asr."""
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
        pass

    @abstractmethod
    async def unload(self) -> bool:
        pass

    @abstractmethod
    async def transcribe(self, audio_file: BinaryIO) -> Dict[str, Any]:
        pass

    def is_ready(self) -> bool:
        return self.is_loaded

    def get_model_info(self) -> Dict[str, Any]:
        return {
            "model_id": self.model_id,
            "is_loaded": self.is_loaded,
            "config": self.config,
        }


# Concrete loaders
class WhisperLoader(ONNXBaseLoader):
    def _get_model_name(self):
        mapping = {
            "whisper-tiny": ["whisper-tiny_q4", "whisper-tiny_q8", "whisper-tiny"],
            "whisper-base": ["whisper-base_q4", "whisper-base_q8", "whisper-base"],
            "whisper-small": ["whisper-small_q4", "whisper-small_q8", "whisper-small"],
            "whisper-medium": [
                "whisper-medium_q4",
                "whisper-medium_q8",
                "whisper-medium",
            ],
            "whisper-large": ["whisper-large_q4", "whisper-large_q8", "whisper-large"],
        }
        return mapping.get(
            self.model_id, ["whisper-base_q4", "whisper-base_q8", "whisper-base"]
        )


class ParakeetTDTLoader(ONNXBaseLoader):
    def _get_model_name(self):
        return [
            "nemo-parakeet-tdt-0.6b-v2_q4",
            "nemo-parakeet-tdt-0.6b-v2_q8",
            "nemo-parakeet-tdt-0.6b-v2",
        ]


class ModelRegistry:
    """Registry for available models."""

    def __init__(self):
        self._models = self._initialize_models()

    def _initialize_models(self) -> Dict[str, Dict[str, Any]]:
        return {
            "whisper-tiny": {
                "id": "whisper-tiny",
                "name": "Whisper Tiny (ONNX)",
                "description": "OpenAI Whisper tiny model - fast & lightweight - ONNX",
                "type": "whisper",
                "size": "tiny",
                "loader": "whisper",
                "languages": ["en", "hi"],
                "sample_rate": 16000,
            },
            "whisper-base": {
                "id": "whisper-base",
                "name": "Whisper Base (ONNX)",
                "description": "OpenAI Whisper base model (74M parameters) - English/Hindi - ONNX",
                "type": "whisper",
                "size": "base",
                "loader": "whisper",
                "languages": ["en", "hi"],
                "sample_rate": 16000,
            },
            "parakeet-tdt-0.6b-v2": {
                "id": "parakeet-tdt-0.6b-v2",
                "name": "Parakeet TDT 0.6B v2 (ONNX)",
                "description": "NVIDIA Parakeet TDT model (0.6B parameters) - English/Hindi - ONNX",
                "type": "parakeet",
                "size": "0.6b",
                "loader": "parakeet_tdt",
                "languages": ["en", "hi"],
                "sample_rate": 16000,
            },
        }

    def list_models(self) -> List[str]:
        return list(self._models.keys())

    def get_model_info(self, model_id: str) -> Optional[Dict[str, Any]]:
        return self._models.get(model_id)

    def is_model_available(self, model_id: str) -> bool:
        return model_id in self._models

    def get_loader_type(self, model_id: str) -> Optional[str]:
        model_info = self._models.get(model_id)
        return model_info.get("loader") if model_info else None


class ModelManager:
    """Manages model loading, unloading, and operations."""

    def __init__(self, settings):
        self.settings = settings
        self.registry = ModelRegistry()
        self.device_detector = DeviceDetector()
        self.current_model = None
        self.current_loader = None
        self.loaders: Dict[str, ONNXBaseLoader] = {}
        self.loader_configs: Dict[str, Dict[str, Any]] = {}

    async def initialize(self):
        logger.info("Initializing model manager")
        await self.device_detector.detect_capabilities()
        self._initialize_loader_configs()
        logger.info("Model manager initialized")

    def _initialize_loader_configs(self):
        device_config = self.device_detector.get_device_config()
        self.loader_configs = {
            "whisper": {
                "device": device_config.get("device", "cpu"),
                "compute_type": device_config.get("compute_type", "float32"),
                "backend": device_config.get("device", "cpu"),
            },
            "parakeet_tdt": {
                "device": device_config.get("device", "cpu"),
                "compute_type": device_config.get("compute_type", "float32"),
                "backend": device_config.get("device", "cpu"),
            },
        }

    async def list_available_models(self) -> List[str]:
        return self.registry.list_models()

    def get_current_model(self) -> Optional[str]:
        return self.current_model

    def get_current_device(self) -> str:
        return self.device_detector.get_current_device()

    def get_current_loader(self):
        if self.current_model and self.current_model in self.loaders:
            return self.loaders[self.current_model]
        return None

    def is_model_ready(self, model_id: str) -> bool:
        if model_id == self.current_model and self.current_loader:
            return self.current_loader.is_ready()
        return False

    async def set_model(self, model_id: str) -> bool:
        try:
            if not self.registry.is_model_available(model_id):
                logger.error(f"Model {model_id} is not available")
                return False

            if model_id == self.current_model and self.is_model_ready(model_id):
                logger.info(f"Model {model_id} is already loaded")
                return True

            if self.current_model and self.current_loader:
                logger.info(f"Unloading current model {self.current_model}")
                await self.current_loader.unload()
                self.current_model = None
                self.current_loader = None

            loader = await self._get_loader(model_id)
            if not loader:
                logger.error(f"Failed to create loader for model {model_id}")
                return False

            logger.info(f"Loading model {model_id}")
            if not await loader.load():
                logger.error(f"Failed to load model {model_id}")
                return False

            self.current_model = model_id
            self.current_loader = loader
            logger.info(f"Model {model_id} loaded successfully")
            return True

        except Exception as e:
            logger.error(f"Failed to set model {model_id}: {e}")
            return False

    async def _get_loader(self, model_id: str):
        if model_id in self.loaders:
            return self.loaders[model_id]

        loader_type = self.registry.get_loader_type(model_id)
        if not loader_type:
            logger.error(f"No loader type found for model {model_id}")
            return None

        model_info = self.registry.get_model_info(model_id)
        if not model_info:
            logger.error(f"No model info found for {model_id}")
            return None

        config = self.loader_configs.get(loader_type, {}).copy()
        config.update(model_info)

        try:
            loader_map = {
                "whisper": WhisperLoader,
                "parakeet_tdt": ParakeetTDTLoader,
            }
            loader_cls = loader_map.get(loader_type)
            if not loader_cls:
                logger.error(f"Unknown loader type: {loader_type}")
                return None
            loader = loader_cls(model_id, config)
            self.loaders[model_id] = loader
            return loader
        except Exception as e:
            logger.error(f"Failed to create loader for model {model_id}: {e}")
            return None

    async def transcribe_file(
        self, audio_file: BinaryIO, model_id: Optional[str] = None
    ) -> Dict[str, Any]:
        try:
            target_model = model_id or self.current_model
            if not target_model:
                raise Exception("No model specified or loaded")

            if target_model != self.current_model:
                if not await self.set_model(target_model):
                    raise Exception(f"Failed to load model {target_model}")

            if not self.current_loader:
                raise Exception("No loader available")

            return await self.current_loader.transcribe(audio_file)

        except Exception as e:
            logger.error(f"Failed to transcribe file: {e}")
            raise

    async def unload_model(self, model_id: str) -> bool:
        try:
            if model_id in self.loaders:
                loader = self.loaders[model_id]
                await loader.unload()
                del self.loaders[model_id]
                if model_id == self.current_model:
                    self.current_model = None
                    self.current_loader = None
                logger.info(f"Model {model_id} unloaded successfully")
                return True
            else:
                logger.warning(f"Model {model_id} is not loaded")
                return False
        except Exception as e:
            logger.error(f"Failed to unload model {model_id}: {e}")
            return False

    async def unload_all_models(self) -> bool:
        try:
            if self.current_loader:
                await self.current_loader.unload()
                self.current_model = None
                self.current_loader = None

            for model_id, loader in self.loaders.items():
                await loader.unload()
            self.loaders.clear()
            logger.info("All models unloaded successfully")
            return True
        except Exception as e:
            logger.error(f"Failed to unload all models: {e}")
            return False

    async def get_model_info(self, model_id: str) -> Optional[Dict[str, Any]]:
        if model_id in self.loaders:
            return self.loaders[model_id].get_model_info()
        else:
            return self.registry.get_model_info(model_id)

    async def cleanup(self):
        logger.info("Cleaning up model manager")
        await self.unload_all_models()
