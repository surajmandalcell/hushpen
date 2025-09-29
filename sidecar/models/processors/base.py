"""
Base processor for handling transcription results and metadata standardization.
"""

from abc import ABC, abstractmethod
from typing import Dict, Any, List, Optional
import time


class BaseProcessor(ABC):
    """Base class for model-specific transcription result processors."""

    def __init__(self, model_id: str, model_family: str):
        self.model_id = model_id
        self.model_family = model_family

    def create_common_metadata(self,
                             raw_result: Dict[str, Any],
                             audio_duration: float,
                             processing_start_time: float) -> Dict[str, Any]:
        """Create common metadata shared across all models."""
        current_time = time.time()
        total_processing_time = current_time - processing_start_time

        metadata = {
            "model": self.model_id,
            "model_family": self.model_family,
            "audio_duration_seconds": audio_duration,
            "processing_time_seconds": round(total_processing_time, 2),
            "backend": raw_result.get("backend", "unknown"),
            "timestamp": current_time,
            "real_time_factor": round(audio_duration / total_processing_time, 2) if total_processing_time > 0 else 0
        }

        # Add common timing info if available
        timing = raw_result.get("timing", {})
        if timing:
            metadata.update({
                "audio_conversion_time_seconds": timing.get("audio_conversion_time", 0),
                "ai_transcription_time_seconds": timing.get("ai_transcription_time", 0),
                "chunks_processed": timing.get("chunks_processed", 1)
            })

        return metadata

    @abstractmethod
    def create_specials(self, raw_result: Dict[str, Any]) -> List[Dict[str, Any]]:
        """Create model-specific special fields from raw transcription result."""
        pass

    def process_result(self,
                      raw_result: Dict[str, Any],
                      audio_duration: float,
                      processing_start_time: float) -> Dict[str, Any]:
        """Process raw transcription result into standardized format with metadata and specials."""

        # Create base response with existing fields
        result = {
            "text": raw_result.get("text", ""),
            "segments": raw_result.get("segments", []),
            "language": raw_result.get("language", "en"),
            "language_probability": raw_result.get("language_probability", 1.0),
            "duration": audio_duration
        }

        # Add common metadata
        result["metadata"] = self.create_common_metadata(
            raw_result, audio_duration, processing_start_time
        )

        # Add model-specific specials
        result["specials"] = self.create_specials(raw_result)

        return result