"""
Whisper model loader for ASR Pro Python Sidecar
"""

import asyncio
from typing import Dict, Any, BinaryIO
import logging
from .base import BaseLoader

logger = logging.getLogger(__name__)

class WhisperLoader(BaseLoader):
    """Whisper model loader using faster-whisper."""
    
    def __init__(self, model_id: str, config: Dict[str, Any]):
        super().__init__(model_id, config)
        self.whisper_model = None
        self.model_size = self._extract_model_size()
    
    def _extract_model_size(self) -> str:
        """Extract model size from model_id."""
        if "tiny" in self.model_id:
            return "tiny"
        elif "base" in self.model_id:
            return "base"
        elif "small" in self.model_id:
            return "small"
        elif "medium" in self.model_id:
            return "medium"
        elif "large" in self.model_id:
            return "large"
        else:
            return "base"  # default
    
    async def load(self) -> bool:
        """Load the Whisper model."""
        try:
            from faster_whisper import WhisperModel
            
            # Get device configuration
            device = self.config.get("device", "cpu")
            compute_type = self.config.get("compute_type", "float16")
            
            logger.info(f"Loading Whisper model {self.model_id} on {device}")
            
            # Load model
            self.whisper_model = WhisperModel(
                self.model_size,
                device=device,
                compute_type=compute_type
            )
            
            self.is_loaded = True
            logger.info(f"Whisper model {self.model_id} loaded successfully")
            return True
            
        except Exception as e:
            logger.error(f"Failed to load Whisper model {self.model_id}: {e}")
            return False
    
    async def unload(self) -> bool:
        """Unload the Whisper model and free memory."""
        try:
            if self.whisper_model:
                del self.whisper_model
                self.whisper_model = None
            
            self.is_loaded = False
            logger.info(f"Whisper model {self.model_id} unloaded successfully")
            return True
            
        except Exception as e:
            logger.error(f"Failed to unload Whisper model {self.model_id}: {e}")
            return False
    
    async def transcribe(self, audio_file: BinaryIO) -> Dict[str, Any]:
        """Transcribe audio file using Whisper."""
        try:
            if not await self.ensure_loaded():
                raise Exception("Model not loaded")
            
            # Read audio file
            audio_data = audio_file.read()
            
            # Transcribe - faster-whisper expects file path or audio array
            # We need to save to temporary file or use a different approach
            import tempfile
            import os
            
            with tempfile.NamedTemporaryFile(suffix='.wav', delete=False) as temp_file:
                temp_file.write(audio_data)
                temp_file_path = temp_file.name
            
            try:
                # Transcribe
                segments, info = self.whisper_model.transcribe(
                    temp_file_path,
                    beam_size=5,
                    best_of=5,
                    patience=1,
                    length_penalty=1,
                    temperature=0.0
                )
            finally:
                # Clean up temporary file
                if os.path.exists(temp_file_path):
                    os.unlink(temp_file_path)
            
            # Collect segments
            segments_list = []
            full_text = ""
            
            for segment in segments:
                segment_data = {
                    "start": segment.start,
                    "end": segment.end,
                    "text": segment.text
                }
                segments_list.append(segment_data)
                full_text += segment.text + " "
            
            return {
                "text": full_text.strip(),
                "language": info.language,
                "language_probability": info.language_probability,
                "duration": info.duration,
                "segments": segments_list
            }
            
        except Exception as e:
            logger.error(f"Failed to transcribe with Whisper model {self.model_id}: {e}")
            raise
    
    def get_model_info(self) -> Dict[str, Any]:
        """Get Whisper model metadata."""
        return {
            "id": self.model_id,
            "name": f"Whisper {self.model_size.title()}",
            "description": f"OpenAI Whisper {self.model_size} model for speech recognition",
            "size": self.model_size,
            "type": "whisper",
            "loaded": self.is_loaded,
            "languages": ["en", "es", "fr", "de", "it", "pt", "ru", "ja", "ko", "zh"],
            "sample_rate": 16000
        }
