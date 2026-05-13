"""
Configuration settings for ASR Pro Python Sidecar
"""

import json
import os
from pathlib import Path
from typing import Dict, Any, Optional
import logging

logger = logging.getLogger(__name__)

class Settings:
    """Configuration settings for the sidecar."""
    
    def __init__(self):
        self.data_dir = self._get_data_dir()
        self.config_path = self._get_config_path()
        self.config = self._get_default_config()
        self._ensure_config_directory()
        self._configure_model_cache_environment()

    def _get_data_dir(self) -> Path:
        """Get the app-owned data directory instead of platform user config folders."""
        env_dir = os.environ.get("ASRPRO_DATA_DIR")
        if env_dir:
            return Path(env_dir)

        return Path(__file__).resolve().parents[2] / "data"
    
    def _get_config_path(self) -> Path:
        """Get app-contained configuration path."""
        return self.data_dir / "config" / "config.json"
    
    def _ensure_config_directory(self):
        """Ensure app-owned data directories exist."""
        self.data_dir.mkdir(parents=True, exist_ok=True)
        self.config_path.parent.mkdir(parents=True, exist_ok=True)
        (self.data_dir / "models").mkdir(parents=True, exist_ok=True)
        (self.data_dir / "logs").mkdir(parents=True, exist_ok=True)
        (self.data_dir / "transcripts").mkdir(parents=True, exist_ok=True)

    def _configure_model_cache_environment(self):
        """Force model frameworks to keep downloads under the contained data dir."""
        models_dir = self.data_dir / "models"
        os.environ["HF_HOME"] = str(models_dir / "huggingface")
        os.environ["HUGGINGFACE_HUB_CACHE"] = str(models_dir / "huggingface" / "hub")
        os.environ["NEMO_HOME"] = str(models_dir / "nemo")
        os.environ["TORCH_HOME"] = str(models_dir / "torch")
        os.environ["XDG_CACHE_HOME"] = str(self.data_dir / "cache")
    
    def _get_default_config(self) -> Dict[str, Any]:
        """Get default configuration."""
        return {
            "server": {
                "host": "127.0.0.1",
                "port": 8000
            },
            "models": {
                "default_model": "whisper-base",
                "cache_dir": str(self.data_dir / "models")
            },
            "device": {
                "prefer_gpu": True,
                "compute_type": "auto"
            }
        }
    
    async def load_config(self):
        """Load configuration from file."""
        try:
            if self.config_path.exists():
                with open(self.config_path, 'r', encoding='utf-8') as f:
                    loaded_config = json.load(f)
                
                # Merge with default config
                self._merge_config(self.config, loaded_config)
                logger.info(f"Configuration loaded from {self.config_path}")
            else:
                logger.info("No existing configuration found, using defaults")
                await self.save_config()
                
        except Exception as e:
            logger.error(f"Failed to load configuration: {e}")
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
    
    def get_models_config(self) -> Dict[str, Any]:
        """Get models configuration."""
        return self.config.get("models", {})
    
    def get_device_config(self) -> Dict[str, Any]:
        """Get device configuration."""
        return self.config.get("device", {})
    
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
