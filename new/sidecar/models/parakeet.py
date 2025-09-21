"""
Parakeet model loader for ASR Pro Python Sidecar
"""

import logging
from typing import Dict, Any, BinaryIO

from .base import BaseLoader

logger = logging.getLogger(__name__)


class ParakeetLoader(BaseLoader):
    """Loader for Parakeet models (placeholder)."""

    def __init__(self, model_id: str, config: Dict[str, Any]):
        super().__init__(model_id, config)

    async def load(self) -> bool:
        """Load the Parakeet model."""
        logger.warning("Parakeet models are not implemented yet")
        return False

    async def unload(self) -> bool:
        """Unload the Parakeet model."""
        self.is_loaded = False
        return True

    async def transcribe(self, audio_file: BinaryIO) -> Dict[str, Any]:
        """Transcribe audio file using Parakeet."""
        raise NotImplementedError("Parakeet models not implemented")
