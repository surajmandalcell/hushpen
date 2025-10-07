"""
Registry for available model containers
"""

import logging
from typing import Dict, List, Optional, Any
from dataclasses import dataclass
from enum import Enum

logger = logging.getLogger(__name__)

class ContainerStatus(Enum):
    """Status of a model container."""
    UNKNOWN = "unknown"
    AVAILABLE = "available"
    PULLING = "pulling"
    READY = "ready"
    RUNNING = "running"
    STOPPED = "stopped"
    ERROR = "error"

@dataclass
class ModelContainerInfo:
    """Information about a model container."""
    id: str
    name: str
    description: str
    family: str
    size: str
    image: str
    port: int
    gpu_memory_mb: int
    languages: List[str]
    sample_rate: int
    environment: Dict[str, str]
    volumes: Dict[str, str]
    restart_policy: str
    status: ContainerStatus = ContainerStatus.UNKNOWN
    local_image: bool = False

class ContainerRegistry:
    """Registry for available model containers."""
    
    def __init__(self, config: Optional[Dict[str, Any]] = None):
        """
        Initialize container registry.
        
        Args:
            config: Docker configuration dictionary
        """
        self.config = config or {}
        self._containers = self._initialize_containers()
        logger.info(f"ContainerRegistry initialized with {len(self._containers)} models")
    
    def _initialize_containers(self) -> Dict[str, ModelContainerInfo]:
        """
        Initialize container metadata with Docker image information.
        
        Returns:
            Dictionary of container information by ID
        """
        # Get model configurations from config if available
        model_configs = self.config.get("model_containers", {})
        
        # Default container configurations
        default_containers = {
            "whisper-tiny": ModelContainerInfo(
                id="whisper-tiny",
                name="Whisper Tiny (Docker)",
                description="OpenAI Whisper tiny model in Docker container",
                family="whisper",
                size="tiny",
                image="asrpro/whisper-tiny:latest",
                port=8001,
                gpu_memory_mb=2048,
                languages=["en", "hi", "es", "fr", "de", "it", "pt", "ru", "ja", "zh"],
                sample_rate=16000,
                environment={
                    "MODEL_NAME": "whisper-tiny",
                    "DEVICE": "cuda",
                    "COMPUTE_TYPE": "float16"
                },
                volumes={
                    "/tmp/asrpro": "/tmp/asrpro"
                },
                restart_policy="unless-stopped"
            ),
            "whisper-base": ModelContainerInfo(
                id="whisper-base",
                name="Whisper Base (Docker)",
                description="OpenAI Whisper base model in Docker container",
                family="whisper",
                size="base",
                image="asrpro/whisper-base:latest",
                port=8002,
                gpu_memory_mb=4096,
                languages=["en", "hi", "es", "fr", "de", "it", "pt", "ru", "ja", "zh"],
                sample_rate=16000,
                environment={
                    "MODEL_NAME": "whisper-base",
                    "DEVICE": "cuda",
                    "COMPUTE_TYPE": "float16"
                },
                volumes={
                    "/tmp/asrpro": "/tmp/asrpro"
                },
                restart_policy="unless-stopped"
            ),
            "whisper-small": ModelContainerInfo(
                id="whisper-small",
                name="Whisper Small (Docker)",
                description="OpenAI Whisper small model in Docker container",
                family="whisper",
                size="small",
                image="asrpro/whisper-small:latest",
                port=8003,
                gpu_memory_mb=6144,
                languages=["en", "hi", "es", "fr", "de", "it", "pt", "ru", "ja", "zh"],
                sample_rate=16000,
                environment={
                    "MODEL_NAME": "whisper-small",
                    "DEVICE": "cuda",
                    "COMPUTE_TYPE": "float16"
                },
                volumes={
                    "/tmp/asrpro": "/tmp/asrpro"
                },
                restart_policy="unless-stopped"
            ),
            "whisper-medium": ModelContainerInfo(
                id="whisper-medium",
                name="Whisper Medium (Docker)",
                description="OpenAI Whisper medium model in Docker container",
                family="whisper",
                size="medium",
                image="asrpro/whisper-medium:latest",
                port=8004,
                gpu_memory_mb=10240,
                languages=["en", "hi", "es", "fr", "de", "it", "pt", "ru", "ja", "zh"],
                sample_rate=16000,
                environment={
                    "MODEL_NAME": "whisper-medium",
                    "DEVICE": "cuda",
                    "COMPUTE_TYPE": "float16"
                },
                volumes={
                    "/tmp/asrpro": "/tmp/asrpro"
                },
                restart_policy="unless-stopped"
            )
        }
        
        # Override with configurations from config if provided
        for container_id, config in model_configs.items():
            if container_id in default_containers:
                # Update existing container with config values
                container = default_containers[container_id]
                container.image = config.get("image", container.image)
                container.port = config.get("port", container.port)
                container.gpu_memory_mb = config.get("gpu_memory_mb", container.gpu_memory_mb)
                container.environment.update(config.get("environment", {}))
                container.volumes.update(config.get("volumes", {}))
                container.restart_policy = config.get("restart_policy", container.restart_policy)
            else:
                # Add new container from config
                default_containers[container_id] = ModelContainerInfo(
                    id=container_id,
                    name=config.get("name", f"Model {container_id}"),
                    description=config.get("description", f"Custom model {container_id}"),
                    family=config.get("family", "custom"),
                    size=config.get("size", "unknown"),
                    image=config.get("image", f"asrpro/{container_id}:latest"),
                    port=config.get("port", 8000),
                    gpu_memory_mb=config.get("gpu_memory_mb", 2048),
                    languages=config.get("languages", ["en"]),
                    sample_rate=config.get("sample_rate", 16000),
                    environment=config.get("environment", {}),
                    volumes=config.get("volumes", {}),
                    restart_policy=config.get("restart_policy", "unless-stopped")
                )
        
        return default_containers
    
    def list_containers(self) -> List[str]:
        """
        List all available container IDs.
        
        Returns:
            List of container IDs
        """
        return list(self._containers.keys())
    
    def get_container_info(self, container_id: str) -> Optional[ModelContainerInfo]:
        """
        Get container metadata by ID.
        
        Args:
            container_id: Container identifier
            
        Returns:
            Container information or None if not found
        """
        return self._containers.get(container_id)
    
    def get_container_port(self, container_id: str) -> Optional[int]:
        """
        Get the port mapping for a container.
        
        Args:
            container_id: Container identifier
            
        Returns:
            Port number or None if not found
        """
        container = self._containers.get(container_id)
        return container.port if container else None
    
    def is_container_available(self, container_id: str) -> bool:
        """
        Check if container is available in registry.
        
        Args:
            container_id: Container identifier
            
        Returns:
            True if container exists in registry
        """
        return container_id in self._containers
    
    def get_containers_by_family(self, family: str) -> List[ModelContainerInfo]:
        """
        Get all containers belonging to a specific family.
        
        Args:
            family: Model family (e.g., "whisper")
            
        Returns:
            List of container information
        """
        return [container for container in self._containers.values() if container.family == family]
    
    def get_containers_by_size(self, size: str) -> List[ModelContainerInfo]:
        """
        Get all containers of a specific size.
        
        Args:
            size: Model size (e.g., "tiny", "base", "small", "medium")
            
        Returns:
            List of container information
        """
        return [container for container in self._containers.values() if container.size == size]
    
    def get_containers_by_language(self, language: str) -> List[ModelContainerInfo]:
        """
        Get all containers that support a specific language.
        
        Args:
            language: Language code (e.g., "en", "hi")
            
        Returns:
            List of container information
        """
        return [
            container for container in self._containers.values() 
            if language in container.languages
        ]
    
    def get_container_image(self, container_id: str) -> Optional[str]:
        """
        Get the Docker image for a container.
        
        Args:
            container_id: Container identifier
            
        Returns:
            Docker image name or None if not found
        """
        container = self._containers.get(container_id)
        return container.image if container else None
    
    def get_container_environment(self, container_id: str) -> Dict[str, str]:
        """
        Get the environment variables for a container.
        
        Args:
            container_id: Container identifier
            
        Returns:
            Environment variables dictionary
        """
        container = self._containers.get(container_id)
        return container.environment if container else {}
    
    def get_container_volumes(self, container_id: str) -> Dict[str, str]:
        """
        Get the volume mappings for a container.
        
        Args:
            container_id: Container identifier
            
        Returns:
            Volume mappings dictionary
        """
        container = self._containers.get(container_id)
        return container.volumes if container else {}
    
    def get_container_restart_policy(self, container_id: str) -> str:
        """
        Get the restart policy for a container.
        
        Args:
            container_id: Container identifier
            
        Returns:
            Restart policy string
        """
        container = self._containers.get(container_id)
        return container.restart_policy if container else "unless-stopped"
    
    def update_container_status(self, container_id: str, status: ContainerStatus) -> bool:
        """
        Update the status of a container.
        
        Args:
            container_id: Container identifier
            status: New container status
            
        Returns:
            True if update was successful
        """
        container = self._containers.get(container_id)
        if container:
            container.status = status
            logger.debug(f"Updated container {container_id} status to {status.value}")
            return True
        return False
    
    def get_container_status(self, container_id: str) -> ContainerStatus:
        """
        Get the status of a container.
        
        Args:
            container_id: Container identifier
            
        Returns:
            Container status
        """
        container = self._containers.get(container_id)
        return container.status if container else ContainerStatus.UNKNOWN
    
    def mark_container_image_local(self, container_id: str, is_local: bool = True) -> bool:
        """
        Mark whether a container image is available locally.
        
        Args:
            container_id: Container identifier
            is_local: Whether the image is available locally
            
        Returns:
            True if update was successful
        """
        container = self._containers.get(container_id)
        if container:
            container.local_image = is_local
            logger.debug(f"Marked container {container_id} image as {'local' if is_local else 'remote'}")
            return True
        return False
    
    def is_container_image_local(self, container_id: str) -> bool:
        """
        Check if a container image is available locally.
        
        Args:
            container_id: Container identifier
            
        Returns:
            True if image is available locally
        """
        container = self._containers.get(container_id)
        return container.local_image if container else False
    
    def get_all_containers_info(self) -> Dict[str, ModelContainerInfo]:
        """
        Get information about all containers.
        
        Returns:
            Dictionary of all container information
        """
        return self._containers.copy()
    
    def get_container_summary(self) -> Dict[str, Any]:
        """
        Get a summary of all containers in the registry.
        
        Returns:
            Summary statistics
        """
        families = set(container.family for container in self._containers.values())
        sizes = set(container.size for container in self._containers.values())
        total_gpu_memory = sum(container.gpu_memory_mb for container in self._containers.values())
        
        return {
            "total_containers": len(self._containers),
            "families": list(families),
            "sizes": list(sizes),
            "total_gpu_memory_mb": total_gpu_memory,
            "local_images": sum(1 for container in self._containers.values() if container.local_image),
            "remote_images": sum(1 for container in self._containers.values() if not container.local_image)
        }
    
    def validate_container_config(self, container_id: str) -> Dict[str, Any]:
        """
        Validate the configuration of a container.
        
        Args:
            container_id: Container identifier
            
        Returns:
            Validation result
        """
        container = self._containers.get(container_id)
        if not container:
            return {
                "valid": False,
                "errors": [f"Container {container_id} not found in registry"]
            }
        
        errors = []
        warnings = []
        
        # Check required fields
        if not container.image:
            errors.append("Image name is required")
        
        if not container.port or container.port <= 0:
            errors.append("Port must be a positive integer")
        
        if container.gpu_memory_mb <= 0:
            errors.append("GPU memory must be positive")
        
        if not container.languages:
            warnings.append("No languages specified")
        
        if container.sample_rate <= 0:
            errors.append("Sample rate must be positive")
        
        # Check port conflicts
        ports_used = [c.port for c in self._containers.values() if c.id != container_id]
        if container.port in ports_used:
            errors.append(f"Port {container.port} is already used by another container")
        
        return {
            "valid": len(errors) == 0,
            "errors": errors,
            "warnings": warnings
        }