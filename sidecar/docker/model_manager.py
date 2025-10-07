"""
Docker-based model manager for ASR processing
"""

import logging
import asyncio
import time
import os
import tempfile
from typing import Dict, Any, Optional, List, Callable
from pathlib import Path

from .registry import ContainerRegistry
from .lifecycle import ContainerLifecycleManager
from .gpu_allocator import GPUAllocator
from .communication import ContainerCommunicationAdapter
from ..config.docker_config import DockerConfig

logger = logging.getLogger(__name__)

class DockerModelManager:
    """Manages Docker-based model containers for ASR processing."""
    
    def __init__(self, config: Optional[Dict[str, Any]] = None):
        """
        Initialize Docker model manager.
        
        Args:
            config: Configuration dictionary
        """
        # Initialize configuration
        self.docker_config = DockerConfig(config or {})
        
        # Initialize components
        self.container_registry = ContainerRegistry(self.docker_config.config)
        self.gpu_allocator = GPUAllocator(self.docker_config.get_gpu_memory_total())
        self.lifecycle_manager = ContainerLifecycleManager(self.docker_config, self.gpu_allocator)
        self.communication_adapter = ContainerCommunicationAdapter(
            timeout=self.docker_config.get_container_timeout(),
            max_retries=3
        )
        
        # State
        self.current_model = None
        self.is_initialized = False
        self._lock = asyncio.Lock()
        
        logger.info("DockerModelManager initialized")
    
    async def initialize(self) -> bool:
        """
        Initialize Docker environment and detect GPU capabilities.
        
        Returns:
            True if initialization was successful
        """
        async with self._lock:
            if self.is_initialized:
                logger.warning("DockerModelManager is already initialized")
                return True
            
            try:
                # Validate Docker configuration
                if not self.docker_config.validate_config():
                    logger.error("Docker configuration validation failed")
                    return False
                
                # Start lifecycle manager
                await self.lifecycle_manager.start()
                
                # Check GPU availability
                gpu_available = self.docker_config.is_gpu_available()
                if gpu_available:
                    logger.info("GPU is available for Docker containers")
                else:
                    logger.warning("GPU is not available, containers will run in CPU mode")
                
                self.is_initialized = True
                logger.info("DockerModelManager initialization completed successfully")
                return True
                
            except Exception as e:
                logger.error(f"Failed to initialize DockerModelManager: {e}")
                return False
    
    async def list_available_models(self) -> List[str]:
        """
        List all available model containers.
        
        Returns:
            List of model IDs
        """
        if not self.is_initialized:
            logger.warning("DockerModelManager not initialized")
            return []
        
        return self.container_registry.list_containers()
    
    async def get_model_info(self, model_id: str) -> Optional[Dict[str, Any]]:
        """
        Get information about a model container.
        
        Args:
            model_id: Model identifier
            
        Returns:
            Model information or None if not found
        """
        if not self.is_initialized:
            logger.warning("DockerModelManager not initialized")
            return None
        
        container_info = self.container_registry.get_container_info(model_id)
        if not container_info:
            return None
        
        # Check if container is running
        container_instance = await self.lifecycle_manager.get_container_instance(model_id)
        is_running = container_instance is not None
        
        # Get GPU utilization
        gpu_allocation = self.gpu_allocator.get_container_allocation(model_id)
        
        return {
            "id": container_info.id,
            "name": container_info.name,
            "description": container_info.description,
            "family": container_info.family,
            "size": container_info.size,
            "languages": container_info.languages,
            "sample_rate": container_info.sample_rate,
            "gpu_memory_mb": container_info.gpu_memory_mb,
            "is_running": is_running,
            "gpu_allocated": gpu_allocation is not None,
            "image": container_info.image,
            "port": container_info.port,
            "status": container_instance.status.value if container_instance else "stopped"
        }
    
    async def set_model(self, model_id: str) -> bool:
        """
        Set active model by starting appropriate container.
        
        Args:
            model_id: Model identifier
            
        Returns:
            True if model was set successfully
        """
        if not self.is_initialized:
            logger.error("DockerModelManager not initialized")
            return False
        
        async with self._lock:
            # Check if model exists
            if not self.container_registry.is_container_available(model_id):
                logger.error(f"Model {model_id} not found in registry")
                return False
            
            # Check if container is already running
            container_instance = await self.lifecycle_manager.get_container_instance(model_id)
            if container_instance and container_instance.status.value == "running":
                logger.info(f"Model {model_id} is already running")
                self.current_model = model_id
                return True
            
            # Get container configuration
            container_info = self.container_registry.get_container_info(model_id)
            if not container_info:
                logger.error(f"Could not get container info for model {model_id}")
                return False
            
            # Validate GPU requirements
            gpu_validation = self.gpu_allocator.validate_gpu_requirements(container_info.gpu_memory_mb)
            if not gpu_validation["valid"]:
                logger.error(f"GPU requirements not satisfied for model {model_id}: {gpu_validation['message']}")
                return False
            
            # Start container
            container_config = {
                "image": container_info.image,
                "port": container_info.port,
                "gpu_memory_mb": container_info.gpu_memory_mb,
                "environment": container_info.environment,
                "volumes": container_info.volumes,
                "restart_policy": container_info.restart_policy
            }
            
            new_container = await self.lifecycle_manager.start_container(model_id, container_config)
            if not new_container:
                logger.error(f"Failed to start container for model {model_id}")
                return False
            
            # Connect to container
            success = await self.communication_adapter.connect_to_container(
                model_id, 
                container_info.port
            )
            
            if not success:
                # Failed to connect, stop container
                await self.lifecycle_manager.stop_container(model_id)
                logger.error(f"Failed to connect to container for model {model_id}")
                return False
            
            self.current_model = model_id
            logger.info(f"Successfully set model {model_id} as active")
            return True
    
    async def transcribe_file(
        self, 
        audio_file: str,
        model_id: Optional[str] = None,
        progress_callback: Optional[Callable[[float, str], None]] = None,
        response_format: str = "json",
        language: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        Transcribe audio file using containerized model.
        
        Args:
            audio_file: Path to audio file
            model_id: Model identifier (uses current model if not specified)
            progress_callback: Optional progress callback function
            response_format: Response format (json|text|srt)
            language: Optional language code
            
        Returns:
            Transcription result
        """
        if not self.is_initialized:
            raise RuntimeError("DockerModelManager not initialized")
        
        # Determine which model to use
        target_model = model_id or self.current_model
        if not target_model:
            raise ValueError("No model specified and no current model set")
        
        # Ensure model container is running
        if target_model != self.current_model:
            if not await self.set_model(target_model):
                raise RuntimeError(f"Failed to set model {target_model}")
        
        # Update container activity
        await self.lifecycle_manager.update_container_activity(target_model)
        
        # Read audio file
        try:
            with open(audio_file, 'rb') as f:
                audio_data = f.read()
        except Exception as e:
            logger.error(f"Failed to read audio file {audio_file}: {e}")
            raise
        
        # Transcribe using container
        try:
            if progress_callback:
                progress_callback(0.0, f"Starting transcription with model {target_model}")
            
            result = await self.communication_adapter.transcribe_with_container(
                container_id=target_model,
                audio_data=audio_data,
                response_format=response_format,
                language=language,
                progress_callback=progress_callback
            )
            
            # Add metadata
            result["model_id"] = target_model
            result["processing_time"] = result.get("timing", {}).get("total_processing_time", 0)
            
            return result
            
        except Exception as e:
            logger.error(f"Transcription failed with model {target_model}: {e}")
            raise
    
    async def transcribe_data(
        self,
        audio_data: bytes,
        model_id: Optional[str] = None,
        progress_callback: Optional[Callable[[float, str], None]] = None,
        response_format: str = "json",
        language: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        Transcribe audio data using containerized model.
        
        Args:
            audio_data: Audio data as bytes
            model_id: Model identifier (uses current model if not specified)
            progress_callback: Optional progress callback function
            response_format: Response format (json|text|srt)
            language: Optional language code
            
        Returns:
            Transcription result
        """
        if not self.is_initialized:
            raise RuntimeError("DockerModelManager not initialized")
        
        # Determine which model to use
        target_model = model_id or self.current_model
        if not target_model:
            raise ValueError("No model specified and no current model set")
        
        # Ensure model container is running
        if target_model != self.current_model:
            if not await self.set_model(target_model):
                raise RuntimeError(f"Failed to set model {target_model}")
        
        # Update container activity
        await self.lifecycle_manager.update_container_activity(target_model)
        
        # Transcribe using container
        try:
            if progress_callback:
                progress_callback(0.0, f"Starting transcription with model {target_model}")
            
            result = await self.communication_adapter.transcribe_with_container(
                container_id=target_model,
                audio_data=audio_data,
                response_format=response_format,
                language=language,
                progress_callback=progress_callback
            )
            
            # Add metadata
            result["model_id"] = target_model
            result["processing_time"] = result.get("timing", {}).get("total_processing_time", 0)
            
            return result
            
        except Exception as e:
            logger.error(f"Transcription failed with model {target_model}: {e}")
            raise
    
    async def get_current_model(self) -> Optional[str]:
        """
        Get the currently active model.
        
        Returns:
            Current model ID or None if no model is active
        """
        return self.current_model
    
    async def get_system_status(self) -> Dict[str, Any]:
        """
        Get system status including GPU utilization and container status.
        
        Returns:
            System status information
        """
        if not self.is_initialized:
            return {"status": "not_initialized"}
        
        # Get GPU utilization
        gpu_utilization = self.gpu_allocator.get_gpu_utilization()
        
        # Get container lifecycle summary
        lifecycle_summary = await self.lifecycle_manager.get_lifecycle_summary()
        
        # Get communication summary
        communication_summary = self.communication_adapter.get_connection_summary()
        
        # Get container registry summary
        registry_summary = self.container_registry.get_container_summary()
        
        return {
            "status": "initialized",
            "current_model": self.current_model,
            "docker_available": self.docker_config.is_docker_available(),
            "gpu_available": self.docker_config.is_gpu_available(),
            "gpu_utilization": gpu_utilization,
            "containers": lifecycle_summary,
            "connections": communication_summary,
            "registry": registry_summary
        }
    
    async def cleanup(self):
        """Stop all active containers and cleanup resources."""
        async with self._lock:
            if not self.is_initialized:
                return
            
            logger.info("Cleaning up DockerModelManager...")
            
            # Stop lifecycle manager
            await self.lifecycle_manager.stop()
            
            # Cleanup communication adapter
            await self.communication_adapter.cleanup_connections()
            
            # Cleanup GPU allocator
            self.gpu_allocator.cleanup_all_allocations()
            
            self.current_model = None
            self.is_initialized = False
            
            logger.info("DockerModelManager cleanup completed")
    
    async def __aenter__(self):
        """Async context manager entry."""
        await self.initialize()
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit."""
        await self.cleanup()