"""
Integration tests for Docker model runner
"""

import pytest
import asyncio
import logging
from unittest.mock import Mock, patch, AsyncMock

from config.docker_config import DockerConfig
from docker.gpu_allocator import GPUAllocator
from docker.registry import ContainerRegistry
from docker.communication import ContainerCommunicationAdapter
from docker.lifecycle import ContainerLifecycleManager
from docker.model_manager import DockerModelManager

logger = logging.getLogger(__name__)


class TestDockerConfig:
    """Test Docker configuration."""
    
    def test_default_config(self):
        """Test default configuration values."""
        config = DockerConfig()
        
        assert config.get_docker_host() == "unix:///var/run/docker.sock"
        assert config.get_container_timeout() == 30
        assert config.get_inactivity_timeout() == 300
        assert config.get_max_concurrent_containers() == 3
        assert config.get_pull_policy() == "if_missing"
    
    def test_custom_config(self):
        """Test custom configuration values."""
        custom_config = {
            "host": "tcp://localhost:2376",
            "container_timeout": 60,
            "inactivity_timeout": 600,
            "max_concurrent_containers": 5
        }
        
        config = DockerConfig(custom_config)
        
        assert config.get_docker_host() == "tcp://localhost:2376"
        assert config.get_container_timeout() == 60
        assert config.get_inactivity_timeout() == 600
        assert config.get_max_concurrent_containers() == 5
    
    def test_model_container_configs(self):
        """Test model container configurations."""
        config = DockerConfig()
        
        whisper_tiny = config.get_model_container_config("whisper-tiny")
        assert whisper_tiny is not None
        assert whisper_tiny["image"] == "asrpro/whisper-tiny:latest"
        assert whisper_tiny["port"] == 8001
        assert whisper_tiny["gpu_memory_mb"] == 2048
        
        # Test non-existent model
        non_existent = config.get_model_container_config("non-existent-model")
        assert non_existent is None


class TestGPUAllocator:
    """Test GPU allocator."""
    
    def test_initialization(self):
        """Test GPU allocator initialization."""
        allocator = GPUAllocator(total_memory_mb=8192)
        
        assert allocator.total_memory_mb == 8192
        assert allocator.allocated_memory_mb == 0
        assert len(allocator.allocations) == 0
    
    def test_memory_allocation(self):
        """Test GPU memory allocation."""
        allocator = GPUAllocator(total_memory_mb=8192)
        
        # Allocate memory
        assert allocator.can_allocate(2048)
        assert allocator.allocate_gpu_memory("test-container", 2048)
        assert allocator.allocated_memory_mb == 2048
        assert len(allocator.allocations) == 1
        
        # Try to allocate more than available
        assert not allocator.can_allocate(8192)
        assert not allocator.allocate_gpu_memory("test-container-2", 8192)
    
    def test_memory_release(self):
        """Test GPU memory release."""
        allocator = GPUAllocator(total_memory_mb=8192)
        
        # Allocate and release memory
        allocator.allocate_gpu_memory("test-container", 2048)
        assert allocator.release_gpu_memory("test-container")
        assert allocator.allocated_memory_mb == 0
        assert len(allocator.allocations) == 0
        
        # Try to release non-existent allocation
        assert not allocator.release_gpu_memory("non-existent")
    
    def test_activity_update(self):
        """Test activity update for allocations."""
        allocator = GPUAllocator(total_memory_mb=8192)
        
        allocator.allocate_gpu_memory("test-container", 2048)
        allocation = allocator.get_container_allocation("test-container")
        initial_activity = allocation.last_activity
        
        # Update activity
        allocator.update_container_activity("test-container")
        updated_allocation = allocator.get_container_allocation("test-container")
        assert updated_allocation.last_activity > initial_activity
    
    def test_gpu_utilization(self):
        """Test GPU utilization statistics."""
        allocator = GPUAllocator(total_memory_mb=8192)
        
        allocator.allocate_gpu_memory("test-container", 2048)
        utilization = allocator.get_gpu_utilization()
        
        assert utilization["total_memory_mb"] == 8192
        assert utilization["allocated_memory_mb"] == 2048
        assert utilization["available_memory_mb"] == 6144
        assert utilization["utilization_percent"] == 25.0
        assert utilization["active_allocations"] == 1


