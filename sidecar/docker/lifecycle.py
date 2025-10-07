"""
Container lifecycle manager for Docker-based model containers
"""

import logging
import asyncio
import time
import threading
from typing import Dict, Any, Optional, List
from dataclasses import dataclass, field
from enum import Enum
import docker
from docker.models.containers import Container

from .gpu_allocator import GPUAllocator
from ..config.docker_config import DockerConfig

logger = logging.getLogger(__name__)

class ContainerStatus(Enum):
    """Status of a container."""
    STOPPED = "stopped"
    STARTING = "starting"
    RUNNING = "running"
    STOPPING = "stopping"
    ERROR = "error"
    UNKNOWN = "unknown"

@dataclass
class ContainerInstance:
    """Represents a running container instance."""
    container_id: str
    docker_container: Container
    model_id: str
    status: ContainerStatus
    created_at: float
    started_at: Optional[float] = None
    last_activity: float = field(default_factory=time.time)
    health_check_count: int = 0
    error_count: int = 0
    gpu_memory_mb: int = 0

class ContainerLifecycleManager:
    """Manages lifecycle of model containers with inactivity timeout."""
    
    def __init__(self, config: DockerConfig, gpu_allocator: GPUAllocator):
        """
        Initialize container lifecycle manager.
        
        Args:
            config: Docker configuration
            gpu_allocator: GPU allocator instance
        """
        self.config = config
        self.gpu_allocator = gpu_allocator
        self.inactivity_timeout = config.get_inactivity_timeout()
        self.active_containers: Dict[str, ContainerInstance] = {}
        self.docker_client = None
        self.lock = asyncio.Lock()
        self.cleanup_task = None
        self._running = False
        
        # Initialize Docker client
        self._initialize_docker_client()
        
        logger.info(f"ContainerLifecycleManager initialized with inactivity timeout={self.inactivity_timeout}s")
    
    def _initialize_docker_client(self):
        """Initialize Docker client."""
        try:
            self.docker_client = docker.from_env()
            self.docker_client.ping()
            logger.info("Docker client initialized successfully")
        except Exception as e:
            logger.error(f"Failed to initialize Docker client: {e}")
            raise
    
    async def start(self):
        """Start the lifecycle manager and cleanup task."""
        if self._running:
            logger.warning("ContainerLifecycleManager is already running")
            return
        
        self._running = True
        self.cleanup_task = asyncio.create_task(self._cleanup_loop())
        logger.info("ContainerLifecycleManager started")
    
    async def stop(self):
        """Stop the lifecycle manager and cleanup task."""
        if not self._running:
            return
        
        self._running = False
        
        if self.cleanup_task:
            self.cleanup_task.cancel()
            try:
                await self.cleanup_task
            except asyncio.CancelledError:
                pass
        
        # Stop all active containers
        await self.cleanup_all_containers()
        
        logger.info("ContainerLifecycleManager stopped")
    
    async def start_container(self, model_id: str, container_info: Dict[str, Any]) -> Optional[ContainerInstance]:
        """
        Start a container and allocate GPU resources.
        
        Args:
            model_id: Model identifier
            container_info: Container configuration information
            
        Returns:
            Container instance or None if failed
        """
        async with self.lock:
            # Check if container is already running
            if model_id in self.active_containers:
                container = self.active_containers[model_id]
                if container.status == ContainerStatus.RUNNING:
                    logger.info(f"Container for model {model_id} is already running")
                    return container
                else:
                    logger.warning(f"Container for model {model_id} exists but has status {container.status.value}")
            
            # Get GPU memory requirement
            gpu_memory_mb = container_info.get("gpu_memory_mb", 0)
            
            # Allocate GPU memory
            if gpu_memory_mb > 0:
                if not self.gpu_allocator.allocate_gpu_memory(model_id, gpu_memory_mb):
                    logger.error(f"Failed to allocate GPU memory for model {model_id}")
                    return None
            
            try:
                # Prepare container configuration
                image = container_info.get("image")
                port = container_info.get("port")
                environment = container_info.get("environment", {})
                volumes = container_info.get("volumes", {})
                restart_policy = container_info.get("restart_policy", "unless-stopped")
                
                # Pull image if not available locally
                if not self._is_image_available_locally(image):
                    logger.info(f"Pulling Docker image {image}...")
                    await self._pull_image(image)
                
                # Create container
                logger.info(f"Starting container for model {model_id} with image {image}")
                
                container_config = {
                    "image": image,
                    "detach": True,
                    "ports": {f"{port}/tcp": port},
                    "environment": environment,
                    "volumes": volumes,
                    "restart_policy": {"Name": restart_policy}
                }
                
                # Add GPU configuration if GPU is available
                if gpu_memory_mb > 0 and self.config.is_gpu_available():
                    container_config["runtime"] = "nvidia"
                    container_config["environment"]["NVIDIA_VISIBLE_DEVICES"] = "all"
                    container_config["environment"]["NVIDIA_DRIVER_CAPABILITIES"] = "compute,utility"
                
                # Start container
                docker_container = self.docker_client.containers.run(**container_config)
                
                # Create container instance
                container_instance = ContainerInstance(
                    container_id=docker_container.id,
                    docker_container=docker_container,
                    model_id=model_id,
                    status=ContainerStatus.STARTING,
                    created_at=time.time(),
                    gpu_memory_mb=gpu_memory_mb
                )
                
                self.active_containers[model_id] = container_instance
                
                # Wait for container to be ready
                if await self._wait_for_container_ready(container_instance):
                    container_instance.status = ContainerStatus.RUNNING
                    container_instance.started_at = time.time()
                    logger.info(f"Container for model {model_id} started successfully")
                    return container_instance
                else:
                    # Failed to start, cleanup
                    await self.stop_container(model_id)
                    logger.error(f"Container for model {model_id} failed to become ready")
                    return None
                    
            except Exception as e:
                # Release GPU memory on failure
                if gpu_memory_mb > 0:
                    self.gpu_allocator.release_gpu_memory(model_id)
                
                logger.error(f"Failed to start container for model {model_id}: {e}")
                return None
    
    async def stop_container(self, model_id: str) -> bool:
        """
        Stop a container and release GPU resources.
        
        Args:
            model_id: Model identifier
            
        Returns:
            True if stop was successful
        """
        async with self.lock:
            if model_id not in self.active_containers:
                logger.warning(f"No active container found for model {model_id}")
                return False
            
            container = self.active_containers.pop(model_id)
            
            try:
                container.status = ContainerStatus.STOPPING
                
                # Stop Docker container
                if container.docker_container:
                    container.docker_container.stop()
                    container.docker_container.remove()
                    logger.info(f"Stopped Docker container {container.container_id}")
                
                # Release GPU memory
                if container.gpu_memory_mb > 0:
                    self.gpu_allocator.release_gpu_memory(model_id)
                
                container.status = ContainerStatus.STOPPED
                logger.info(f"Container for model {model_id} stopped successfully")
                return True
                
            except Exception as e:
                container.status = ContainerStatus.ERROR
                logger.error(f"Failed to stop container for model {model_id}: {e}")
                return False
    
    async def stop_inactive_containers(self) -> int:
        """
        Stop containers that have exceeded inactivity timeout.
        
        Returns:
            Number of containers stopped
        """
        current_time = time.time()
        inactive_containers = []
        
        async with self.lock:
            for model_id, container in self.active_containers.items():
                inactive_time = current_time - container.last_activity
                if inactive_time > self.inactivity_timeout:
                    inactive_containers.append(model_id)
        
        # Stop inactive containers
        stopped_count = 0
        for model_id in inactive_containers:
            if await self.stop_container(model_id):
                stopped_count += 1
                logger.info(f"Stopped inactive container for model {model_id}")
        
        return stopped_count
    
    async def update_container_activity(self, model_id: str) -> bool:
        """
        Update the last activity timestamp for a container.
        
        Args:
            model_id: Model identifier
            
        Returns:
            True if update was successful
        """
        async with self.lock:
            if model_id not in self.active_containers:
                return False
            
            self.active_containers[model_id].last_activity = time.time()
            
            # Also update GPU allocator activity
            self.gpu_allocator.update_container_activity(model_id)
            
            return True
    
    async def get_container_status(self, model_id: str) -> Optional[ContainerStatus]:
        """
        Get the current status of a container.
        
        Args:
            model_id: Model identifier
            
        Returns:
            Container status or None if not found
        """
        async with self.lock:
            container = self.active_containers.get(model_id)
            return container.status if container else None
    
    async def get_container_instance(self, model_id: str) -> Optional[ContainerInstance]:
        """
        Get the container instance for a model.
        
        Args:
            model_id: Model identifier
            
        Returns:
            Container instance or None if not found
        """
        async with self.lock:
            return self.active_containers.get(model_id)
    
    async def list_active_containers(self) -> List[str]:
        """
        List all active container model IDs.
        
        Returns:
            List of model IDs with active containers
        """
        async with self.lock:
            return list(self.active_containers.keys())
    
    async def cleanup_all_containers(self) -> int:
        """
        Stop all active containers.
        
        Returns:
            Number of containers cleaned up
        """
        model_ids = await self.list_active_containers()
        cleaned_count = 0
        
        for model_id in model_ids:
            if await self.stop_container(model_id):
                cleaned_count += 1
        
        logger.info(f"Cleaned up {cleaned_count} containers")
        return cleaned_count
    
    async def _cleanup_loop(self):
        """Background task to periodically check for inactive containers."""
        while self._running:
            try:
                await asyncio.sleep(60)  # Check every minute
                if self._running:
                    stopped_count = await self.stop_inactive_containers()
                    if stopped_count > 0:
                        logger.info(f"Stopped {stopped_count} inactive containers")
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in cleanup loop: {e}")
    
    def _is_image_available_locally(self, image: str) -> bool:
        """
        Check if a Docker image is available locally.
        
        Args:
            image: Image name
            
        Returns:
            True if image is available locally
        """
        try:
            self.docker_client.images.get(image)
            return True
        except docker.errors.ImageNotFound:
            return False
        except Exception as e:
            logger.warning(f"Error checking if image {image} is available locally: {e}")
            return False
    
    async def _pull_image(self, image: str) -> bool:
        """
        Pull a Docker image from registry.
        
        Args:
            image: Image name
            
        Returns:
            True if pull was successful
        """
        try:
            # Run in thread pool to avoid blocking
            loop = asyncio.get_event_loop()
            await loop.run_in_executor(None, self.docker_client.images.pull, image)
            logger.info(f"Successfully pulled image {image}")
            return True
        except Exception as e:
            logger.error(f"Failed to pull image {image}: {e}")
            return False
    
    async def _wait_for_container_ready(self, container: ContainerInstance, timeout: int = 60) -> bool:
        """
        Wait for a container to be ready.
        
        Args:
            container: Container instance
            timeout: Timeout in seconds
            
        Returns:
            True if container became ready
        """
        start_time = time.time()
        
        while time.time() - start_time < timeout:
            try:
                # Refresh container status
                container.docker_container.reload()
                status = container.docker_container.status
                
                if status == "running":
                    # Additional health check could be added here
                    return True
                elif status in ("exited", "dead", "removing"):
                    logger.error(f"Container {container.container_id} exited with status: {status}")
                    return False
                
                await asyncio.sleep(1)
            except Exception as e:
                logger.warning(f"Error checking container readiness: {e}")
                await asyncio.sleep(1)
        
        logger.error(f"Container {container.container_id} did not become ready within {timeout}s")
        return False
    
    async def get_container_logs(self, model_id: str, lines: int = 100) -> Optional[str]:
        """
        Get logs from a container.
        
        Args:
            model_id: Model identifier
            lines: Number of lines to retrieve
            
        Returns:
            Container logs or None if not found
        """
        async with self.lock:
            container = self.active_containers.get(model_id)
            if not container or not container.docker_container:
                return None
            
            try:
                logs = container.docker_container.logs(tail=lines).decode('utf-8')
                return logs
            except Exception as e:
                logger.error(f"Failed to get logs for container {container.container_id}: {e}")
                return None
    
    async def get_container_stats(self, model_id: str) -> Optional[Dict[str, Any]]:
        """
        Get resource usage statistics for a container.
        
        Args:
            model_id: Model identifier
            
        Returns:
            Container stats or None if not found
        """
        async with self.lock:
            container = self.active_containers.get(model_id)
            if not container or not container.docker_container:
                return None
            
            try:
                stats = container.docker_container.stats(stream=False)
                return stats
            except Exception as e:
                logger.error(f"Failed to get stats for container {container.container_id}: {e}")
                return None
    
    async def get_lifecycle_summary(self) -> Dict[str, Any]:
        """
        Get a summary of container lifecycle information.
        
        Returns:
            Lifecycle summary
        """
        async with self.lock:
            total_containers = len(self.active_containers)
            running_containers = sum(1 for c in self.active_containers.values() if c.status == ContainerStatus.RUNNING)
            total_gpu_memory = sum(c.gpu_memory_mb for c in self.active_containers.values())
            
            return {
                "total_containers": total_containers,
                "running_containers": running_containers,
                "total_gpu_memory_mb": total_gpu_memory,
                "inactivity_timeout": self.inactivity_timeout,
                "containers": {
                    model_id: {
                        "container_id": container.container_id,
                        "status": container.status.value,
                        "created_at": container.created_at,
                        "started_at": container.started_at,
                        "last_activity": container.last_activity,
                        "uptime": time.time() - container.started_at if container.started_at else 0,
                        "gpu_memory_mb": container.gpu_memory_mb,
                        "health_check_count": container.health_check_count,
                        "error_count": container.error_count
                    }
                    for model_id, container in self.active_containers.items()
                }
            }