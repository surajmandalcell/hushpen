"""
Whisper-specific result processor with enhanced statistics and metadata.
"""

from typing import Dict, Any, List
from .base import BaseProcessor


class WhisperProcessor(BaseProcessor):
    """Processor for Whisper model results."""

    def __init__(self, model_id: str):
        super().__init__(model_id, "whisper")

    def create_specials(self, raw_result: Dict[str, Any]) -> List[Dict[str, Any]]:
        """Create Whisper-specific special fields."""
        specials = []

        # Whisper model information
        model_info = {
            "type": "whisper_model_info",
            "data": {
                "model_size": self._extract_model_size(self.model_id),
                "is_multilingual": self._is_multilingual_model(self.model_id),
                "supported_languages": self._get_supported_languages(self.model_id),
                "parameter_count": self._get_parameter_count(self.model_id)
            }
        }
        specials.append(model_info)

        # Language detection results
        if "language" in raw_result and "language_probability" in raw_result:
            language_detection = {
                "type": "language_detection",
                "data": {
                    "detected_language": raw_result["language"],
                    "confidence": raw_result["language_probability"],
                    "is_confident": raw_result["language_probability"] > 0.8
                }
            }
            specials.append(language_detection)

        # Performance metrics
        timing = raw_result.get("timing", {})
        if timing:
            performance_metrics = {
                "type": "performance_metrics",
                "data": {
                    "processing_efficiency": self._calculate_efficiency(timing),
                    "words_per_second": self._calculate_words_per_second(raw_result, timing),
                    "memory_efficient": timing.get("chunks_processed", 1) == 1
                }
            }
            specials.append(performance_metrics)

        # Segment analysis for quality assessment
        segments = raw_result.get("segments", [])
        if segments:
            segment_analysis = {
                "type": "segment_analysis",
                "data": {
                    "total_segments": len(segments),
                    "avg_segment_length": self._calculate_avg_segment_length(segments),
                    "silence_detection": self._analyze_silence_patterns(segments),
                    "quality_indicators": self._assess_transcription_quality(segments)
                }
            }
            specials.append(segment_analysis)

        return specials

    def _extract_model_size(self, model_id: str) -> str:
        """Extract model size from model ID."""
        if "tiny" in model_id.lower():
            return "tiny"
        elif "base" in model_id.lower():
            return "base"
        elif "small" in model_id.lower():
            return "small"
        elif "medium" in model_id.lower():
            return "medium"
        elif "large" in model_id.lower():
            return "large"
        return "unknown"

    def _is_multilingual_model(self, model_id: str) -> bool:
        """Check if the model supports multiple languages."""
        # For now, assume all whisper models are multilingual except tiny
        return "tiny" not in model_id.lower()

    def _get_supported_languages(self, model_id: str) -> List[str]:
        """Get list of supported languages for the model."""
        # Simplified - in practice, this would be more comprehensive
        if "tiny" in model_id.lower():
            return ["en", "hi"]
        else:
            return ["en", "hi", "es", "fr", "de", "it", "pt", "ru", "ko", "zh", "ja", "ar"]

    def _get_parameter_count(self, model_id: str) -> str:
        """Get approximate parameter count for the model."""
        size_to_params = {
            "tiny": "39M",
            "base": "74M",
            "small": "244M",
            "medium": "769M",
            "large": "1550M"
        }
        size = self._extract_model_size(model_id)
        return size_to_params.get(size, "unknown")

    def _calculate_efficiency(self, timing: Dict[str, Any]) -> float:
        """Calculate processing efficiency ratio."""
        ai_time = timing.get("ai_transcription_time", 0)
        total_time = timing.get("total_processing_time", ai_time)
        if total_time > 0:
            return round(ai_time / total_time, 2)
        return 0.0

    def _calculate_words_per_second(self, raw_result: Dict[str, Any], timing: Dict[str, Any]) -> float:
        """Calculate transcription speed in words per second."""
        text = raw_result.get("text", "")
        word_count = len(text.split()) if text else 0
        ai_time = timing.get("ai_transcription_time", 0)
        if ai_time > 0:
            return round(word_count / ai_time, 2)
        return 0.0

    def _calculate_avg_segment_length(self, segments: List[Dict[str, Any]]) -> float:
        """Calculate average segment duration."""
        if not segments:
            return 0.0

        total_duration = sum(
            segment.get("end", 0) - segment.get("start", 0)
            for segment in segments
        )
        return round(total_duration / len(segments), 2)

    def _analyze_silence_patterns(self, segments: List[Dict[str, Any]]) -> Dict[str, Any]:
        """Analyze silence patterns between segments."""
        if len(segments) < 2:
            return {"gaps_detected": 0, "avg_gap_duration": 0.0}

        gaps = []
        for i in range(1, len(segments)):
            gap = segments[i].get("start", 0) - segments[i-1].get("end", 0)
            if gap > 0:
                gaps.append(gap)

        return {
            "gaps_detected": len(gaps),
            "avg_gap_duration": round(sum(gaps) / len(gaps), 2) if gaps else 0.0,
            "max_gap_duration": round(max(gaps), 2) if gaps else 0.0
        }

    def _assess_transcription_quality(self, segments: List[Dict[str, Any]]) -> Dict[str, Any]:
        """Assess overall transcription quality indicators."""
        if not segments:
            return {"confidence": "unknown"}

        # Simple quality indicators based on segment characteristics
        avg_segment_length = self._calculate_avg_segment_length(segments)
        total_text_length = sum(
            len(segment.get("text", "").strip())
            for segment in segments
        )

        quality = "good"
        if avg_segment_length < 1.0:  # Very short segments might indicate choppy audio
            quality = "fair"
        elif total_text_length < 10:  # Very little transcribed text
            quality = "poor"
        elif avg_segment_length > 30.0:  # Very long segments might indicate missing punctuation
            quality = "fair"

        return {
            "confidence": quality,
            "total_characters": total_text_length,
            "avg_chars_per_segment": round(total_text_length / len(segments), 1) if segments else 0
        }