class TestContainerRegistry:
    """Test container registry."""
    
    def test_initialization(self):
        """Test container registry initialization."""
        registry = ContainerRegistry()
        
        assert len(registry.list_containers()) > 0
        assert "whisper-tiny" in registry.list_containers()
        assert "whisper-base" in registry.list_containers()
    
    def test_container_info(self):
        """Test container information retrieval."""
        registry = ContainerRegistry()
        
        # Get container info
        info = registry.get_container_info("whisper-tiny")
        assert info is not None
        assert info.id == "whisper-tiny"
        assert info.family == "whisper"
        assert info.size == "tiny"
        assert info.port == 8001
        
        # Get non-existent container
        non_existent = registry.get_container_info("non-existent")
        assert non_existent is None
    
    def test_container_availability(self):
        """Test container availability checks."""
        registry = ContainerRegistry()
        
        assert registry.is_container_available("whisper-tiny")
        assert not registry.is_container_available("non-existent")
    
    def test_containers_by_family(self):
        """Test getting containers by family."""
        registry = ContainerRegistry()
        
        whisper_containers = registry.get_containers_by_family("whisper")
        assert len(whisper_containers) > 0
        
        tiny_containers = registry.get_containers_by_size("tiny")
        assert len(tiny_containers) == 1
        assert tiny_containers[0].id == "whisper-tiny"
    
    def test_container_status(self):
        """Test container status management."""
        registry = ContainerRegistry()
        
        # Initial status should be unknown
        assert registry.get_container_status("whisper-tiny").value == "unknown"
        
        # Update status
        registry.update_container_status("whisper-tiny", registry.get_container_info("whisper-tiny").__class__.__module__.ContainerStatus.READY)
        assert registry.get_container_status("whisper-tiny").value == "ready"


class TestContainerCommunicationAdapter:
    """Test container communication adapter."""
    
    @pytest.fixture
    def adapter(self):
        """Create a communication adapter for testing."""
        return ContainerCommunicationAdapter(timeout=10, max_retries=2)
    
    @pytest.mark.asyncio
    async def test_connection_management(self, adapter):
        """Test connection management."""
        # Connect to a container
        with patch('httpx.AsyncClient.get') as mock_get:
            mock_response = Mock()
            mock_response.status_code = 200
            mock_response.json.return_value = {"status": "healthy"}
            mock_get.return_value = mock_response
            
            success = await adapter.connect_to_container("test-container", 8001)
            assert success
            assert adapter.get_connection_status("test-container").value == "connected"
            
            # Disconnect from container
            success = await adapter.disconnect_from_container("test-container")
            assert success
            assert adapter.get_connection_status("test-container") is None
    
    @pytest.mark.asyncio
    async def test_health_check(self, adapter):
        """Test container health check."""
        # Connect to a container first
        with patch('httpx.AsyncClient.get') as mock_get:
            mock_response = Mock()
            mock_response.status_code = 200
            mock_response.json.return_value = {"status": "healthy"}
            mock_get.return_value = mock_response
            
            await adapter.connect_to_container("test-container", 8001)
            
            # Check health
            is_healthy = await adapter.check_container_health("test-container")
            assert is_healthy
    
    @pytest.mark.asyncio
    async def test_transcription_request(self, adapter):
        """Test transcription request."""
        # Connect to a container first
        with patch('httpx.AsyncClient.get') as mock_get:
            mock_response = Mock()
            mock_response.status_code = 200
            mock_response.json.return_value = {"status": "healthy"}
            mock_get.return_value = mock_response
            
            await adapter.connect_to_container("test-container", 8001)
            
            # Mock transcription response
            with patch('httpx.AsyncClient.stream') as mock_stream:
                mock_stream_response = Mock()
                mock_stream_response.status_code = 200
                mock_stream_response.json.return_value = {
                    "text": "Test transcription",
                    "language": "en"
                }
                
                mock_stream.return_value.__aenter__.return_value = mock_stream_response
                
                # Make transcription request
                result = await adapter.transcribe_with_container(
                    "test-container",
                    b"fake audio data"
                )
                
                assert result["text"] == "Test transcription"
                assert result["language"] == "en"
    
    @pytest.mark.asyncio
    async def test_connection_summary(self, adapter):
        """Test connection summary."""
        # Connect to a container
        with patch('httpx.AsyncClient.get') as mock_get:
            mock_response = Mock()
            mock_response.status_code = 200
            mock_response.json.return_value = {"status": "healthy"}
            mock_get.return_value = mock_response
            
            await adapter.connect_to_container("test-container", 8001)
            
            # Get connection summary
            summary = adapter.get_connection_summary()
            
            assert summary["total_connections"] == 1
            assert summary["connected_count"] == 1
            assert "test-container" in summary["connections"]


