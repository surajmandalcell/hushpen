"""
Tests for Settings configuration
"""

import pytest
import json
import tempfile
import os
from pathlib import Path
from unittest.mock import patch, mock_open, AsyncMock

from config.settings import Settings

class TestSettings:
    """Test cases for Settings class."""
    
    def test_init(self, settings):
        """Test Settings initialization."""
        assert settings.config is not None
        assert "server" in settings.config
        assert "models" in settings.config
        assert "device" in settings.config
    
    def test_get_config_path_uses_contained_data_dir_env(self):
        """Test config path stays under the app-owned data directory."""
        with patch.dict('os.environ', {'ASRPRO_DATA_DIR': '/tmp/asrpro-contained'}, clear=False):
            with patch('pathlib.Path.mkdir'):
                settings = Settings()
                expected_path = Path('/tmp/asrpro-contained') / 'config' / 'config.json'
                assert settings.config_path == expected_path
    
    def test_get_default_config(self, settings):
        """Test default configuration."""
        config = settings._get_default_config()
        
        assert config["server"]["host"] == "127.0.0.1"
        assert config["server"]["port"] == 8000
        assert config["models"]["default_model"] == "whisper-base"
        assert config["models"]["cache_dir"].endswith("models")
        assert config["device"]["prefer_gpu"] is True
        assert config["device"]["compute_type"] == "auto"
    
    @pytest.mark.asyncio
    async def test_load_config_file_exists(self, settings):
        """Test loading configuration from existing file."""
        test_config = {
            "server": {"host": "0.0.0.0", "port": 9000},
            "models": {"default_model": "whisper-large"}
        }
        
        with patch.object(settings, 'config_path') as mock_path:
            mock_path.exists.return_value = True
            with patch('builtins.open', mock_open(read_data=json.dumps(test_config))):
                await settings.load_config()
                
                assert settings.config["server"]["host"] == "0.0.0.0"
                assert settings.config["server"]["port"] == 9000
                assert settings.config["models"]["default_model"] == "whisper-large"
    
    @pytest.mark.asyncio
    async def test_load_config_file_not_exists(self, settings):
        """Test loading configuration when file doesn't exist."""
        with patch.object(settings, 'config_path') as mock_path:
            mock_path.exists.return_value = False
            with patch.object(settings, 'save_config', new_callable=AsyncMock) as mock_save:
                await settings.load_config()
                mock_save.assert_called_once()
    
    @pytest.mark.asyncio
    async def test_save_config(self, settings):
        """Test saving configuration to file."""
        with patch.object(settings, 'config_path') as mock_path:
            with patch('builtins.open', mock_open()) as mock_file:
                await settings.save_config()
                mock_file.assert_called_once_with(mock_path, 'w', encoding='utf-8')
    
    def test_get_server_config(self, settings):
        """Test getting server configuration."""
        config = settings.get_server_config()
        assert config["host"] == "127.0.0.1"
        assert config["port"] == 8000
    
    def test_get_models_config(self, settings):
        """Test getting models configuration."""
        config = settings.get_models_config()
        assert config["default_model"] == "whisper-base"
    
    def test_get_device_config(self, settings):
        """Test getting device configuration."""
        config = settings.get_device_config()
        assert config["prefer_gpu"] is True
        assert config["compute_type"] == "auto"
    
    def test_get_config_dot_notation(self, settings):
        """Test getting configuration with dot notation."""
        value = settings.get_config("server.host")
        assert value == "127.0.0.1"
        
        value = settings.get_config("nonexistent.key", "default")
        assert value == "default"
    
    def test_set_config_dot_notation(self, settings):
        """Test setting configuration with dot notation."""
        settings.set_config("server.host", "0.0.0.0")
        assert settings.config["server"]["host"] == "0.0.0.0"
        
        settings.set_config("new.nested.key", "value")
        assert settings.config["new"]["nested"]["key"] == "value"
    
    @pytest.mark.asyncio
    async def test_update_config(self, settings):
        """Test updating multiple configuration values."""
        updates = {
            "server": {"host": "0.0.0.0"},
            "models": {"default_model": "whisper-large"}
        }
        
        with patch.object(settings, 'save_config', new_callable=AsyncMock) as mock_save:
            await settings.update_config(updates)
            
            assert settings.config["server"]["host"] == "0.0.0.0"
            assert settings.config["models"]["default_model"] == "whisper-large"
            mock_save.assert_called_once()
