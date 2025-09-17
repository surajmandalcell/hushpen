"""Pytest configuration and fixtures."""

import sys
import pytest
from pathlib import Path

# Add project root to Python path
project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))


@pytest.fixture
def temp_config_dir(tmp_path):
    """Create a temporary config directory for testing."""
    config_dir = tmp_path / "test_config"
    config_dir.mkdir()
    return config_dir


@pytest.fixture
def mock_audio_file(tmp_path):
    """Create a mock audio file for testing."""
    audio_file = tmp_path / "test_audio.wav"
    # Create a minimal WAV file header
    audio_file.write_bytes(b'RIFF' + b'\x00' * 40)  # Minimal WAV header
    return audio_file