class TestContainerLifecycleManager:
    """Test container lifecycle manager."""
    
    @pytest.fixture
    def config(self):
        """Create a Docker config for testing."""
        return DockerConfig()
    
    @pytest.fixture
    def gpu_allocator(self):
        """Create a GPU allocator for testing."""
        return GPUAllocator(total_memory_mb=8192)
    
    @pytest.fixture
    def lifecycle_manager(self, config, gpu_allocator):
        """Create a lifecycle manager for testing."""
        with patch('docker.from_env'):
            return ContainerLifecycleManager(config, gpu_allocator)
    
    @pytest.mark.asyncio
    async def test_start_stop(self, lifecycle_manager):
        """Test starting and stopping the lifecycle manager."""
        # Start the manager
        await lifecycle_manager.start()
        assert lifecycle_manager._running
        
        # Stop the manager
        await lifecycle_manager.stop()
        assert not lifecycle_manager._running
    
    @pytest.mark.asyncio
    async def test_container_start_stop(self, lifecycle_manager):
        """Test starting and stopping a container."""
        # Mock Docker container
        with patch('docker.from_env') as mock_docker:
            mock_client = Mock()
            mock_docker.return_value = mock_client
            
            # Mock image check
            mock_client.images.get.return_value = Mock()
            
            # Mock container
            mock_container = Mock()
            mock_container.id = "test-container-id"
            mock_container.status = "running"
            mock_client.containers.run.return_value = mock_container
            
            # Start container
            container_info = {
                "image": "test/image:latest",
                "port": 8001,
                "gpu_memory_mb": 2048,
                "environment": {},
                "volumes": {},
                "restart_policy": "unless-stopped"
            }
            
            container = await lifecycle_manager.start_container("test-model", container_info)
            assert container is not None
            assert container.model_id == "test-model"
            
            # Stop container
            success = await lifecycle_manager.stop_container("test-model")
            assert success
    
    @pytest.mark.asyncio
    async def test_activity_update(self, lifecycle_manager):
        """Test updating container activity."""
        # Mock Docker container
        with patch('docker.from_env') as mock_docker:
            mock_client = Mock()
            mock_docker.return_value = mock_client
            
            # Mock image check
            mock_client.images.get.return_value = Mock()
            
            # Mock container
            mock_container = Mock()
            mock_container.id = "test-container-id"
            mock_container.status = "running"
            mock_client.containers.run.return_value = mock_container
            
            # Start container
            container_info = {
                "image": "test/image:latest",
                "port": 8001,
                "gpu_memory_mb": 2048,
                "environment": {},
                "volumes": {},
                "restart_policy": "unless-stopped"
            }
            
            await lifecycle_manager.start_container("test-model", container_info)
            
            # Update activity
            success = await lifecycle_manager.update_container_activity("test-model")
            assert success
    
    @pytest.mark.asyncio
    async def test_inactive_container_cleanup(self, lifecycle_manager):
        """Test cleanup of inactive containers."""
        # Start the manager
        await lifecycle_manager.start()
        
        # Mock Docker container
        with patch('docker.from_env') as mock_docker:
            mock_client = Mock()
            mock_docker.return_value = mock_client
            
            # Mock image check
            mock_client.images.get.return_value = Mock()
            
            # Mock container
            mock_container = Mock()
            mock_container.id = "test-container-id"
            mock_container.status = "running"
            mock_client.containers.run.return_value = mock_container
            
            # Start container
            container_info = {
                "image": "test/image:latest",
                "port": 8001,
                "gpu_memory_mb": 2048,
                "environment": {},
                "volumes": {},
                "restart_policy": "unless-stopped"
            }
            
            await lifecycle_manager.start_container("test-model", container_info)
            
            # Manually set last activity to simulate inactivity
            container = await lifecycle_manager.get_container_instance("test-model")
            container.last_activity = 0  # Very old timestamp
            
            # Stop inactive containers
            stopped_count = await lifecycle_manager.stop_inactive_containers()
            assert stopped_count == 1
        
        # Stop the manager
        await lifecycle_manager.stop()


