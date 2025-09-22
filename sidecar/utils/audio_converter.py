"""
Audio conversion utilities for ASR Pro Python Sidecar
"""

import tempfile
import os
import logging
from typing import BinaryIO

logger = logging.getLogger(__name__)


def convert_to_wav(audio_file: BinaryIO, target_sample_rate: int = 16000) -> str:
    """
    Convert audio file to WAV format using librosa.

    Args:
        audio_file: BinaryIO object containing audio data
        target_sample_rate: Target sample rate for conversion

    Returns:
        Path to temporary WAV file
    """
    try:
        import librosa
        import soundfile as sf

        # Save original audio to temporary file
        with tempfile.NamedTemporaryFile(suffix=".tmp", delete=False) as temp_file:
            temp_file.write(audio_file.read())
            temp_path = temp_file.name

        try:
            # Load audio with librosa (handles MP3, WAV, etc.)
            audio_data, sample_rate = librosa.load(temp_path, sr=target_sample_rate)

            # Create WAV file
            wav_path = temp_path.replace(".tmp", ".wav")
            sf.write(wav_path, audio_data, target_sample_rate)

            return wav_path

        finally:
            # Clean up original temp file
            if os.path.exists(temp_path):
                os.unlink(temp_path)

    except Exception as e:
        logger.error(f"Failed to convert audio to WAV: {e}")
        raise
