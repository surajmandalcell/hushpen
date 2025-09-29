"""
Base loader abstractions and ONNX runtime loading/transcription utilities.
"""

import logging
import os
from abc import ABC, abstractmethod
from typing import Dict, Any, BinaryIO, Optional, List
from pathlib import Path

from utils.audio_converter import convert_to_wav
from .processors import WhisperProcessor, ParakeetProcessor


logger = logging.getLogger(__name__)


class ONNXBaseLoader(ABC):
    """Base class for ONNX model loaders with common functionality."""

    def __init__(self, model_id: str, config: Dict[str, Any]):
        self.model_id = model_id
        self.config = config
        self.model = None
        self.current_backend = None
        self.is_loaded = False

        # Initialize appropriate processor based on model family
        self.processor = self._create_processor()

    def _create_processor(self):
        """Create appropriate processor based on model family."""
        model_family = self.config.get("family", "")

        if "whisper" in model_family.lower() or "whisper" in self.model_id.lower():
            return WhisperProcessor(self.model_id)
        elif "parakeet" in model_family.lower() or "parakeet" in self.model_id.lower():
            return ParakeetProcessor(self.model_id)
        else:
            # Default to whisper processor for unknown models
            return WhisperProcessor(self.model_id)

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

            # Choose ONNX Runtime providers based on desired backend and OS support
            provider_sets: List[List[str]] = []

            # CUDA path (Windows/Linux with NVIDIA)
            if preferred == "cuda":
                provider_sets.append(["CUDAExecutionProvider", "CPUExecutionProvider"])

            # Apple Silicon path: use CoreML EP as the onnxruntime provider
            elif preferred == "mps":
                provider_sets.append(
                    ["CoreMLExecutionProvider", "CPUExecutionProvider"]
                )

            # Windows DirectML path
            elif preferred == "directml":
                # Some builds expose "DmlExecutionProvider" (ORT Python), others "DirectMLExecutionProvider"
                provider_sets.append(["DmlExecutionProvider", "CPUExecutionProvider"])
                provider_sets.append(
                    ["DirectMLExecutionProvider", "CPUExecutionProvider"]
                )

            # Vulkan: not supported directly by ORT; fallback to CPU
            elif preferred == "vulkan":
                provider_sets.append(["CPUExecutionProvider"])  # placeholder fallback

            # Default: no GPU preference → try CPU
            else:
                provider_sets.append(["CPUExecutionProvider"])

            # Always ensure CPU-only as last resort
            if ["CPUExecutionProvider"] not in provider_sets:
                provider_sets.append(["CPUExecutionProvider"])

            last_error: Optional[Exception] = None
            for providers in provider_sets:
                for candidate_name in model_candidates:
                    try:
                        # Resolve local path if configured for file source or path-like candidate
                        is_file_source = self.config.get("source") == "file"
                        candidate_path = Path(str(candidate_name))
                        resolved = None
                        
                        if (
                            is_file_source
                            or "/" in str(candidate_name)
                            or candidate_path.suffix == ".onnx"
                        ):
                            # Try absolute path first
                            if candidate_path.is_absolute() and candidate_path.exists():
                                resolved = str(candidate_path)
                            else:
                                # Try relative to models/onnx directory
                                base_dir = Path(__file__).parent / "onnx"
                                
                                # Handle different path patterns
                                potential_paths = [
                                    base_dir / candidate_name,  # Direct path
                                    base_dir / candidate_path,  # As provided
                                ]
                                
                                # If candidate_name doesn't include directory, try common patterns
                                if "/" not in str(candidate_name) and not candidate_path.suffix:
                                    potential_paths.extend([
                                        base_dir / candidate_name / "encoder_model.onnx",
                                        base_dir / candidate_name / "decoder_model_merged.onnx",
                                        base_dir / f"{candidate_name}.onnx",
                                    ])
                                
                                for guess in potential_paths:
                                    if guess.exists():
                                        resolved = str(guess)
                                        break
                                        
                        to_load = resolved or candidate_name

                        self.model = onnx_asr.load_model(to_load, providers=providers)
                        if "CUDAExecutionProvider" in providers:
                            self.current_backend = "cuda"
                        elif "CoreMLExecutionProvider" in providers:
                            self.current_backend = "mps"
                        elif (
                            "DmlExecutionProvider" in providers
                            or "DirectMLExecutionProvider" in providers
                        ):
                            self.current_backend = "directml"
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

    def _transcribe_common(self, audio_file: BinaryIO, backend: str, filename: Optional[str] = None) -> Dict[str, Any]:
        """Common transcription logic for all backends."""
        import time

        if not self.is_ready():
            raise Exception("Model not loaded")

        start_time = time.time()

        try:
            # Convert audio to WAV format
            conversion_start = time.time()
            wav_path = convert_to_wav(audio_file, target_sample_rate=16000, original_filename=filename)
            conversion_time = time.time() - conversion_start

            try:
                # Check if this is a Parakeet model and audio is long
                is_parakeet = "parakeet" in self.model_id.lower()

                if is_parakeet:
                    # Get audio duration to check if chunking is needed
                    import librosa
                    audio_data, sample_rate = librosa.load(wav_path, sr=16000)
                    duration_seconds = len(audio_data) / sample_rate

                    logger.info(f"Audio duration: {duration_seconds:.1f}s, Parakeet model: {is_parakeet}")

                    # If audio is longer than 5 minutes, chunk it for Parakeet
                    if duration_seconds > 300:  # 5 minutes
                        logger.info("Audio too long for Parakeet, chunking into smaller segments")
                        transcription_start = time.time()
                        result = self._transcribe_chunked_parakeet(wav_path, backend, duration_seconds)
                        transcription_time = time.time() - transcription_start

                        # Add timing metadata
                        total_time = time.time() - start_time
                        result.update({
                            "timing": {
                                "total_processing_time": round(total_time, 2),
                                "audio_conversion_time": round(conversion_time, 2),
                                "ai_transcription_time": round(transcription_time, 2),
                                "chunks_processed": len(result.get("segments", [])),
                                "audio_duration_seconds": duration_seconds,
                                "real_time_factor": round(duration_seconds / total_time, 2)
                            }
                        })

                        # Process result through model-specific processor
                        return self.processor.process_result(result, duration_seconds, start_time)

                # Standard transcription for non-Parakeet models or short audio
                transcription_start = time.time()
                transcription = self.model.recognize(wav_path)
                transcription_time = time.time() - transcription_start

                # Get audio duration for metadata
                import librosa
                audio_data, sample_rate = librosa.load(wav_path, sr=16000)
                duration_seconds = len(audio_data) / sample_rate

                # Extract text
                text = (
                    transcription
                    if isinstance(transcription, str)
                    else str(transcription)
                )

                # Create segments (ONNX models don't provide timestamps by default)
                segments = [{"start": 0.0, "end": duration_seconds, "text": text}]

                total_time = time.time() - start_time

                raw_result = {
                    "text": text,
                    "segments": segments,
                    "language": "en",
                    "language_probability": 1.0,
                    "duration": duration_seconds,
                    "backend": backend,
                    "timing": {
                        "total_processing_time": round(total_time, 2),
                        "audio_conversion_time": round(conversion_time, 2),
                        "ai_transcription_time": round(transcription_time, 2),
                        "chunks_processed": 1,
                        "audio_duration_seconds": duration_seconds,
                        "real_time_factor": round(duration_seconds / total_time, 2)
                    }
                }

                # Process result through model-specific processor
                return self.processor.process_result(raw_result, duration_seconds, start_time)

            finally:
                # Clean up temporary WAV file
                if os.path.exists(wav_path):
                    os.unlink(wav_path)

        except Exception as e:
            logger.error(
                f"Failed to transcribe with ONNX {self.model_id} ({backend}): {e}"
            )
            raise

    def _transcribe_chunked_parakeet(self, wav_path: str, backend: str, total_duration: float) -> Dict[str, Any]:
        """Transcribe long audio by chunking it for Parakeet models."""
        import librosa
        import soundfile as sf
        import tempfile
        import time

        chunk_start_time = time.time()

        # Load full audio
        audio_data, sample_rate = librosa.load(wav_path, sr=16000)

        # Chunk into 4-minute segments with 30-second overlap
        chunk_duration = 4 * 60  # 4 minutes in seconds
        overlap = 30  # 30 seconds overlap
        chunk_samples = chunk_duration * sample_rate
        overlap_samples = overlap * sample_rate

        transcribed_chunks = []
        segments = []
        chunk_timings = []

        start_sample = 0
        chunk_index = 0

        while start_sample < len(audio_data):
            end_sample = min(start_sample + chunk_samples, len(audio_data))
            chunk_audio = audio_data[start_sample:end_sample]

            # Skip chunks that are too small (less than 1 second)
            if len(chunk_audio) < sample_rate:
                logger.info(f"Skipping chunk {chunk_index + 1} - too small ({len(chunk_audio)} samples)")
                break

            # Save chunk to temporary file
            with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as temp_chunk:
                sf.write(temp_chunk.name, chunk_audio, sample_rate)
                chunk_path = temp_chunk.name

            try:
                # Transcribe chunk with timing
                chunk_transcription_start = time.time()
                logger.info(f"Transcribing chunk {chunk_index + 1} ({start_sample/sample_rate:.1f}s - {end_sample/sample_rate:.1f}s)")
                chunk_transcription = self.model.recognize(chunk_path)
                chunk_transcription_time = time.time() - chunk_transcription_start

                chunk_text = (
                    chunk_transcription
                    if isinstance(chunk_transcription, str)
                    else str(chunk_transcription)
                )

                # Record chunk timing info
                chunk_timings.append({
                    "chunk_index": chunk_index + 1,
                    "start_time_seconds": start_sample / sample_rate,
                    "end_time_seconds": end_sample / sample_rate,
                    "chunk_duration_seconds": (end_sample - start_sample) / sample_rate,
                    "ai_processing_time": round(chunk_transcription_time, 2),
                    "word_count": len(chunk_text.split()) if chunk_text.strip() else 0
                })

                if chunk_text.strip():  # Only add non-empty transcriptions
                    transcribed_chunks.append(chunk_text)
                    segments.append({
                        "start": start_sample / sample_rate,
                        "end": end_sample / sample_rate,
                        "text": chunk_text
                    })

            except Exception as e:
                logger.warning(f"Failed to transcribe chunk {chunk_index + 1}: {e}")
                # Continue with next chunk
            finally:
                # Clean up chunk file
                if os.path.exists(chunk_path):
                    os.unlink(chunk_path)

            # FIXED: Move to next chunk properly - if we're at the end, stop
            if end_sample >= len(audio_data):
                logger.info(f"Reached end of audio at chunk {chunk_index + 1}")
                break

            # Move to next chunk with overlap
            next_start = end_sample - overlap_samples

            # Prevent infinite loop - if next start isn't moving forward, break
            if next_start <= start_sample:
                logger.info(f"Next chunk would not advance (next={next_start}, current={start_sample}), stopping")
                break

            start_sample = next_start
            chunk_index += 1

        # Combine all transcriptions
        full_text = " ".join(transcribed_chunks)

        total_chunk_time = time.time() - chunk_start_time
        total_transcription_time = sum(timing["ai_processing_time"] for timing in chunk_timings)

        # Return raw result that will be processed by the processor
        return {
            "text": full_text,
            "segments": segments,
            "language": "en",
            "language_probability": 1.0,
            "duration": total_duration,
            "backend": backend,
            "chunking_stats": {
                "total_chunks": len(chunk_timings),
                "successful_chunks": len(transcribed_chunks),
                "chunk_duration_seconds": chunk_duration,
                "overlap_seconds": overlap,
                "chunk_details": chunk_timings,
                "total_word_count": sum(timing["word_count"] for timing in chunk_timings),
                "avg_ai_time_per_chunk": round(total_transcription_time / len(chunk_timings), 2) if chunk_timings else 0,
                "fastest_chunk_ai_time": min(timing["ai_processing_time"] for timing in chunk_timings) if chunk_timings else 0,
                "slowest_chunk_ai_time": max(timing["ai_processing_time"] for timing in chunk_timings) if chunk_timings else 0,
            }
        }

    async def transcribe_cuda(self, audio_file: BinaryIO, filename: Optional[str] = None) -> Dict[str, Any]:
        return self._transcribe_common(audio_file, "cuda", filename=filename)

    async def transcribe_mps(self, audio_file: BinaryIO, filename: Optional[str] = None) -> Dict[str, Any]:
        return self._transcribe_common(audio_file, "mps", filename=filename)

    async def transcribe_vulkan(self, audio_file: BinaryIO, filename: Optional[str] = None) -> Dict[str, Any]:
        return self._transcribe_common(audio_file, "vulkan", filename=filename)

    async def transcribe_cpu(self, audio_file: BinaryIO, filename: Optional[str] = None) -> Dict[str, Any]:
        return self._transcribe_common(audio_file, "cpu", filename=filename)

    async def transcribe_directml(self, audio_file: BinaryIO, filename: Optional[str] = None) -> Dict[str, Any]:
        return self._transcribe_common(audio_file, "directml", filename=filename)

    async def transcribe(self, audio_file: BinaryIO, filename: Optional[str] = None) -> Dict[str, Any]:
        backend = self.current_backend or self.config.get("backend", "cpu")
        if backend == "cuda":
            return await self.transcribe_cuda(audio_file, filename=filename)
        if backend == "mps":
            return await self.transcribe_mps(audio_file, filename=filename)
        if backend == "directml":
            return await self.transcribe_directml(audio_file, filename=filename)
        return await self.transcribe_cpu(audio_file, filename=filename)

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