class TestDockerModelManager:
    """Test Docker model manager."""
    
    @pytest.fixture
    def model_manager(self):
        """Create a model manager for testing."""
        with patch('docker.from_env'):
            return DockerModelManager()
    
    @pytest.mark.asyncio
    async def test_initialization(self, model_manager):
        """Test model manager initialization."""
        # Mock Docker availability
        with patch.object(model_manager.docker_config, 'validate_config', return_value=True):
            success = await model_manager.initialize()
            assert success
            assert model_manager.is_initialized
    
    @pytest.mark.asyncio
    async def test_list_models(self, model_manager):
        """Test listing available models."""
        # Initialize the manager
        with patch.object(model_manager.docker_config, 'validate_config', return_value=True):
            await model_manager.initialize()
        
        # List models
        models = await model_manager.list_available_models()
        assert len(models) > 0
        assert "whisper-tiny" in models
        assert "whisper-base" in models
    
    @pytest.mark.asyncio
    async def test_get_model_info(self, model_manager):
        """Test getting model information."""
        # Initialize the manager
        with patch.object(model_manager.docker_config, 'validate_config', return_value=True):
            await model_manager.initialize()
        
        # Get model info
        info = await model_manager.get_model_info("whisper-tiny")
        assert info is not None
        assert info["id"] == "whisper-tiny"
        assert info["family"] == "whisper"
        assert info["size"] == "tiny"
        
        # Get non-existent model info
        non_existent = await model_manager.get_model_info("non-existent")
        assert non_existent is None
    
    @pytest.mark.asyncio
    async def test_set_model(self, model_manager):
        """Test setting active model."""
        # Initialize the manager
        with patch.object(model_manager.docker_config, 'validate_config', return_value=True):
            await model_manager.initialize()
        
        # Mock container lifecycle
        with patch.object(model_manager.lifecycle_manager, 'start_container') as mock_start:
            mock_container = Mock()
            mock_container.status.value = "running"
            mock_start.return_value = mock_container
            
            # Mock communication adapter
            with patch.object(model_manager.communication_adapter, 'connect_to_container', return_value=True):
                # Set model
                success = await model_manager.set_model("whisper-tiny")
                assert success
                assert model_manager.current_model == "whisper-tiny"
    
    @pytest.mark.asyncio
    async def test_transcribe_data(self, model_manager):
        """Test transcribing audio data."""
        # Initialize the manager
        with patch.object(model_manager.docker_config, 'validate_config', return_value=True):
            await model_manager.initialize()
        
        # Set model
        with patch.object(model_manager.lifecycle_manager, 'start_container') as mock_start:
            mock_container = Mock()
            mock_container.status.value = "running"
            mock_start.return_value = mock_container
            
            with patch.object(model_manager.communication_adapter, 'connect_to_container', return_value=True):
                await model_manager.set_model("whisper-tiny")
                
                # Mock transcription
                with patch.object(model_manager.communication_adapter, 'transcribe_with_container') as mock_transcribe:
                    mock_transcribe.return_value = {
                        "text": "Test transcription",
                        "language": "en"
                    }
                    
                    # Transcribe audio data
                    result = await model_manager.transcribe_data(
                        b"fake audio data",
                        model_id="whisper-tiny"
                    )
                    
                    assert result["text"] == "Test transcription"
                    assert result["language"] == "en"
                    assert result["model_id"] == "whisper-tiny"
    
    @pytest.mark.asyncio
    async def test_get_system_status(self, model_manager):
        """Test getting system status."""
        # Initialize the manager
        with patch.object(model_manager.docker_config, 'validate_config', return_value=True):
            await model_manager.initialize()
        
        # Get system status
        status = await model_manager.get_system_status()
        
        assert status["status"] == "initialized"
        assert "docker_available" in status
        assert "gpu_available" in status
        assert "gpu_utilization" in status
        assert "containers" in status
        assert "connections" in status
        assert "registry" in status
    
    @pytest.mark.asyncio
    async def test_cleanup(self, model_manager):
        """Test cleanup."""
        # Initialize the manager
        with patch.object(model_manager.docker_config, 'validate_config', return_value=True):
            await model_manager.initialize()
        
        # Set model
        with patch.object(model_manager.lifecycle_manager, 'start_container') as mock_start:
            mock_container = Mock()
            mock_container.status.value = "running"
            mock_start.return_value = mock_container
            
            with patch.object(model_manager.communication_adapter, 'connect_to_container', return_value=True):
                await model_manager.set_model("whisper-tiny")
                
                # Cleanup
                await model_manager.cleanup()
                
                assert model_manager.current_model is None
                assert not model_manager.is_initialized


@pytest.mark.asyncio
async def test_full_integration():
    """Test full integration of Docker components."""
    # This test would require actual Docker to be running
    # For now, we'll just mock the components
    
    with patch('docker.from_env'):
        # Create components
        config = DockerConfig()
        gpu_allocator = GPUAllocator()
        registry = ContainerRegistry(config.config)
        communication_adapter = ContainerCommunicationAdapter()
        lifecycle_manager = ContainerLifecycleManager(config, gpu_allocator)
        
        # Create model manager
        model_manager = DockerModelManager(config.config)
        
        # Initialize
        with patch.object(model_manager.docker_config, 'validate_config', return_value=True):
            assert await model_manager.initialize()
        
        # List models
        models = await model_manager.list_available_models()
        assert len(models) > 0
        
        # Get model info
        info = await model_manager.get_model_info(models[0])
        assert info is not None
        
        # Cleanup
        await model_manager.cleanup()


if __name__ == "__main__":
    # Run tests
    pytest.main([__file__])