"""
ModelManager: lifecycle, loader creation, and inference entry points.
"""

import logging
import os
from typing import Dict, Any, Optional, List, BinaryIO

from utils.device import DeviceDetector
from .registry import ModelRegistry
from .base import BaseLoader
from .loaders import ConfigDrivenLoader, NemoParakeetLoader


logger = logging.getLogger(__name__)


class ModelManager:
    """Manages model loading, unloading, and operations."""

    def __init__(self, settings):
        self.settings = settings
        self.registry = ModelRegistry()
        self.device_detector = DeviceDetector()
        self.current_model = None
        self.current_loader = None
        self.loaders: Dict[str, BaseLoader] = {}
        self.loader_configs: Dict[str, Dict[str, Any]] = {}

    async def initialize(self):
        logger.info("Initializing model manager")
        await self.device_detector.detect_capabilities()
        self._initialize_loader_configs()
        default_model = self.settings.get_config("models.default_model")
        if default_model and should_eager_load_default_model():
            if await self.set_model(default_model):
                logger.info(f"Default model {default_model} loaded")
            else:
                logger.warning(f"Default model {default_model} could not be loaded")
        elif default_model:
            logger.info(
                f"Skipping eager load for default model {default_model}; it will load on first transcription or model selection"
            )
        logger.info("Model manager initialized")

    def _initialize_loader_configs(self):
        device_config = self.device_detector.get_device_config()
        base_config = {
            "device": device_config.get("device", "cpu"),
            "compute_type": device_config.get("compute_type", "float32"),
            "backend": device_config.get("device", "cpu"),
            "cache_dir": self.settings.get_config("models.cache_dir"),
        }
        # Support both legacy keys and family-based keys
        self.loader_configs = {
            "config": base_config,
            "whisper": base_config.copy(),
            "parakeet": base_config.copy(),
            "nemo": base_config.copy(),
        }

    async def list_available_models(self) -> List[str]:
        # Include all; loaders handle hub vs local path resolution
        return self.registry.list_models()

    def get_current_model(self) -> Optional[str]:
        return self.current_model

    def get_current_device(self) -> str:
        return self.device_detector.get_current_device()

    def get_current_loader(self):
        if self.current_model and self.current_model in self.loaders:
            return self.loaders[self.current_model]
        return None

    def get_current_backend(self) -> str:
        """Return the actual backend the current loader is using (cuda/cpu/directml/mps),
        falling back to detected device if loader isn't ready yet."""
        if self.current_loader and getattr(
            self.current_loader, "current_backend", None
        ):
            return self.current_loader.current_backend or self.get_current_device()
        return self.get_current_device()

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
        if loader_type not in self.loader_configs:
            logger.error(f"Unknown loader type {loader_type} for model {model_id}")
            return None

        model_info = self.registry.get_model_info(model_id)
        if not model_info:
            logger.error(f"No model info found for {model_id}")
            return None

        config = self.loader_configs.get(loader_type, {}).copy()
        config.update(model_info)

        try:
            if loader_type == "nemo":
                loader = NemoParakeetLoader(model_id, config)
            else:
                # Single configurable loader keeps ONNX behavior DRY; registry supplies candidates.
                if loader_type != "config":
                    logger.warning(
                        f"Loader type '{loader_type}' not 'config'; using ConfigDrivenLoader for {model_id}"
                    )
                loader = ConfigDrivenLoader(model_id, config)

            logger.info(
                f"Creating loader for {model_id} with backend '{config.get('backend', config.get('device', 'cpu'))}'"
            )
            self.loaders[model_id] = loader
            return loader
        except Exception as e:
            logger.error(f"Failed to create loader for model {model_id}: {e}")
            return None

    async def transcribe_file(
        self,
        audio_file: BinaryIO,
        model_id: Optional[str] = None,
        progress_callback=None,
    ) -> Dict[str, Any]:
        try:
            target_model = model_id or self.current_model
            if not target_model:
                raise Exception("No model specified or loaded")

            # Send initial progress notification
            if progress_callback:
                await progress_callback(10, "Preparing model...")

            if target_model != self.current_model:
                if not await self.set_model(target_model):
                    raise Exception(f"Failed to load model {target_model}")

            if not self.current_loader:
                raise Exception("No loader available")

            # Send progress notification for audio conversion
            if progress_callback:
                await progress_callback(30, "Converting audio...")

            result = await self.current_loader.transcribe(audio_file)

            # Send final progress notification
            if progress_callback:
                await progress_callback(100, "Transcription completed")

            return result

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
            # Unload all loaders in the dictionary (includes current loader if loaded)
            for model_id, loader in self.loaders.items():
                await loader.unload()

            # Clear all references
            self.loaders.clear()
            self.current_model = None
            self.current_loader = None

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


def should_eager_load_default_model() -> bool:
    value = os.environ.get("ASRPRO_EAGER_LOAD_MODEL", "1").strip().lower()
    return value not in {"0", "false", "no", "off"}
