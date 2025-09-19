"""
Parakeet model loader for ASR Pro Python Sidecar
"""

import asyncio
from typing import Dict, Any, BinaryIO
import logging
from .base import BaseLoader

logger = logging.getLogger(__name__)

class ParakeetLoader(BaseLoader):
    """Parakeet model loader using NeMo framework."""
    
    def __init__(self, model_id: str, config: Dict[str, Any]):
        super().__init__(model_id, config)
        self.parakeet_model = None
        self.model_variant = self._extract_model_variant()
    
    def _extract_model_variant(self) -> str:
        """Extract model variant from model_id."""
        if "ctc" in self.model_id:
            return "ctc"
        elif "rnnt" in self.model_id:
            return "rnnt"
        elif "transducer" in self.model_id:
            return "transducer"
        else:
            return "ctc"  # default
    
    async def load(self) -> bool:
        """Load the Parakeet model."""
        try:
            import nemo.collections.asr as nemo_asr
            
            logger.info(f"Loading Parakeet model {self.model_id}")
            
            # Load pre-trained model based on variant
            if self.model_variant == "ctc":
                self.parakeet_model = nemo_asr.models.ASRModel.from_pretrained(
                    "nvidia/stt_en_parakeet_ctc_1.1b"
                )
            elif self.model_variant == "rnnt":
                self.parakeet_model = nemo_asr.models.ASRModel.from_pretrained(
                    "nvidia/stt_en_parakeet_rnnt_1.1b"
                )
            else:
                self.parakeet_model = nemo_asr.models.ASRModel.from_pretrained(
                    "nvidia/stt_en_parakeet_transducer_1.1b"
                )
            
            # Move to appropriate device if available
            device = self.config.get("device", "cpu")
            if device == "cuda":
                self.parakeet_model = self.parakeet_model.cuda()
            
            self.is_loaded = True
            logger.info(f"Parakeet model {self.model_id} loaded successfully")
            return True
            
        except Exception as e:
            logger.error(f"Failed to load Parakeet model {self.model_id}: {e}")
            return False
    
    async def unload(self) -> bool:
        """Unload the Parakeet model and free memory."""
        try:
            if self.parakeet_model:
                del self.parakeet_model
                self.parakeet_model = None
            
            self.is_loaded = False
            logger.info(f"Parakeet model {self.model_id} unloaded successfully")
            return True
            
        except Exception as e:
            logger.error(f"Failed to unload Parakeet model {self.model_id}: {e}")
            return False
    
    async def transcribe(self, audio_file: BinaryIO) -> Dict[str, Any]:
        """Transcribe audio file using Parakeet."""
        try:
            if not await self.ensure_loaded():
                raise Exception("Model not loaded")
            
            # Read audio file
            audio_data = audio_file.read()
            
            # Transcribe
            transcription = self.parakeet_model.transcribe(
                [audio_data],
                batch_size=1,
                return_hypotheses=False
            )
            
            # Parakeet typically returns just text without segments
            text = transcription[0] if transcription else ""
            
            return {
                "text": text,
                "segments": [
                    {
                        "start": 0,
                        "end": 0,  # Parakeet doesn't provide timing by default
                        "text": text
                    }
                ]
            }
            
        except Exception as e:
            logger.error(f"Failed to transcribe with Parakeet model {self.model_id}: {e}")
            raise
    
    def get_model_info(self) -> Dict[str, Any]:
        """Get Parakeet model metadata."""
        return {
            "id": self.model_id,
            "name": f"Parakeet {self.model_variant.upper()}",
            "description": f"NVIDIA Parakeet {self.model_variant.upper()} model for speech recognition",
            "variant": self.model_variant,
            "type": "parakeet",
            "loaded": self.is_loaded,
            "languages": ["en"],
            "sample_rate": 16000,
            "framework": "nemo"
        }
