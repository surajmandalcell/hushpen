"""
Docker configuration settings for ASR Pro
"""

import os
from typing import Dict, Any, Optional
import logging

logger = logging.getLogger(__name__)

class DockerConfig:
    """Configuration settings for Docker integration."""
    
    def __init__(self, config: Optional[Dict[str, Any]] = None):
        """Initialize Docker configuration with defaults or provided config."""
        self.config = config or self._get_default_config()
    
    def _get_default_config(self) -> Dict[str, Any]:
        """Get default Docker configuration."""
        return {
            "host": "unix:///var/run/docker.sock",
            "registry_url": "https://registry.hub.docker.com",
            "container_network": "asrpro-network",
            "gpu_driver": "nvidia",
            "container_timeout": 30,  # Container startup timeout in seconds
            "health_check_interval": 30,  # Health check interval in seconds
            "max_concurrent_containers": 3,  # Max containers running simultaneously
            "inactivity_timeout": 300,  # 5 minutes default
            "pull_policy": "if_missing",  # always|never|if_missing
            "model_containers": {
                "whisper-tiny": {
                    "image": "asrpro/whisper-tiny:latest",
                    "port": 8001,
                    "gpu_memory_mb": 2048,
                    "environment": {
                        "MODEL_NAME": "whisper-tiny",
                        "DEVICE": "cuda",
                        "COMPUTE_TYPE": "float16"
                    },
                    "volumes": {
                        "/tmp/asrpro": "/tmp/asrpro"
                    },
                    "restart_policy": "unless-stopped"
                },
                "whisper-base": {
                    "image": "asrpro/whisper-base:latest",
                    "port": 8002,
                    "gpu_memory_mb": 4096,
                    "environment": {
                        "MODEL_NAME": "whisper-base",
                        "DEVICE": "cuda",
                        "COMPUTE_TYPE": "float16"
                    },
                    "volumes": {
                        "/tmp/asrpro": "/tmp/asrpro"
                    },
                    "restart_policy": "unless-stopped"
                },
                "whisper-small": {
                    "image": "asrpro/whisper-small:latest",
                    "port": 8003,
                    "gpu_memory_mb": 6144,
                    "environment": {
                        "MODEL_NAME": "whisper-small",
                        "DEVICE": "cuda",
                        "COMPUTE_TYPE": "float16"
                    },
                    "volumes": {
                        "/tmp/asrpro": "/tmp/asrpro"
                    },
                    "restart_policy": "unless-stopped"
                },
                "whisper-medium": {
                    "image": "asrpro/whisper-medium:latest",
                    "port": 8004,
                    "gpu_memory_mb": 10240,
                    "environment": {
                        "MODEL_NAME": "whisper-medium",
                        "DEVICE": "cuda",
                        "COMPUTE_TYPE": "float16"
                    },
                    "volumes": {
                        "/tmp/asrpro": "/tmp/asrpro"
                    },
                    "restart_policy": "unless-stopped"
                }
            }
        }
    
    def get_docker_host(self) -> str:
        """Get Docker host configuration."""
        # Check for environment variable override
        docker_host = os.environ.get("DOCKER_HOST")
        if docker_host:
            return docker_host
        return self.config.get("host", "unix:///var/run/docker.sock")
    
    def get_registry_url(self) -> str:
        """Get Docker registry URL."""
        return self.config.get("registry_url", "https://registry.hub.docker.com")
    
    def get_container_network(self) -> str:
        """Get Docker network name for containers."""
        return self.config.get("container_network", "asrpro-network")
    
    def get_gpu_driver(self) -> str:
        """Get GPU driver configuration."""
        return self.config.get("gpu_driver", "nvidia")
    
    def get_container_timeout(self) -> int:
        """Get container startup timeout in seconds."""
        return self.config.get("container_timeout", 30)
    
    def get_health_check_interval(self) -> int:
        """Get health check interval in seconds."""
        return self.config.get("health_check_interval", 30)
    
    def get_max_concurrent_containers(self) -> int:
        """Get maximum number of concurrent containers."""
        return self.config.get("max_concurrent_containers", 3)
    
    def get_inactivity_timeout(self) -> int:
        """Get inactivity timeout in seconds."""
        return self.config.get("inactivity_timeout", 300)
    
    def get_pull_policy(self) -> str:
        """Get image pull policy."""
        return self.config.get("pull_policy", "if_missing")
    
    def get_model_container_config(self, model_id: str) -> Optional[Dict[str, Any]]:
        """Get configuration for a specific model container."""
        model_containers = self.config.get("model_containers", {})
        return model_containers.get(model_id)
    
    def get_all_model_configs(self) -> Dict[str, Dict[str, Any]]:
        """Get all model container configurations."""
        return self.config.get("model_containers", {})
    
    def is_docker_available(self) -> bool:
        """Check if Docker is available on the system."""
        try:
            import docker
            client = docker.from_env()
            client.ping()
            return True
        except Exception as e:
            logger.warning(f"Docker not available: {e}")
            return False
    
    def is_gpu_available(self) -> bool:
        """Check if GPU is available for Docker containers."""
        try:
            import docker
            client = docker.from_env()
            # Check if nvidia-docker is available
            result = client.containers.run(
                "nvidia/cuda:12.0.0-base-ubuntu22.04",
                "nvidia-smi",
                remove=True,
                runtime="nvidia"
            )
            return True
        except Exception as e:
            logger.warning(f"NVIDIA Docker runtime not available: {e}")
            return False
    
    def get_gpu_memory_total(self) -> int:
        """Get total GPU memory in MB for RTX 4070 Super."""
        # RTX 4070 Super has 16GB VRAM
        return 16384
    
    def validate_config(self) -> bool:
        """Validate Docker configuration."""
        # Check if Docker is available
        if not self.is_docker_available():
            logger.error("Docker is not available on this system")
            return False
        
        # Validate model configurations
        model_configs = self.get_all_model_configs()
        if not model_configs:
            logger.error("No model container configurations found")
            return False
        
        # Check for required fields in each model config
        for model_id, config in model_configs.items():
            required_fields = ["image", "port", "gpu_memory_mb"]
            for field in required_fields:
                if field not in config:
                    logger.error(f"Missing required field '{field}' in model '{model_id}' configuration")
                    return False
        
        return True