"""
Base loader interface for ASR Pro Python Sidecar
"""

from abc import ABC, abstractmethod
from typing import Dict, Any, Optional, BinaryIO
import logging

logger = logging.getLogger(__name__)

class BaseLoader(ABC):
    """Abstract base class for model loaders."""
    
    def __init__(self, model_id: str, config: Dict[str, Any]):
        self.model_id = model_id
        self.config = config
        self.model = None
        self.is_loaded = False
    
    @abstractmethod
    async def load(self) -> bool:
        """Load the model."""
        pass
    
    @abstractmethod
    async def unload(self) -> bool:
        """Unload the model and free memory."""
        pass
    
    @abstractmethod
    async def transcribe(self, audio_file: BinaryIO) -> Dict[str, Any]:
        """Transcribe audio file."""
        pass
    
    @abstractmethod
    def get_model_info(self) -> Dict[str, Any]:
        """Get model metadata."""
        pass
    
    def is_ready(self) -> bool:
        """Check if model is loaded and ready."""
        return self.is_loaded and self.model is not None
    
    async def ensure_loaded(self) -> bool:
        """Ensure model is loaded, load if necessary."""
        if not self.is_ready():
            return await self.load()
        return True
