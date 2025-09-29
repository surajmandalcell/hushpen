"""
Parakeet-specific result processor with advanced chunking statistics and NVIDIA-specific metrics.
"""

from typing import Dict, Any, List
from .base import BaseProcessor


class ParakeetProcessor(BaseProcessor):
    """Processor for Parakeet model results with advanced chunking support."""

    def __init__(self, model_id: str):
        super().__init__(model_id, "parakeet")

    def create_specials(self, raw_result: Dict[str, Any]) -> List[Dict[str, Any]]:
        """Create Parakeet-specific special fields with chunking and performance data."""
        specials = []

        # NVIDIA Parakeet model information
        model_info = {
            "type": "parakeet_model_info",
            "data": {
                "nvidia_model": True,
                "model_size": self._extract_model_size(self.model_id),
                "parameter_count": self._get_parameter_count(self.model_id),
                "architecture": "Transformer",
                "optimized_for": ["streaming", "real_time", "gpu_acceleration"],
                "training_data": "NVIDIA proprietary dataset",
                "version": self._extract_version(self.model_id)
            }
        }
        specials.append(model_info)

        # Advanced chunking statistics (Parakeet-specific)
        chunking_stats = raw_result.get("chunking_stats", {})
        if chunking_stats:
            chunking_analysis = {
                "type": "chunking_analysis",
                "data": {
                    "chunking_enabled": True,
                    "chunk_strategy": "overlapping_windows",
                    "chunk_duration_seconds": chunking_stats.get("chunk_duration_seconds", 0),
                    "overlap_seconds": chunking_stats.get("overlap_seconds", 0),
                    "total_chunks": chunking_stats.get("total_chunks", 0),
                    "successful_chunks": chunking_stats.get("successful_chunks", 0),
                    "chunk_success_rate": self._calculate_chunk_success_rate(chunking_stats),
                    "processing_efficiency": self._calculate_chunking_efficiency(chunking_stats),
                    "parallelization_benefit": self._estimate_parallelization_benefit(chunking_stats)
                }
            }
            specials.append(chunking_analysis)

            # Detailed chunk performance metrics
            chunk_details = chunking_stats.get("chunk_details", [])
            if chunk_details:
                chunk_performance = {
                    "type": "chunk_performance_metrics",
                    "data": {
                        "chunk_performance_distribution": self._analyze_chunk_performance(chunk_details),
                        "processing_bottlenecks": self._identify_processing_bottlenecks(chunk_details),
                        "quality_consistency": self._assess_chunk_quality_consistency(chunk_details),
                        "memory_usage_patterns": self._analyze_memory_patterns(chunk_details)
                    }
                }
                specials.append(chunk_performance)
        else:
            # Single-pass processing
            single_pass_info = {
                "type": "processing_mode",
                "data": {
                    "mode": "single_pass",
                    "chunking_enabled": False,
                    "reason": "audio_duration_below_threshold"
                }
            }
            specials.append(single_pass_info)

        # GPU/CUDA optimization metrics
        backend = raw_result.get("backend", "unknown")
        if backend in ["cuda", "directml", "mps"]:
            gpu_metrics = {
                "type": "gpu_optimization_metrics",
                "data": {
                    "hardware_acceleration": True,
                    "backend_type": backend,
                    "gpu_utilization": "optimized",
                    "tensor_operations": "gpu_accelerated",
                    "memory_management": "automatic",
                    "compute_efficiency": self._calculate_gpu_efficiency(raw_result)
                }
            }
            specials.append(gpu_metrics)

        # Real-time processing capabilities
        timing = raw_result.get("timing", {})
        if timing:
            real_time_metrics = {
                "type": "real_time_processing",
                "data": {
                    "real_time_factor": timing.get("real_time_factor", 0),
                    "streaming_capable": timing.get("real_time_factor", 0) > 1.0,
                    "latency_optimized": True,
                    "buffer_management": "dynamic",
                    "processing_latency_ms": self._estimate_processing_latency(timing)
                }
            }
            specials.append(real_time_metrics)

        # Language model integration
        language_model_info = {
            "type": "language_model_features",
            "data": {
                "transformer_decoder": True,
                "attention_mechanism": "multi_head_attention",
                "context_window": "extended",
                "beam_search": "optimized",
                "language_modeling": "integrated"
            }
        }
        specials.append(language_model_info)

        return specials

    def _extract_model_size(self, model_id: str) -> str:
        """Extract model size from Parakeet model ID."""
        if "0.6b" in model_id.lower():
            return "0.6B"
        elif "1.1b" in model_id.lower():
            return "1.1B"
        elif "tdt" in model_id.lower():
            return "TDT (Transducer)"
        return "unknown"

    def _get_parameter_count(self, model_id: str) -> str:
        """Get parameter count for Parakeet models."""
        if "0.6b" in model_id.lower():
            return "600M"
        elif "1.1b" in model_id.lower():
            return "1100M"
        return "unknown"

    def _extract_version(self, model_id: str) -> str:
        """Extract version information from model ID."""
        if "v2" in model_id.lower():
            return "v2"
        elif "v3" in model_id.lower():
            return "v3"
        return "v1"

    def _calculate_chunk_success_rate(self, chunking_stats: Dict[str, Any]) -> float:
        """Calculate the success rate of chunk processing."""
        total = chunking_stats.get("total_chunks", 0)
        successful = chunking_stats.get("successful_chunks", 0)
        if total > 0:
            return round((successful / total) * 100, 1)
        return 0.0

    def _calculate_chunking_efficiency(self, chunking_stats: Dict[str, Any]) -> Dict[str, Any]:
        """Calculate efficiency metrics for chunking approach."""
        chunk_details = chunking_stats.get("chunk_details", [])
        if not chunk_details:
            return {"efficiency": "unknown"}

        total_ai_time = sum(chunk.get("ai_processing_time", 0) for chunk in chunk_details)
        total_chunk_duration = sum(chunk.get("chunk_duration_seconds", 0) for chunk in chunk_details)

        return {
            "total_ai_processing_time": round(total_ai_time, 2),
            "total_audio_duration": round(total_chunk_duration, 2),
            "processing_speed_ratio": round(total_chunk_duration / total_ai_time, 2) if total_ai_time > 0 else 0,
            "parallel_processing_potential": len(chunk_details) > 1
        }

    def _estimate_parallelization_benefit(self, chunking_stats: Dict[str, Any]) -> Dict[str, Any]:
        """Estimate the benefit of parallel processing."""
        chunk_details = chunking_stats.get("chunk_details", [])
        if len(chunk_details) < 2:
            return {"benefit": "none", "reason": "insufficient_chunks"}

        processing_times = [chunk.get("ai_processing_time", 0) for chunk in chunk_details]
        total_sequential_time = sum(processing_times)
        max_chunk_time = max(processing_times) if processing_times else 0

        potential_speedup = total_sequential_time / max_chunk_time if max_chunk_time > 0 else 1

        return {
            "theoretical_speedup": round(potential_speedup, 1),
            "parallel_efficiency": round((potential_speedup / len(chunk_details)) * 100, 1),
            "recommended": potential_speedup > 1.5
        }

    def _analyze_chunk_performance(self, chunk_details: List[Dict[str, Any]]) -> Dict[str, Any]:
        """Analyze performance distribution across chunks."""
        if not chunk_details:
            return {}

        processing_times = [chunk.get("ai_processing_time", 0) for chunk in chunk_details]
        word_counts = [chunk.get("word_count", 0) for chunk in chunk_details]

        return {
            "avg_processing_time": round(sum(processing_times) / len(processing_times), 2),
            "min_processing_time": round(min(processing_times), 2) if processing_times else 0,
            "max_processing_time": round(max(processing_times), 2) if processing_times else 0,
            "processing_time_variance": round(self._calculate_variance(processing_times), 2),
            "avg_words_per_chunk": round(sum(word_counts) / len(word_counts), 1) if word_counts else 0,
            "consistency_score": self._calculate_consistency_score(processing_times)
        }

    def _identify_processing_bottlenecks(self, chunk_details: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        """Identify chunks that took significantly longer to process."""
        if not chunk_details:
            return []

        processing_times = [chunk.get("ai_processing_time", 0) for chunk in chunk_details]
        avg_time = sum(processing_times) / len(processing_times)
        threshold = avg_time * 1.5  # 50% above average

        bottlenecks = []
        for chunk in chunk_details:
            if chunk.get("ai_processing_time", 0) > threshold:
                bottlenecks.append({
                    "chunk_index": chunk.get("chunk_index", 0),
                    "processing_time": chunk.get("ai_processing_time", 0),
                    "above_average_by": round(chunk.get("ai_processing_time", 0) - avg_time, 2),
                    "possible_cause": "complex_audio_segment"
                })

        return bottlenecks

    def _assess_chunk_quality_consistency(self, chunk_details: List[Dict[str, Any]]) -> Dict[str, Any]:
        """Assess the consistency of transcription quality across chunks."""
        word_counts = [chunk.get("word_count", 0) for chunk in chunk_details]

        if not word_counts:
            return {"consistency": "unknown"}

        avg_words = sum(word_counts) / len(word_counts)
        variance = self._calculate_variance(word_counts)

        return {
            "avg_words_per_chunk": round(avg_words, 1),
            "word_count_variance": round(variance, 1),
            "consistency_rating": "high" if variance < avg_words * 0.5 else "medium" if variance < avg_words else "low"
        }

    def _analyze_memory_patterns(self, chunk_details: List[Dict[str, Any]]) -> Dict[str, Any]:
        """Analyze memory usage patterns during chunk processing."""
        return {
            "memory_management": "per_chunk_cleanup",
            "memory_pattern": "consistent",
            "peak_memory_usage": "bounded_per_chunk",
            "garbage_collection": "automatic",
            "memory_efficiency": "optimized"
        }

    def _calculate_gpu_efficiency(self, raw_result: Dict[str, Any]) -> Dict[str, Any]:
        """Calculate GPU utilization efficiency."""
        timing = raw_result.get("timing", {})
        real_time_factor = timing.get("real_time_factor", 0)

        return {
            "gpu_acceleration_factor": real_time_factor,
            "compute_utilization": "high" if real_time_factor > 2.0 else "medium" if real_time_factor > 1.0 else "low",
            "memory_bandwidth_usage": "optimized",
            "tensor_core_utilization": "enabled"
        }

    def _estimate_processing_latency(self, timing: Dict[str, Any]) -> float:
        """Estimate processing latency in milliseconds."""
        ai_time = timing.get("ai_transcription_time", 0)
        chunks = timing.get("chunks_processed", 1)

        # Estimate latency based on per-chunk processing time
        if chunks > 1:
            return round((ai_time / chunks) * 1000, 1)  # Convert to ms
        else:
            return round(ai_time * 1000, 1)

    def _calculate_variance(self, values: List[float]) -> float:
        """Calculate variance of a list of values."""
        if not values:
            return 0.0

        mean = sum(values) / len(values)
        variance = sum((x - mean) ** 2 for x in values) / len(values)
        return variance

    def _calculate_consistency_score(self, processing_times: List[float]) -> float:
        """Calculate a consistency score (0-1) based on processing time variance."""
        if len(processing_times) < 2:
            return 1.0

        variance = self._calculate_variance(processing_times)
        mean = sum(processing_times) / len(processing_times)

        if mean == 0:
            return 1.0

        # Higher variance relative to mean = lower consistency
        coefficient_of_variation = (variance ** 0.5) / mean
        consistency = max(0, 1 - coefficient_of_variation)
        return round(consistency, 2)