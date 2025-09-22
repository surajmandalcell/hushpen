"""
ModelManager: lifecycle, loader creation, and inference entry points.
"""

import logging
from typing import Dict, Any, Optional, List, BinaryIO

from utils.device import DeviceDetector
from .registry import ModelRegistry
from .base import ONNXBaseLoader
from .loaders import ConfigDrivenLoader


logger = logging.getLogger(__name__)


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
        # Use a generic 'config' key since registry loaders are 'config'
        self.loader_configs = {
            "config": {
                "device": device_config.get("device", "cpu"),
                "compute_type": device_config.get("compute_type", "float32"),
                "backend": device_config.get("device", "cpu"),
            }
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

        model_info = self.registry.get_model_info(model_id)
        if not model_info:
            logger.error(f"No model info found for {model_id}")
            return None

        config = self.loader_configs.get(loader_type, {}).copy()
        config.update(model_info)

        try:
            # Single configurable loader keeps code DRY; behavior comes from registry config
            if loader_type != "config":
                logger.warning(
                    f"Loader type '{loader_type}' not 'config'; using ConfigDrivenLoader for {model_id}"
                )
            logger.info(
                f"Creating loader for {model_id} with backend '{config.get('backend', config.get('device', 'cpu'))}'"
            )
            loader = ConfigDrivenLoader(model_id, config)
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
