"""
Audio conversion utilities for ASR Pro Python Sidecar
"""

import tempfile
import os
import logging
from typing import BinaryIO

logger = logging.getLogger(__name__)


def _extract_audio_data_from_corrupted_upload(file_data: bytes, original_size: int) -> bytes:
    """
    Try to extract valid audio data from corrupted web interface uploads.

    Args:
        file_data: The raw uploaded data (potentially corrupted)
        original_size: Original size of the data

    Returns:
        Cleaned audio data or original data if no corruption detected
    """
    try:
        # Audio file signatures to look for
        audio_signatures = {
            b'ID3': 'mp3',  # MP3 with ID3 tag
            b'\xff\xfb': 'mp3',  # MP3 frame header
            b'\xff\xf3': 'mp3',  # MP3 frame header
            b'\xff\xf2': 'mp3',  # MP3 frame header
            b'RIFF': 'wav',  # WAV file
            b'fLaC': 'flac',  # FLAC file
            b'\x00\x00\x00\x20ftypM4A': 'm4a',  # M4A file
        }

        # First, check if the data is already clean (starts with a valid audio signature)
        for signature, format_name in audio_signatures.items():
            if file_data.startswith(signature):
                logger.info(f"Data appears to be clean {format_name.upper()} file")
                return file_data

        # Look for multipart form data corruption patterns
        if b'Content-Disposition' in file_data or b'form-data' in file_data:
            logger.info("Detected multipart form data corruption, attempting to extract audio data")

            # Try to find audio data within the multipart mess
            for signature, format_name in audio_signatures.items():
                signature_pos = file_data.find(signature)
                if signature_pos > 0:  # Found signature not at the start
                    logger.info(f"Found {format_name.upper()} signature at position {signature_pos}, extracting audio data")

                    # Extract everything from the signature onwards
                    extracted_data = file_data[signature_pos:]

                    # For multipart data, we might have trailing boundaries, try to clean them
                    if format_name == 'mp3':
                        # Look for the end of valid MP3 data (common corruption: trailing form boundaries)
                        # MP3 files often end with ID3 tags or just audio frames
                        # Try to find obvious multipart boundaries after the audio
                        boundary_patterns = [b'--', b'Content-', b'form-data', b'\r\n\r\n--']

                        for pattern in boundary_patterns:
                            boundary_pos = extracted_data.find(pattern, len(extracted_data) // 2)  # Look in second half
                            if boundary_pos > 0:
                                logger.info(f"Found trailing boundary at position {boundary_pos}, truncating")
                                extracted_data = extracted_data[:boundary_pos]
                                break

                    logger.info(f"Extracted {len(extracted_data)} bytes of {format_name.upper()} data (from {original_size} bytes)")
                    return extracted_data

        # Look for binary patterns that suggest embedded audio data
        # Sometimes form data might not have obvious headers but still contain embedded audio
        for signature, format_name in audio_signatures.items():
            signature_pos = file_data.find(signature)
            if signature_pos >= 0:
                logger.info(f"Found {format_name.upper()} signature at position {signature_pos}")
                return file_data[signature_pos:]

        # If we can't find any audio signatures, check if this might be base64 encoded
        if original_size > 1000:
            # Look for base64 patterns (lots of alphanumeric chars with occasional = padding)
            try:
                import base64
                # Try to decode as base64 - this might work for some web interfaces
                decoded_data = base64.b64decode(file_data, validate=True)
                if len(decoded_data) > 100:
                    # Check if decoded data has audio signatures
                    for signature, format_name in audio_signatures.items():
                        if decoded_data.startswith(signature):
                            logger.info(f"Successfully decoded base64 data to {format_name.upper()}")
                            return decoded_data
            except Exception:
                pass  # Base64 decoding failed, continue

        logger.warning("Could not extract audio data from corrupted upload, using original data")
        return file_data

    except Exception as e:
        logger.warning(f"Error during audio data extraction: {e}, using original data")
        return file_data


def convert_to_wav(audio_file: BinaryIO, target_sample_rate: int = 16000, original_filename: str = None) -> str:
    """
    Convert audio/video file to WAV format using FFmpeg and librosa.

    Supports all FFmpeg-compatible formats including:
    - Audio: MP3, WAV, FLAC, M4A, OGG, AAC, WMA, AIFF, etc.
    - Video: MP4, AVI, MKV, MOV, FLV, WebM, 3GP, etc.
    - YouTube/streaming formats: Any format FFmpeg can handle

    Args:
        audio_file: BinaryIO object containing audio/video data
        target_sample_rate: Target sample rate for conversion
        original_filename: Original filename to detect format

    Returns:
        Path to temporary WAV file
    """
    try:
        import librosa
        import soundfile as sf
        from pydub import AudioSegment

        # Read the file data first to check for corruption
        file_data = audio_file.read()
        file_size = len(file_data)

        # Try to recover audio data from corrupted web interface uploads
        if original_filename == "filename" and file_size > 0:
            file_data = _extract_audio_data_from_corrupted_upload(file_data, file_size)

        # Check for minimum file size after potential recovery
        if len(file_data) < 100:
            raise ValueError(
                f"File is too small ({len(file_data)} bytes) to be a valid audio file. "
                "Could not extract valid audio data from upload."
            )

        # Determine file extension from original filename or try to detect format
        supported_audio_exts = ['.mp3', '.wav', '.m4a', '.flac', '.ogg', '.aac', '.wma', '.aiff', '.opus']
        supported_video_exts = ['.mp4', '.avi', '.mkv', '.mov', '.flv', '.webm', '.3gp', '.wmv', '.mpg', '.mpeg', '.m4v', '.ts', '.f4v']

        if original_filename:
            _, ext = os.path.splitext(original_filename.lower())
            if ext in supported_audio_exts or ext in supported_video_exts:
                suffix = ext
            else:
                # For unknown extensions, try to detect based on content or default to .mp4 for videos
                if any(sig in file_data[:50] for sig in [b'ftyp', b'RIFF', b'ID3', b'\xff\xfb']):
                    suffix = '.mp4' if b'ftyp' in file_data[:50] else '.mp3'
                else:
                    suffix = '.mp4'  # Default for unknown formats (likely video)
        else:
            suffix = '.mp4'  # Default for generic filenames

        # Save original audio to temporary file with correct extension
        with tempfile.NamedTemporaryFile(suffix=suffix, delete=False) as temp_file:
            temp_file.write(file_data)
            temp_path = temp_file.name

        try:
            logger.info(f"Converting audio/video file: {temp_path} (original: {original_filename}, size: {file_size} bytes)")

            # Check if this is a video file or format that needs FFmpeg
            video_extensions = ['.mp4', '.avi', '.mkv', '.mov', '.flv', '.webm', '.3gp', '.wmv', '.mpg', '.mpeg', '.m4v', '.ts', '.f4v']
            use_ffmpeg = suffix in video_extensions or suffix not in ['.mp3', '.wav', '.flac']

            if use_ffmpeg:
                logger.info(f"Using FFmpeg for {suffix} conversion (video or complex format)")
                # Use FFmpeg to extract audio and convert to WAV
                ffmpeg_wav_path = temp_path.replace(suffix, "_ffmpeg.wav")

                import subprocess
                # FFmpeg command to extract audio and convert to WAV
                cmd = [
                    'ffmpeg', '-i', temp_path,
                    '-acodec', 'pcm_s16le',  # 16-bit PCM
                    '-ar', str(target_sample_rate),  # Sample rate
                    '-ac', '1',  # Mono
                    '-y',  # Overwrite output
                    ffmpeg_wav_path
                ]

                try:
                    result = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
                    if result.returncode != 0:
                        raise RuntimeError(f"FFmpeg failed: {result.stderr}")

                    # Load the FFmpeg-converted WAV with librosa for final processing
                    audio_data, sample_rate = librosa.load(ffmpeg_wav_path, sr=target_sample_rate)

                    # Clean up FFmpeg temp file
                    if os.path.exists(ffmpeg_wav_path):
                        os.unlink(ffmpeg_wav_path)

                except subprocess.TimeoutExpired:
                    raise RuntimeError("FFmpeg conversion timed out (>60s)")
                except FileNotFoundError:
                    raise RuntimeError("FFmpeg not found. Please install FFmpeg to process video files.")

            elif suffix == '.mp3':
                logger.info("Using pydub for MP3 conversion")
                # Load MP3 with pydub (more reliable for MP3 on Linux)
                audio = AudioSegment.from_mp3(temp_path)

                # Convert to WAV temporarily for librosa
                wav_temp_path = temp_path.replace('.mp3', '_pydub.wav')
                audio.export(wav_temp_path, format="wav")

                # Now load with librosa for resampling
                audio_data, sample_rate = librosa.load(wav_temp_path, sr=target_sample_rate)

                # Clean up temporary WAV
                if os.path.exists(wav_temp_path):
                    os.unlink(wav_temp_path)
            else:
                logger.info(f"Using librosa for {suffix} conversion")
                # Use librosa directly for other audio formats
                audio_data, sample_rate = librosa.load(temp_path, sr=target_sample_rate)

            logger.info(f"Audio loaded successfully: {len(audio_data)} samples at {sample_rate}Hz")

            # Create final WAV file
            wav_path = temp_path.replace(suffix, ".wav")
            sf.write(wav_path, audio_data, target_sample_rate)

            logger.info(f"WAV file created: {wav_path}")

            return wav_path

        finally:
            # Clean up original temp file
            if os.path.exists(temp_path):
                os.unlink(temp_path)

    except ValueError as ve:
        # Re-raise ValueError with helpful message
        logger.error(f"File validation error: {ve}")
        raise
    except Exception as e:
        logger.error(f"Failed to convert audio to WAV: {e}")
        import traceback
        logger.error(f"Full traceback: {traceback.format_exc()}")
        raise
