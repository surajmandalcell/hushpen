"""
Model-specific processors for handling transcription results and metadata.
"""

from .base import BaseProcessor
from .whisper_processor import WhisperProcessor
from .parakeet_processor import ParakeetProcessor

__all__ = [
    "BaseProcessor",
    "WhisperProcessor",
    "ParakeetProcessor"
]