"""
Configuration Manager for ASR Pro Python Sidecar
"""

import json
import os
import platform
from pathlib import Path
from typing import Dict, Any, Optional
import logging

logger = logging.getLogger(__name__)

class ConfigManager:
    """Manages configuration loading, saving, and validation."""
    
    def __init__(self):
        self.config_path = self._get_config_path()
        self.config = self._get_default_config()
        self._ensure_config_directory()
    
    def _get_config_path(self) -> Path:
        """Get platform-specific configuration path."""
        system = platform.system()
        
        if system == "Darwin":  # macOS
            base_path = Path.home() / "Library" / "Application Support" / "asrpro"
        elif system == "Windows":
            base_path = Path(os.environ.get("APPDATA", "")) / "asrpro"
        else:  # Linux and others
            base_path = Path.home() / ".config" / "asrpro"
        
        return base_path / "config.json"
    
    def _ensure_config_directory(self):
        """Ensure configuration directory exists."""
        self.config_path.parent.mkdir(parents=True, exist_ok=True)
    
    def _get_default_config(self) -> Dict[str, Any]:
        """Get default configuration."""
        return {
            "server": {
                "enabled": True,
                "host": "127.0.0.1",
                "port": 8000,
                "auto_start": True
            },
            "device": {
                "prefer_gpu": True,
                "cuda_enabled": False,
                "mps_enabled": False,
                "vulkan_enabled": False
            },
            "ui": {
                "theme": "dark",
                "opacity": 1.0,
                "animations": True
            },
            "audio": {
                "sample_rate": 16000,
                "channels": 1,
                "format": "wav",
                "quality": "high"
            },
            "model": {
                "auto_unload": True,
                "auto_unload_timeout": 1800,  # 30 minutes
                "prefer_gpu": True,
                "cache_directory": ""
            },
            "hotkey": {
                "combination": "",
                "overlay": True,
                "auto_paste": True
            }
        }
    
    async def load_config(self):
        """Load configuration from file."""
        try:
            if self.config_path.exists():
                with open(self.config_path, 'r', encoding='utf-8') as f:
                    loaded_config = json.load(f)
                
                # Merge with default config to ensure all keys exist
                self._merge_config(self.config, loaded_config)
                logger.info(f"Configuration loaded from {self.config_path}")
            else:
                logger.info("No existing configuration found, using defaults")
                await self.save_config()
                
        except Exception as e:
            logger.error(f"Failed to load configuration: {e}")
            logger.info("Using default configuration")
            self.config = self._get_default_config()
    
    async def save_config(self):
        """Save configuration to file."""
        try:
            with open(self.config_path, 'w', encoding='utf-8') as f:
                json.dump(self.config, f, indent=2, ensure_ascii=False)
            logger.info(f"Configuration saved to {self.config_path}")
        except Exception as e:
            logger.error(f"Failed to save configuration: {e}")
    
    def _merge_config(self, base: Dict[str, Any], update: Dict[str, Any]):
        """Recursively merge configuration dictionaries."""
        for key, value in update.items():
            if key in base and isinstance(base[key], dict) and isinstance(value, dict):
                self._merge_config(base[key], value)
            else:
                base[key] = value
    
    def get_server_config(self) -> Dict[str, Any]:
        """Get server configuration."""
        return self.config.get("server", {})
    
    def get_device_config(self) -> Dict[str, Any]:
        """Get device configuration."""
        return self.config.get("device", {})
    
    def get_ui_config(self) -> Dict[str, Any]:
        """Get UI configuration."""
        return self.config.get("ui", {})
    
    def get_audio_config(self) -> Dict[str, Any]:
        """Get audio configuration."""
        return self.config.get("audio", {})
    
    def get_model_config(self) -> Dict[str, Any]:
        """Get model configuration."""
        return self.config.get("model", {})
    
    def get_hotkey_config(self) -> Dict[str, Any]:
        """Get hotkey configuration."""
        return self.config.get("hotkey", {})
    
    def get_config(self, key: str, default: Any = None) -> Any:
        """Get configuration value by dot notation key."""
        keys = key.split('.')
        value = self.config
        
        for k in keys:
            if isinstance(value, dict) and k in value:
                value = value[k]
            else:
                return default
        
        return value
    
    def set_config(self, key: str, value: Any):
        """Set configuration value by dot notation key."""
        keys = key.split('.')
        config = self.config
        
        # Navigate to the parent of the target key
        for k in keys[:-1]:
            if k not in config:
                config[k] = {}
            config = config[k]
        
        # Set the value
        config[keys[-1]] = value
    
    async def update_config(self, updates: Dict[str, Any]):
        """Update multiple configuration values."""
        self._merge_config(self.config, updates)
        await self.save_config()
    
    def get_cache_directory(self) -> Path:
        """Get model cache directory."""
        cache_dir = self.get_config("model.cache_directory", "")
        if cache_dir:
            return Path(cache_dir)
        else:
            # Default cache directory
            system = platform.system()
            if system == "Darwin":
                return Path.home() / "Library" / "Caches" / "asrpro"
            elif system == "Windows":
                return Path(os.environ.get("LOCALAPPDATA", "")) / "asrpro" / "cache"
            else:
                return Path.home() / ".cache" / "asrpro"
