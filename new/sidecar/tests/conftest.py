"""
Pytest configuration and fixtures for ASR Pro tests
"""

import pytest
import asyncio
import tempfile
import os
from pathlib import Path
from unittest.mock import Mock, patch
from unittest.mock import AsyncMock
import sys

# Add the sidecar directory to Python path
sidecar_dir = Path(__file__).parent.parent
sys.path.insert(0, str(sidecar_dir))

from config.settings import Settings
from models.manager import ModelManager
from models.registry import ModelRegistry
from utils.device import DeviceDetector

@pytest.fixture
def settings():
    """Create a test settings instance."""
    return Settings()

@pytest.fixture
def model_registry():
    """Create a test model registry."""
    return ModelRegistry()

@pytest.fixture
def device_detector():
    """Create a test device detector."""
    return DeviceDetector()

@pytest.fixture
def model_manager(settings):
    """Create a test model manager."""
    return ModelManager(settings)

@pytest.fixture
def sample_audio_file():
    """Create a sample audio file for testing."""
    # Create a minimal WAV file (44 bytes header + some data)
    wav_header = b'RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x01\x00\x40\x1f\x00\x00\x80\x3e\x00\x00\x02\x00\x10\x00data\x00\x00\x00\x00'
    wav_data = wav_header + b'\x00' * 1000  # Add some silence data
    
    with tempfile.NamedTemporaryFile(suffix='.wav', delete=False) as f:
        f.write(wav_data)
        f.flush()
        yield f.name
    
    # Cleanup
    if os.path.exists(f.name):
        os.unlink(f.name)

@pytest.fixture
def mock_whisper_model():
    """Create a mock Whisper model."""
    mock_model = Mock()
    mock_segment = Mock()
    mock_segment.start = 0.0
    mock_segment.end = 1.0
    mock_segment.text = "Hello world"
    
    mock_info = Mock()
    mock_info.language = "en"
    mock_info.language_probability = 0.95
    mock_info.duration = 1.0
    
    mock_model.transcribe.return_value = ([mock_segment], mock_info)
    return mock_model

@pytest.fixture
def mock_parakeet_model():
    """Create a mock Parakeet model."""
    mock_model = Mock()
    mock_model.transcribe.return_value = ["Hello world"]
    return mock_model

@pytest.fixture(scope="session")
def event_loop():
    """Create an instance of the default event loop for the test session."""
    loop = asyncio.get_event_loop_policy().new_event_loop()
    yield loop
    loop.close()
