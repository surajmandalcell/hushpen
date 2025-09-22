"""
Model management module for ASR Pro Python Sidecar
"""

import asyncio
from typing import Dict, Any, Optional, List, BinaryIO
import logging

from .registry import ModelRegistry
from .whisper import WhisperLoader
from .whisper_onnx import WhisperONNXLoader
from .parakeet import ParakeetLoader
from utils.device import DeviceDetector

logger = logging.getLogger(__name__)


class ModelManager:
    """Manages model loading, unloading, and operations."""

    def __init__(self, settings):
        self.settings = settings
        self.registry = ModelRegistry()
        self.device_detector = DeviceDetector()
        self.current_model = None
        self.current_loader = None
        self.loaders = {}
        self.loader_configs = {}

    async def initialize(self):
        """Initialize the model manager."""
        logger.info("Initializing model manager")

        # Detect device capabilities
        await self.device_detector.detect_capabilities()

        # Initialize loader configurations
        self._initialize_loader_configs()

        logger.info("Model manager initialized")

    def _initialize_loader_configs(self):
        """Initialize loader configurations."""
        device_config = self.device_detector.get_device_config()

        self.loader_configs = {
            "whisper": {
                "device": device_config.get("device", "cpu"),
                "compute_type": device_config.get("compute_type", "float16"),
            },
            "parakeet": {
                "device": device_config.get("device", "cpu"),
            },
        }

    async def list_available_models(self) -> List[str]:
        """List all available models."""
        return self.registry.list_models()

    def get_current_model(self) -> Optional[str]:
        """Get the currently loaded model."""
        return self.current_model

    def get_current_device(self) -> str:
        """Get the current device being used."""
        return self.device_detector.get_current_device()

    def is_model_ready(self, model_id: str) -> bool:
        """Check if a model is ready."""
        if model_id == self.current_model and self.current_loader:
            return self.current_loader.is_ready()
        return False

    async def set_model(self, model_id: str) -> bool:
        """Set the active model."""
        try:
            # Check if model is available
            if not self.registry.is_model_available(model_id):
                logger.error(f"Model {model_id} is not available")
                return False

            # If same model is already loaded, return success
            if model_id == self.current_model and self.is_model_ready(model_id):
                logger.info(f"Model {model_id} is already loaded")
                return True

            # Unload current model if different
            if self.current_model and self.current_loader:
                logger.info(f"Unloading current model {self.current_model}")
                await self.current_loader.unload()
                self.current_model = None
                self.current_loader = None

            # Get or create loader for the model
            loader = await self._get_loader(model_id)
            if not loader:
                logger.error(f"Failed to create loader for model {model_id}")
                return False

            # Load the model
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
        """Get or create a loader for the model."""
        # Check if loader already exists
        if model_id in self.loaders:
            return self.loaders[model_id]

        # Get loader type
        loader_type = self.registry.get_loader_type(model_id)
        if not loader_type:
            logger.error(f"No loader type found for model {model_id}")
            return None

        # Get model info
        model_info = self.registry.get_model_info(model_id)
        if not model_info:
            logger.error(f"No model info found for {model_id}")
            return None

        # Get loader configuration
        config = self.loader_configs.get(loader_type, {}).copy()
        config.update(model_info)

        # Create loader
        try:
            if loader_type == "whisper":
                loader = WhisperLoader(model_id, config)
            elif loader_type == "whisper_onnx":
                loader = WhisperONNXLoader(model_id, config)
            elif loader_type == "parakeet":
                loader = ParakeetLoader(model_id, config)
            else:
                logger.error(f"Unknown loader type: {loader_type}")
                return None

            self.loaders[model_id] = loader
            return loader

        except Exception as e:
            logger.error(f"Failed to create loader for model {model_id}: {e}")
            return None

    async def transcribe_file(
        self, audio_file: BinaryIO, model_id: Optional[str] = None
    ) -> Dict[str, Any]:
        """Transcribe an audio file."""
        try:
            # Use specified model or current model
            target_model = model_id or self.current_model
            if not target_model:
                raise Exception("No model specified or loaded")

            # Ensure model is loaded
            if target_model != self.current_model:
                if not await self.set_model(target_model):
                    raise Exception(f"Failed to load model {target_model}")

            # Transcribe using current loader
            if not self.current_loader:
                raise Exception("No loader available")

            return await self.current_loader.transcribe(audio_file)

        except Exception as e:
            logger.error(f"Failed to transcribe file: {e}")
            raise

    async def unload_model(self, model_id: str) -> bool:
        """Unload a specific model."""
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
        """Unload all models."""
        try:
            # Unload current model
            if self.current_loader:
                await self.current_loader.unload()
                self.current_model = None
                self.current_loader = None

            # Unload all cached loaders
            for model_id, loader in self.loaders.items():
                await loader.unload()

            self.loaders.clear()
            logger.info("All models unloaded successfully")
            return True

        except Exception as e:
            logger.error(f"Failed to unload all models: {e}")
            return False

    async def get_model_info(self, model_id: str) -> Optional[Dict[str, Any]]:
        """Get information about a model."""
        if model_id in self.loaders:
            return self.loaders[model_id].get_model_info()
        else:
            return self.registry.get_model_info(model_id)

    async def cleanup(self):
        """Cleanup resources."""
        logger.info("Cleaning up model manager")
        await self.unload_all_models()


# Export the main classes
from .base import BaseLoader

__all__ = [
    "ModelManager",
    "ModelRegistry",
    "BaseLoader",
    "WhisperLoader",
    "WhisperONNXLoader",
    "ParakeetLoader",
]
