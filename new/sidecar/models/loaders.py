"""
Concrete model loaders built on ONNXBaseLoader.
"""

from .base import ONNXBaseLoader


class WhisperLoader(ONNXBaseLoader):
    def _get_model_name(self):
        mapping = {
            "whisper-tiny": ["whisper-tiny_q4", "whisper-tiny_q8", "whisper-tiny"],
            "whisper-base": ["whisper-base_q4", "whisper-base_q8", "whisper-base"],
            "whisper-small": ["whisper-small_q4", "whisper-small_q8", "whisper-small"],
            "whisper-medium": [
                "whisper-medium_q4",
                "whisper-medium_q8",
                "whisper-medium",
            ],
            "whisper-large": ["whisper-large_q4", "whisper-large_q8", "whisper-large"],
        }
        return mapping.get(
            self.model_id, ["whisper-base_q4", "whisper-base_q8", "whisper-base"]
        )


class ParakeetTDTLoader(ONNXBaseLoader):
    def _get_model_name(self):
        return [
            "nemo-parakeet-tdt-0.6b-v2_q4",
            "nemo-parakeet-tdt-0.6b-v2_q8",
            "nemo-parakeet-tdt-0.6b-v2",
        ]
